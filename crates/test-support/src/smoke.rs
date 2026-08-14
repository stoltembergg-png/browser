//! Engine-neutral MVP smoke harness with bound evidence.
//!
//! The reference-platform smoke runs the MVP flow (clean profile, session
//! open/commit, engine boot, tab create, navigate, render signal, back,
//! forward, reload, stop, shutdown) against the fake engine and records
//! evidence bound to repository, head/tree SHA, engine revision and
//! OS/arch. With the fake engine the report is explicitly `claim: no-alpha`;
//! a failed step makes the whole report `NO_GO` (fail-closed).

use browser_core::session_lifecycle::{PendingWork, SessionLifecycle};
use browser_core::vertical_slice::{SliceResult, VerticalSlice};
use browser_domain::ids::{ProfileId, RequestId, TabId};
use browser_domain::session::SessionRecord;
use browser_domain::ui::{CommandEnvelope, UiCommand, UI_CONTRACT_VERSION};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Result of one smoke step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepResult {
    Pass,
    Skipped(String),
    Fail(String),
}

/// One recorded step of the MVP flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmokeStep {
    pub name: String,
    pub result: StepResult,
    pub detail: String,
}

/// Evidence bundle for one smoke run, bound to the exact checkout.
#[derive(Debug, Clone)]
pub struct SmokeEvidence {
    pub repository: String,
    pub head_sha: String,
    pub tree_sha: String,
    pub engine_revision: String,
    pub os_and_arch: String,
    pub steps: Vec<SmokeStep>,
    pub status: String,
    pub claim: String,
}

impl SmokeEvidence {
    pub fn new(
        repository: String,
        head_sha: String,
        tree_sha: String,
        engine_revision: String,
        os_and_arch: String,
    ) -> Self {
        Self {
            repository,
            head_sha,
            tree_sha,
            engine_revision,
            os_and_arch,
            steps: Vec::new(),
            status: "GO".to_string(),
            claim: "no-alpha".to_string(),
        }
    }

    pub fn add_step(&mut self, name: impl Into<String>, result: StepResult, detail: String) {
        self.steps.push(SmokeStep {
            name: name.into(),
            result,
            detail,
        });
    }

    pub fn pass(&mut self, name: &str) {
        self.add_step(name, StepResult::Pass, String::new());
    }

    pub fn skip(&mut self, name: &str, reason: &str) {
        self.add_step(
            name,
            StepResult::Skipped(reason.to_string()),
            reason.to_string(),
        );
    }

    pub fn fail(&mut self, name: &str, detail: &str) {
        self.add_step(
            name,
            StepResult::Fail(detail.to_string()),
            detail.to_string(),
        );
    }

    /// Fail-closed: any failed step turns the whole report `NO_GO`.
    pub fn finish(&mut self) {
        let has_failure = self
            .steps
            .iter()
            .any(|step| matches!(step.result, StepResult::Fail(_)));
        if has_failure {
            self.status = "NO_GO".to_string();
        }
    }

    /// Validate that the evidence is complete and bound. Returns the list of
    /// missing fields; empty means the evidence is valid.
    pub fn validate(&self) -> Vec<String> {
        let mut missing = Vec::new();
        for (name, value) in [
            ("repository", self.repository.as_str()),
            ("head_sha", self.head_sha.as_str()),
            ("tree_sha", self.tree_sha.as_str()),
            ("engine_revision", self.engine_revision.as_str()),
            ("os_and_arch", self.os_and_arch.as_str()),
        ] {
            if value.is_empty() {
                missing.push(name.to_string());
            }
        }
        if self.steps.is_empty() {
            missing.push("steps".to_string());
        }
        if self.status != "GO" && self.status != "NO_GO" {
            missing.push("status".to_string());
        }
        missing
    }

    pub fn to_json(&self) -> String {
        serde_json::json!({
            "repository": self.repository,
            "commit_sha": self.head_sha,
            "tree_sha": self.tree_sha,
            "engine_revision": self.engine_revision,
            "os_and_arch": self.os_and_arch,
            "status": self.status,
            "claim": self.claim,
            "steps": self.steps.iter().map(|step| serde_json::json!({
                "name": step.name,
                "result": match &step.result {
                    StepResult::Pass => "pass",
                    StepResult::Skipped(_) => "skipped",
                    StepResult::Fail(_) => "fail",
                },
                "detail": step.detail,
            })).collect::<Vec<_>>(),
        })
        .to_string()
    }
}

fn temp_profile(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("pr029-{label}-{nonce}"))
}

fn raw_command(request_id: &str, command: UiCommand) -> String {
    serde_json::to_string(&CommandEnvelope {
        version: UI_CONTRACT_VERSION,
        request_id: RequestId::new(request_id),
        tab_id: Some(TabId::new("tab-1")),
        command,
    })
    .expect("command serializes")
}

/// Run the MVP flow against the fake engine and return bound evidence.
pub fn run_mvp_smoke(
    repository: String,
    head_sha: String,
    tree_sha: String,
    engine_revision: String,
    os_and_arch: String,
) -> SmokeEvidence {
    let mut evidence =
        SmokeEvidence::new(repository, head_sha, tree_sha, engine_revision, os_and_arch);

    // 1. Clean profile and transactional session open/commit.
    let profile_dir = temp_profile("smoke");
    match SessionLifecycle::open(&profile_dir).and_then(|mut lifecycle| {
        lifecycle.begin_shutdown(PendingWork::none())?;
        let record = SessionRecord::new(ProfileId::new("smoke-profile"), now_secs());
        let pending = lifecycle.prepare_session(record)?;
        lifecycle.commit_session(pending)
    }) {
        Ok(receipt) => evidence.add_step(
            "session_clean_profile_open_commit",
            StepResult::Pass,
            format!("sequence {}", receipt.sequence()),
        ),
        Err(error) => evidence.fail("session_clean_profile_open_commit", &error.to_string()),
    }

    // 2. Boot the engine host with the fake engine.
    let mut slice = VerticalSlice::new();
    match slice.boot() {
        Ok(SliceResult::Signal(_)) => evidence.pass("engine_boot"),
        Ok(other) => evidence.fail("engine_boot", &format!("unexpected result {other:?}")),
        Err(error) => evidence.fail("engine_boot", &error),
    }

    // 3. Tab create.
    match slice.create_tab("tab-1") {
        SliceResult::Ok => evidence.pass("tab_create"),
        other => evidence.fail("tab_create", &format!("{other:?}")),
    }

    // 4-5. Navigate and render signal (pump).
    match slice.send_command(&raw_command(
        "req-1",
        UiCommand::Navigate {
            url: "https://one.example".into(),
        },
    )) {
        SliceResult::Ok => evidence.pass("navigate_one"),
        other => evidence.fail("navigate_one", &format!("{other:?}")),
    }
    match slice.pump() {
        SliceResult::Ok | SliceResult::Signal(_) => evidence.pass("render_signal"),
        other => evidence.fail("render_signal", &format!("{other:?}")),
    }

    // 6. Input: no engine-neutral input command exists with the fake engine.
    evidence.skip(
        "input",
        "no engine-neutral input command; engine input path lands with PR-026",
    );

    // 7-8. Second navigation enables back/forward.
    match slice.send_command(&raw_command(
        "req-2",
        UiCommand::Navigate {
            url: "https://two.example".into(),
        },
    )) {
        SliceResult::Ok => evidence.pass("navigate_two"),
        other => evidence.fail("navigate_two", &format!("{other:?}")),
    }
    match slice.send_command(&raw_command("req-3", UiCommand::GoBack)) {
        SliceResult::Ok => evidence.pass("go_back"),
        other => evidence.fail("go_back", &format!("{other:?}")),
    }
    match slice.send_command(&raw_command("req-4", UiCommand::GoForward)) {
        SliceResult::Ok => evidence.pass("go_forward"),
        other => evidence.fail("go_forward", &format!("{other:?}")),
    }

    // 9. Reload and stop.
    match slice.send_command(&raw_command("req-5", UiCommand::Reload)) {
        SliceResult::Ok => evidence.pass("reload"),
        other => evidence.fail("reload", &format!("{other:?}")),
    }
    match slice.send_command(&raw_command("req-6", UiCommand::Stop)) {
        SliceResult::Ok => evidence.pass("stop"),
        other => evidence.fail("stop", &format!("{other:?}")),
    }

    // 10. Shutdown drains the host.
    match slice.shutdown() {
        Ok(_) => evidence.pass("shutdown"),
        Err(error) => evidence.fail("shutdown", &error),
    }

    evidence.finish();
    evidence
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence_fixture() -> SmokeEvidence {
        SmokeEvidence::new(
            "local".into(),
            "a".repeat(40),
            "b".repeat(40),
            "fake@859bd5ed".into(),
            "darwin-aarch64".into(),
        )
    }

    #[test]
    fn evidence_starts_go_with_no_alpha_claim() {
        let evidence = evidence_fixture();
        assert_eq!(evidence.status, "GO");
        assert_eq!(evidence.claim, "no-alpha");
        assert_eq!(evidence.validate(), vec!["steps".to_string()]);
    }

    #[test]
    fn failed_step_turns_report_no_go() {
        let mut evidence = evidence_fixture();
        evidence.pass("a");
        evidence.fail("b", "boom");
        evidence.finish();
        assert_eq!(evidence.status, "NO_GO");
    }

    #[test]
    fn skipped_step_does_not_fail_report() {
        let mut evidence = evidence_fixture();
        evidence.skip("input", "fake engine");
        evidence.pass("boot");
        evidence.finish();
        assert_eq!(evidence.status, "GO");
        assert_eq!(evidence.validate(), Vec::<String>::new());
    }

    #[test]
    fn json_binds_sha_os_and_engine_revision() {
        let mut evidence = evidence_fixture();
        evidence.pass("boot");
        evidence.finish();
        let json = evidence.to_json();
        assert!(json.contains(&format!("\"commit_sha\":\"{}\"", "a".repeat(40))));
        assert!(json.contains("\"tree_sha\":\"bbbb"));
        assert!(json.contains("\"os_and_arch\":\"darwin-aarch64\""));
        assert!(json.contains("\"engine_revision\":\"fake@859bd5ed\""));
        assert!(json.contains("\"status\":\"GO\""));
    }

    #[test]
    fn validate_rejects_missing_binding_fields() {
        let mut evidence = SmokeEvidence::new(
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        );
        evidence.pass("boot");
        let missing = evidence.validate();
        assert_eq!(
            missing,
            vec![
                "repository",
                "head_sha",
                "tree_sha",
                "engine_revision",
                "os_and_arch"
            ]
        );
    }

    #[test]
    fn mvp_smoke_produces_valid_bound_evidence() {
        let evidence = run_mvp_smoke(
            "local".into(),
            "c".repeat(40),
            "d".repeat(40),
            "fake@859bd5ed".into(),
            "darwin-aarch64".into(),
        );
        assert_eq!(evidence.status, "GO", "steps: {:?}", evidence.steps);
        assert!(evidence.validate().is_empty());
        assert!(evidence
            .steps
            .iter()
            .any(|step| matches!(step.result, StepResult::Skipped(_))));
    }
}
