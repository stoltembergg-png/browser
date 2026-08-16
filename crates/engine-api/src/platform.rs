//! Engine-neutral platform input, scale, focus and accessibility boundary.
//!
//! Native window events are normalized here before they reach `EngineCommand`.
//! This module intentionally contains no Tauri, Servo or OS toolkit types.
//! Platform adapters must preserve event identity and ordering, and must reject
//! unsupported paths explicitly instead of silently dropping them.

use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::contract::{InputEvent, PointerButton};
use crate::surface::Viewport;

/// Version of the platform event contract.
pub const PLATFORM_CONTRACT_VERSION: u32 = 1;
/// Maximum byte length of a text input payload.
pub const MAX_TEXT_BYTES: usize = 4096;
const MAX_KEY_BYTES: usize = 128;
const MAX_SCALE_COMPONENT: u32 = 16;

/// Operating-system identity carried by native input fixtures and evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlatformOs {
    Linux,
    Windows,
    Macos,
}

/// Whether the native window currently owns keyboard/pointer focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusState {
    Focused,
    Unfocused,
}

/// A bounded rational scale factor. Rational representation avoids float
/// rounding differences between OS adapters and keeps the wire contract stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScaleFactor {
    pub numerator: u32,
    pub denominator: u32,
}

impl ScaleFactor {
    pub fn one() -> Self {
        Self {
            numerator: 1,
            denominator: 1,
        }
    }

    pub fn new(numerator: u32, denominator: u32) -> Result<Self, PlatformInputError> {
        if numerator == 0
            || denominator == 0
            || numerator > MAX_SCALE_COMPONENT
            || denominator > MAX_SCALE_COMPONENT
        {
            return Err(PlatformInputError::InvalidScale);
        }
        let divisor = greatest_common_divisor(numerator, denominator);
        Ok(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }

    fn validate(self) -> Result<(), PlatformInputError> {
        if self.numerator == 0
            || self.denominator == 0
            || self.numerator > MAX_SCALE_COMPONENT
            || self.denominator > MAX_SCALE_COMPONENT
            || greatest_common_divisor(self.numerator, self.denominator) != 1
        {
            return Err(PlatformInputError::InvalidScale);
        }
        Ok(())
    }

    /// Convert a logical coordinate to a device-pixel coordinate using
    /// deterministic nearest-integer rounding.
    pub fn logical_to_device(self, logical: i32) -> Result<i32, PlatformInputError> {
        let magnitude = i64::from(logical).unsigned_abs();
        let scaled = magnitude
            .checked_mul(u64::from(self.numerator))
            .ok_or(PlatformInputError::CoordinateOverflow)?;
        let rounded = scaled
            .checked_add(u64::from(self.denominator / 2))
            .ok_or(PlatformInputError::CoordinateOverflow)?
            / u64::from(self.denominator);
        let signed = if logical < 0 {
            -(i64::try_from(rounded).map_err(|_| PlatformInputError::CoordinateOverflow)?)
        } else {
            i64::try_from(rounded).map_err(|_| PlatformInputError::CoordinateOverflow)?
        };
        i32::try_from(signed).map_err(|_| PlatformInputError::CoordinateOverflow)
    }
}

fn greatest_common_divisor(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

/// Native event before engine normalization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlatformInputEvent {
    FocusChanged {
        focused: bool,
    },
    PointerMove {
        x: i32,
        y: i32,
    },
    PointerDown {
        button: PointerButton,
        x: i32,
        y: i32,
    },
    PointerUp {
        button: PointerButton,
        x: i32,
        y: i32,
    },
    KeyDown {
        key: String,
    },
    KeyUp {
        key: String,
    },
    Text {
        text: String,
    },
}

/// A native event with identity, ordering, platform and post-event focus state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformInputEnvelope {
    pub contract_version: u32,
    pub platform: PlatformOs,
    pub event_id: u64,
    pub sequence: u64,
    pub scale_factor: ScaleFactor,
    pub focus: FocusState,
    pub event: PlatformInputEvent,
}

impl PlatformInputEnvelope {
    /// Constructor used by platform adapters and contract fixtures.
    pub fn focused(
        platform: PlatformOs,
        event_id: u64,
        sequence: u64,
        scale_factor: ScaleFactor,
        event: PlatformInputEvent,
        focus: FocusState,
    ) -> Self {
        Self {
            contract_version: PLATFORM_CONTRACT_VERSION,
            platform,
            event_id,
            sequence,
            scale_factor,
            focus,
            event,
        }
    }

    pub fn event_id(&self) -> u64 {
        self.event_id
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }
}

/// One-to-one output of normalization. Focus events have no engine payload but
/// remain visible as first-class events, so normalization never drops them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizedPlatformEvent {
    FocusChanged {
        event_id: u64,
        sequence: u64,
        platform: PlatformOs,
        scale_factor: ScaleFactor,
        focused: bool,
    },
    Engine {
        event_id: u64,
        sequence: u64,
        platform: PlatformOs,
        scale_factor: ScaleFactor,
        event: InputEvent,
    },
}

impl NormalizedPlatformEvent {
    pub fn event_id(&self) -> u64 {
        match self {
            Self::FocusChanged { event_id, .. } | Self::Engine { event_id, .. } => *event_id,
        }
    }

    pub fn sequence(&self) -> u64 {
        match self {
            Self::FocusChanged { sequence, .. } | Self::Engine { sequence, .. } => *sequence,
        }
    }

    pub fn platform(&self) -> PlatformOs {
        match self {
            Self::FocusChanged { platform, .. } | Self::Engine { platform, .. } => *platform,
        }
    }
}

/// Explicit failures for invalid, stale or unsupported platform input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformInputError {
    UnknownContractVersion { version: u32 },
    InvalidEventId,
    DuplicateEventId,
    SequenceNotIncreasing,
    PlatformChanged,
    ScaleChanged,
    FocusRequired,
    FocusStateMismatch,
    FocusEventNotEngineInput,
    InvalidScale,
    CoordinateOverflow,
    CoordinateOutsideViewport,
    InvalidText,
    InvalidKey,
}

impl fmt::Display for PlatformInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::UnknownContractVersion { version } => {
                return write!(formatter, "unknown platform contract version: {version}")
            }
            Self::InvalidEventId => "event_id must be non-zero",
            Self::DuplicateEventId => "event_id was repeated",
            Self::SequenceNotIncreasing => "event sequence must strictly increase",
            Self::PlatformChanged => "platform changed inside one input batch",
            Self::ScaleChanged => "scale factor changed inside one input batch",
            Self::FocusRequired => "focused input is required",
            Self::FocusStateMismatch => "envelope focus does not match the event state",
            Self::FocusEventNotEngineInput => "focus event cannot be sent to the engine input API",
            Self::InvalidScale => "scale factor is outside the bounded contract",
            Self::CoordinateOverflow => "coordinate conversion overflowed",
            Self::CoordinateOutsideViewport => "coordinate is outside the device viewport",
            Self::InvalidText => "text must be non-empty, NUL-free and at most 4096 bytes",
            Self::InvalidKey => "key must be non-empty, NUL-free and at most 128 bytes",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for PlatformInputError {}

/// Normalize one ordered native batch into engine-neutral input events.
pub fn normalize_events(
    events: &[PlatformInputEnvelope],
    viewport: Viewport,
) -> Result<Vec<NormalizedPlatformEvent>, PlatformInputError> {
    let Some(first) = events.first() else {
        return Ok(Vec::new());
    };
    let platform = first.platform;
    let scale_factor = first.scale_factor;
    let mut focused = false;
    let mut last_sequence = 0;
    let mut seen_ids = HashSet::with_capacity(events.len());
    let mut normalized = Vec::with_capacity(events.len());

    for envelope in events {
        if envelope.contract_version != PLATFORM_CONTRACT_VERSION {
            return Err(PlatformInputError::UnknownContractVersion {
                version: envelope.contract_version,
            });
        }
        if envelope.event_id == 0 {
            return Err(PlatformInputError::InvalidEventId);
        }
        envelope.scale_factor.validate()?;
        if !seen_ids.insert(envelope.event_id) {
            return Err(PlatformInputError::DuplicateEventId);
        }
        if envelope.sequence == 0 || envelope.sequence <= last_sequence {
            return Err(PlatformInputError::SequenceNotIncreasing);
        }
        last_sequence = envelope.sequence;
        if envelope.platform != platform {
            return Err(PlatformInputError::PlatformChanged);
        }
        if envelope.scale_factor != scale_factor {
            return Err(PlatformInputError::ScaleChanged);
        }

        match &envelope.event {
            PlatformInputEvent::FocusChanged { focused: next } => {
                if envelope.focus
                    != if *next {
                        FocusState::Focused
                    } else {
                        FocusState::Unfocused
                    }
                {
                    return Err(PlatformInputError::FocusStateMismatch);
                }
                focused = *next;
                normalized.push(NormalizedPlatformEvent::FocusChanged {
                    event_id: envelope.event_id,
                    sequence: envelope.sequence,
                    platform,
                    scale_factor,
                    focused: *next,
                });
            }
            event => {
                if !focused || envelope.focus != FocusState::Focused {
                    return Err(PlatformInputError::FocusRequired);
                }
                normalized.push(NormalizedPlatformEvent::Engine {
                    event_id: envelope.event_id,
                    sequence: envelope.sequence,
                    platform,
                    scale_factor,
                    event: normalize_engine_event(event, scale_factor, viewport)?,
                });
            }
        }
    }
    Ok(normalized)
}

fn normalize_engine_event(
    event: &PlatformInputEvent,
    scale_factor: ScaleFactor,
    viewport: Viewport,
) -> Result<InputEvent, PlatformInputError> {
    let point = |x: i32, y: i32| {
        let device_x = scale_factor.logical_to_device(x)?;
        let device_y = scale_factor.logical_to_device(y)?;
        if device_x < 0
            || device_y < 0
            || u64::try_from(device_x).map_err(|_| PlatformInputError::CoordinateOverflow)?
                >= u64::from(viewport.width)
            || u64::try_from(device_y).map_err(|_| PlatformInputError::CoordinateOverflow)?
                >= u64::from(viewport.height)
        {
            return Err(PlatformInputError::CoordinateOutsideViewport);
        }
        Ok((device_x, device_y))
    };

    match event {
        PlatformInputEvent::PointerMove { x, y } => {
            let (x, y) = point(*x, *y)?;
            Ok(InputEvent::PointerMove { x, y })
        }
        PlatformInputEvent::PointerDown { button, x, y } => {
            let (x, y) = point(*x, *y)?;
            Ok(InputEvent::PointerDown {
                button: *button,
                x,
                y,
            })
        }
        PlatformInputEvent::PointerUp { button, x, y } => {
            let (x, y) = point(*x, *y)?;
            Ok(InputEvent::PointerUp {
                button: *button,
                x,
                y,
            })
        }
        PlatformInputEvent::KeyDown { key } => {
            validate_key(key)?;
            Ok(InputEvent::KeyDown { key: key.clone() })
        }
        PlatformInputEvent::KeyUp { key } => {
            validate_key(key)?;
            Ok(InputEvent::KeyUp { key: key.clone() })
        }
        PlatformInputEvent::Text { text } => {
            validate_text(text)?;
            Ok(InputEvent::Text { text: text.clone() })
        }
        PlatformInputEvent::FocusChanged { .. } => {
            Err(PlatformInputError::FocusEventNotEngineInput)
        }
    }
}

fn validate_text(text: &str) -> Result<(), PlatformInputError> {
    if text.is_empty() || text.as_bytes().len() > MAX_TEXT_BYTES || text.contains('\0') {
        return Err(PlatformInputError::InvalidText);
    }
    Ok(())
}

fn validate_key(key: &str) -> Result<(), PlatformInputError> {
    if key.is_empty() || key.as_bytes().len() > MAX_KEY_BYTES || key.contains('\0') {
        return Err(PlatformInputError::InvalidKey);
    }
    Ok(())
}

/// Whether a narrowly-scoped accessibility capability is contracted or not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessibilitySupport {
    Contracted,
    Unsupported { reason: String },
}

/// Platform-bound accessibility boundary. This is a contract declaration, not
/// a claim that the native bridge has already been exercised on every OS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessibilityContract {
    pub contract_version: u32,
    pub platform: PlatformOs,
    pub focus_events: AccessibilitySupport,
    pub keyboard_navigation: AccessibilitySupport,
    pub text_input: AccessibilitySupport,
    pub screen_reader_bridge: AccessibilitySupport,
}

impl AccessibilityContract {
    pub fn for_platform(platform: PlatformOs) -> Self {
        Self {
            contract_version: PLATFORM_CONTRACT_VERSION,
            platform,
            focus_events: AccessibilitySupport::Contracted,
            keyboard_navigation: AccessibilitySupport::Contracted,
            text_input: AccessibilitySupport::Contracted,
            screen_reader_bridge: AccessibilitySupport::Unsupported {
                reason: "native screen-reader bridge is outside PR-051".to_string(),
            },
        }
    }

    pub fn screen_reader_reason(&self) -> Option<&str> {
        match &self.screen_reader_bridge {
            AccessibilitySupport::Unsupported { reason } => Some(reason.as_str()),
            AccessibilitySupport::Contracted => None,
        }
    }
}
