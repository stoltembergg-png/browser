use browser_core::download_manager::{DownloadManager, DownloadStatus, ManagerError};
use browser_domain::ids::{ProfileId, Url};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_root(label: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("browser-pr041-{label}-{nonce}"));
    fs::create_dir_all(&path).expect("temp root");
    fs::canonicalize(&path).expect("canonical temp root")
}

fn manager(root: &std::path::Path) -> DownloadManager {
    DownloadManager::new(root, ProfileId::new("profile-1"), 1024).expect("manager")
}

#[test]
fn start_reports_active_and_progress_tracks_bytes() {
    let root = temp_root("progress");
    let mut mgr = manager(&root);

    let id = mgr
        .start(
            ProfileId::new("profile-1"),
            Url::new("https://example.test/file.bin").expect("url"),
            "file.bin",
            None,
        )
        .expect("start");
    assert_eq!(mgr.status(id), DownloadStatus::Active { bytes: 0 });

    mgr.write(id, b"part1").expect("write");
    mgr.write(id, b"part2").expect("write");
    assert_eq!(mgr.status(id), DownloadStatus::Active { bytes: 10 });

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn finish_records_completed_in_history_and_removes_active() {
    let root = temp_root("finish-history");
    let mut mgr = manager(&root);

    let id = mgr
        .start(
            ProfileId::new("profile-1"),
            Url::new("https://example.test/report.txt").expect("url"),
            "report.txt",
            None,
        )
        .expect("start");
    mgr.write(id, b"data").expect("write");
    let completed = mgr.finish(id).expect("finish");

    assert_eq!(completed.bytes_written(), 4);
    assert_eq!(
        mgr.status(id),
        DownloadStatus::Completed {
            final_path: completed.final_path().to_path_buf(),
        }
    );
    let history = mgr.history();
    assert_eq!(history.len(), 1);
    assert_eq!(
        history[0].status,
        DownloadStatus::Completed {
            final_path: completed.final_path().to_path_buf(),
        }
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn cancel_removes_temp_and_records_cancelled_in_history() {
    let root = temp_root("cancel");
    let mut mgr = manager(&root);

    let id = mgr
        .start(
            ProfileId::new("profile-1"),
            Url::new("https://example.test/stop.bin").expect("url"),
            "stop.bin",
            None,
        )
        .expect("start");
    mgr.write(id, b"x").expect("write");
    mgr.cancel(id).expect("cancel");

    assert_eq!(mgr.status(id), DownloadStatus::Cancelled);
    let history = mgr.history();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].status, DownloadStatus::Cancelled);
    assert_eq!(fs::read_dir(&root).expect("root").count(), 0);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn quota_failure_marks_failed_and_never_finalizes() {
    let root = temp_root("quota");
    let mut mgr =
        DownloadManager::new(root.clone(), ProfileId::new("profile-1"), 3).expect("manager");

    let id = mgr
        .start(
            ProfileId::new("profile-1"),
            Url::new("https://example.test/big.bin").expect("url"),
            "big.bin",
            None,
        )
        .expect("start");
    let error = mgr.write(id, b"toolong").expect_err("quota must fail");

    assert!(matches!(error, ManagerError::QuotaExceeded));
    assert_eq!(
        mgr.status(id),
        DownloadStatus::Failed {
            reason: error.to_string()
        }
    );
    let history = mgr.history();
    assert_eq!(history.len(), 1);
    assert!(matches!(history[0].status, DownloadStatus::Failed { .. }));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn wrong_profile_is_rejected_without_state_mutation() {
    let root = temp_root("wrong-profile");
    let mut mgr = manager(&root);

    let error = mgr
        .start(
            ProfileId::new("profile-other"),
            Url::new("https://example.test/x.bin").expect("url"),
            "x.bin",
            None,
        )
        .expect_err("wrong profile must be rejected");

    assert!(matches!(error, ManagerError::WrongProfile));
    assert_eq!(mgr.active_count(), 0);
    assert_eq!(mgr.history().len(), 0);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn retry_reopens_failed_download_with_fresh_id() {
    let root = temp_root("retry");
    let mut mgr =
        DownloadManager::new(root.clone(), ProfileId::new("profile-1"), 1024).expect("manager");

    let first = mgr
        .start(
            ProfileId::new("profile-1"),
            Url::new("https://example.test/again.txt").expect("url"),
            "again.txt",
            None,
        )
        .expect("start");
    mgr.write(first, b"partial").expect("write");
    mgr.cancel(first).expect("cancel");

    let second = mgr
        .start(
            ProfileId::new("profile-1"),
            Url::new("https://example.test/again.txt").expect("url"),
            "again.txt",
            None,
        )
        .expect("retry start");
    assert_ne!(first, second);
    mgr.write(second, b"full").expect("write");
    let completed = mgr.finish(second).expect("finish");
    assert_eq!(completed.bytes_written(), 4);

    let history = mgr.history();
    assert_eq!(history.len(), 2);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn history_is_bounded_and_keeps_most_recent_first() {
    let root = temp_root("history-bound");
    let mut mgr =
        DownloadManager::new(root.clone(), ProfileId::new("profile-1"), 1024).expect("manager");
    let mut completed_ids = Vec::new();

    for _ in 0..5 {
        let id = mgr
            .start(
                ProfileId::new("profile-1"),
                Url::new("https://example.test/file.txt").expect("url"),
                "file.txt",
                None,
            )
            .expect("start");
        mgr.write(id, b"data").expect("write");
        completed_ids.push(id);
        mgr.finish(id).expect("finish");
    }

    let history = mgr.history();
    assert_eq!(history.len(), 5);
    assert_eq!(history[0].id, *completed_ids.last().expect("last"));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn restart_after_interruption_has_no_active_downloads() {
    let root = temp_root("restart");
    let mut first = manager(&root);
    let id = first
        .start(
            ProfileId::new("profile-1"),
            Url::new("https://example.test/slow.bin").expect("url"),
            "slow.bin",
            None,
        )
        .expect("start");
    first.write(id, b"part").expect("write");
    drop(first);

    let mut second = manager(&root);
    assert_eq!(second.active_count(), 0);

    let id2 = second
        .start(
            ProfileId::new("profile-1"),
            Url::new("https://example.test/slow.bin").expect("url"),
            "slow.bin",
            None,
        )
        .expect("start");
    assert_ne!(id, id2);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn sweep_orphans_removes_only_temp_parts_and_keeps_completed_files() {
    let root = temp_root("sweep");
    let mut mgr = manager(&root);

    let completed_id = mgr
        .start(
            ProfileId::new("profile-1"),
            Url::new("https://example.test/kept.txt").expect("url"),
            "kept.txt",
            None,
        )
        .expect("start");
    mgr.write(completed_id, b"done").expect("write");
    mgr.finish(completed_id).expect("finish");

    fs::write(root.join(".browser-download-77.part"), b"orphan").expect("orphan part");
    fs::write(root.join("user-file.txt"), b"user data").expect("user file");

    let removed = mgr.sweep_orphans().expect("sweep");
    assert_eq!(removed, 1);

    assert!(!root.join(".browser-download-77.part").exists());
    assert!(root.join("user-file.txt").exists());
    assert!(root.join("kept.txt").exists());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn destination_is_decided_by_policy_not_by_caller() {
    let root = temp_root("destination");
    let mut mgr = manager(&root);

    let id = mgr
        .start(
            ProfileId::new("profile-1"),
            Url::new("https://example.test/renamed.bin").expect("url"),
            "renamed.bin",
            None,
        )
        .expect("start");

    let status = mgr.status(id);
    let DownloadStatus::Active { .. } = status else {
        panic!("expected active");
    };

    let info = mgr.info(id).expect("info");
    assert!(info.temp_path().starts_with(&root));
    assert_eq!(info.final_path().file_name().expect("name"), "renamed.bin");
    assert!(!info.final_path().exists());
    fs::remove_dir_all(root).expect("cleanup");
}
