//! Brokered download policy and safe local finalization.
//!
//! The page supplies metadata only. The broker owns the profile-bound root,
//! creates a temporary non-final file, enforces the byte quota while streaming,
//! and finalizes with a collision-safe rename.

use browser_domain::ids::{ProfileId, Url};
use std::collections::HashMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const MAX_COLLISION_ATTEMPTS: u32 = 1000;

/// Errors raised by download policy or local finalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadError {
    Traversal,
    AlternateDataStream,
    DeviceName,
    InvalidFilename,
    WrongProfile,
    QuotaExceeded,
    Collision,
    NotFound,
    NotActive,
    Io {
        operation: &'static str,
        detail: String,
    },
}

impl fmt::Display for DownloadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Traversal => formatter.write_str("download filename traversal rejected"),
            Self::AlternateDataStream => {
                formatter.write_str("download alternate data stream rejected")
            }
            Self::DeviceName => formatter.write_str("download device name rejected"),
            Self::InvalidFilename => formatter.write_str("download filename rejected"),
            Self::WrongProfile => formatter.write_str("download profile scope rejected"),
            Self::QuotaExceeded => formatter.write_str("download quota exceeded"),
            Self::Collision => formatter.write_str("download filename collision limit reached"),
            Self::NotFound => formatter.write_str("download not found"),
            Self::NotActive => formatter.write_str("download is not active"),
            Self::Io { operation, detail } => write!(formatter, "{operation} failed: {detail}"),
        }
    }
}

impl std::error::Error for DownloadError {}

fn io_error(operation: &'static str, error: std::io::Error) -> DownloadError {
    DownloadError::Io {
        operation,
        detail: error.to_string(),
    }
}

/// Validate a page-provided filename without ever treating it as a path.
pub fn sanitize_filename(name: &str) -> Result<String, DownloadError> {
    if name.is_empty() || name.len() > 255 || name.trim() != name {
        return Err(DownloadError::InvalidFilename);
    }
    if name == "." || name == ".." {
        return Err(DownloadError::Traversal);
    }
    if name.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(DownloadError::InvalidFilename);
    }
    if name.contains('/') || name.contains('\\') {
        return Err(DownloadError::Traversal);
    }
    if name.contains(':') {
        return Err(DownloadError::AlternateDataStream);
    }

    let stem = name
        .split_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(name)
        .trim_end_matches(['.', ' ']);
    let reserved = matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );
    if reserved {
        return Err(DownloadError::DeviceName);
    }
    Ok(name.to_string())
}

/// Metadata supplied by the engine for one download request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadRequest {
    profile_id: ProfileId,
    source_url: Url,
    suggested_name: String,
    content_length: Option<u64>,
}

impl DownloadRequest {
    pub fn new(
        profile_id: ProfileId,
        source_url: Url,
        suggested_name: impl Into<String>,
        content_length: Option<u64>,
    ) -> Self {
        Self {
            profile_id,
            source_url,
            suggested_name: suggested_name.into(),
            content_length,
        }
    }

    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    pub fn source_url(&self) -> &Url {
        &self.source_url
    }

    pub fn suggested_name(&self) -> &str {
        &self.suggested_name
    }

    pub const fn content_length(&self) -> Option<u64> {
        self.content_length
    }
}

/// Handle returned after the broker has created a temporary file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadHandle {
    id: u64,
    temp_path: PathBuf,
    final_path: PathBuf,
}

impl DownloadHandle {
    pub const fn id(&self) -> u64 {
        self.id
    }

    pub fn temp_path(&self) -> &Path {
        &self.temp_path
    }

    pub fn final_path(&self) -> &Path {
        &self.final_path
    }
}

/// Result of a successful atomic finalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedDownload {
    id: u64,
    final_path: PathBuf,
    bytes_written: u64,
}

impl CompletedDownload {
    pub const fn id(&self) -> u64 {
        self.id
    }

    pub fn final_path(&self) -> &Path {
        &self.final_path
    }

    pub const fn bytes_written(&self) -> u64 {
        self.bytes_written
    }
}

#[derive(Debug, Clone)]
struct ActiveDownload {
    handle: DownloadHandle,
    bytes_written: u64,
}

/// Profile-bound download broker.
#[derive(Debug)]
pub struct DownloadBroker {
    root: PathBuf,
    profile_id: ProfileId,
    max_bytes: u64,
    quarantine: bool,
    next_id: u64,
    active: HashMap<u64, ActiveDownload>,
}

impl DownloadBroker {
    /// Create a broker whose root is canonicalized and owned by one profile.
    pub fn new(
        root: impl AsRef<Path>,
        profile_id: ProfileId,
        max_bytes: u64,
    ) -> Result<Self, DownloadError> {
        fs::create_dir_all(root.as_ref())
            .map_err(|error| io_error("create download root", error))?;
        let root = fs::canonicalize(root.as_ref())
            .map_err(|error| io_error("canonicalize download root", error))?;
        Ok(Self {
            root,
            profile_id,
            max_bytes,
            quarantine: false,
            next_id: 1,
            active: HashMap::new(),
        })
    }

    /// Route completed files into a broker-owned quarantine directory.
    pub const fn with_quarantine(mut self, enabled: bool) -> Self {
        self.quarantine = enabled;
        self
    }

    pub fn start(&mut self, request: DownloadRequest) -> Result<DownloadHandle, DownloadError> {
        if request.profile_id != self.profile_id {
            return Err(DownloadError::WrongProfile);
        }
        let filename = sanitize_filename(&request.suggested_name)?;
        if request
            .content_length
            .is_some_and(|size| size > self.max_bytes)
        {
            return Err(DownloadError::QuotaExceeded);
        }

        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let temp_path = self.root.join(format!(".browser-download-{id}.part"));
        let final_root = self.final_root()?;
        let final_path = self.allocate_final_path(&final_root, &filename)?;
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|error| io_error("create temporary download", error))?;

        let handle = DownloadHandle {
            id,
            temp_path,
            final_path,
        };
        self.active.insert(
            id,
            ActiveDownload {
                handle: handle.clone(),
                bytes_written: 0,
            },
        );
        Ok(handle)
    }

    pub fn write(&mut self, id: u64, bytes: &[u8]) -> Result<(), DownloadError> {
        let current = self
            .active
            .get(&id)
            .ok_or(DownloadError::NotActive)?
            .bytes_written;
        let next = current
            .checked_add(bytes.len() as u64)
            .ok_or(DownloadError::QuotaExceeded)?;
        if next > self.max_bytes {
            self.cancel(id)?;
            return Err(DownloadError::QuotaExceeded);
        }

        let temp_path = self
            .active
            .get(&id)
            .ok_or(DownloadError::NotActive)?
            .handle
            .temp_path
            .clone();
        let mut file = OpenOptions::new()
            .append(true)
            .open(temp_path)
            .map_err(|error| io_error("append temporary download", error))?;
        file.write_all(bytes)
            .map_err(|error| io_error("write temporary download", error))?;
        if let Some(active) = self.active.get_mut(&id) {
            active.bytes_written = next;
        }
        Ok(())
    }

    pub fn finish(&mut self, id: u64) -> Result<CompletedDownload, DownloadError> {
        let active = self
            .active
            .get(&id)
            .cloned()
            .ok_or(DownloadError::NotActive)?;
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&active.handle.temp_path)
            .map_err(|error| io_error("open temporary download", error))?;
        file.flush()
            .and_then(|_| file.sync_all())
            .map_err(|error| io_error("flush temporary download", error))?;
        if active.handle.final_path.exists() {
            return Err(DownloadError::Collision);
        }
        fs::rename(&active.handle.temp_path, &active.handle.final_path)
            .map_err(|error| io_error("finalize download", error))?;
        self.active.remove(&id);
        Ok(CompletedDownload {
            id,
            final_path: active.handle.final_path,
            bytes_written: active.bytes_written,
        })
    }

    pub fn cancel(&mut self, id: u64) -> Result<(), DownloadError> {
        let active = self.active.remove(&id).ok_or(DownloadError::NotActive)?;
        match fs::remove_file(active.handle.temp_path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(io_error("remove temporary download", error)),
        }
    }

    pub fn interrupt(&mut self, id: u64) -> Result<(), DownloadError> {
        self.cancel(id)
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    fn final_root(&self) -> Result<PathBuf, DownloadError> {
        let root = if self.quarantine {
            self.root.join(".quarantine")
        } else {
            self.root.clone()
        };
        fs::create_dir_all(&root).map_err(|error| io_error("create download final root", error))?;
        Ok(root)
    }

    fn allocate_final_path(&self, root: &Path, filename: &str) -> Result<PathBuf, DownloadError> {
        for index in 0..MAX_COLLISION_ATTEMPTS {
            let candidate_name = if index == 0 {
                filename.to_string()
            } else {
                collision_name(filename, index)?
            };
            let candidate = root.join(candidate_name);
            let active_collision = self
                .active
                .values()
                .any(|download| download.handle.final_path == candidate);
            if !candidate.exists() && !active_collision {
                return Ok(candidate);
            }
        }
        Err(DownloadError::Collision)
    }
}

fn collision_name(filename: &str, index: u32) -> Result<String, DownloadError> {
    let candidate = match filename.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => {
            format!("{stem} ({index}).{extension}")
        }
        _ => format!("{filename} ({index})"),
    };
    sanitize_filename(&candidate)
}
