#![cfg(feature = "servo-backend")]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use engine_api::contract::{
    BrowserEngine, EngineCommand, EngineInstanceId, EngineInstanceSpec, InputEvent, PointerButton,
    ENGINE_API_VERSION,
};
use engine_api::surface::SurfaceSpec;
use serde_json::json;
use servo_engine::{RealServoAdapter, ServoEvidence};
use sha2::{Digest, Sha256};

struct FixtureServer {
    base_url: String,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FixtureServer {
    fn start() -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind fixture server");
        listener
            .set_nonblocking(true)
            .expect("set fixture server nonblocking");
        let address = listener.local_addr().expect("fixture address");
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let thread = thread::spawn(move || {
            while !thread_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) => serve_request(stream),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            base_url: format!("http://{address}"),
            stop,
            thread: Some(thread),
        }
    }

    fn url(&self) -> String {
        format!("{}/", self.base_url)
    }
}

impl Drop for FixtureServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect(self.base_url.trim_start_matches("http://"));
        if let Some(thread) = self.thread.take() {
            thread.join().expect("fixture server thread");
        }
    }
}

fn serve_request(mut stream: TcpStream) {
    let mut request = [0_u8; 4096];
    let length = stream.read(&mut request).unwrap_or(0);
    let request = String::from_utf8_lossy(&request[..length]);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");
    let (title, body) = if path == "/clicked" {
        ("clicked", "<p id=clicked>clicked</p>")
    } else {
        (
            "fixture",
            r#"<a id=link href="/clicked" style="position:absolute;left:0;top:10px;width:240px;height:40px">link</a><input id=input style="position:absolute;left:0;top:80px;width:240px;height:30px" onfocus="document.title='focused'" onkeydown="document.title='key-down'" oninput="document.title=this.value">"#,
        )
    };
    let html = format!("<!doctype html><title>{title}</title><body>{body}</body>");
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        html.len(),
        html
    );
    let _ = stream.write_all(response.as_bytes());
}

fn wait_for<F>(label: &str, adapter: &RealServoAdapter, id: &EngineInstanceId, predicate: F)
where
    F: Fn(&ServoEvidence) -> bool,
{
    let mut last_evidence = None;
    for _ in 0..600 {
        adapter.pump().expect("pump Servo event loop");
        let evidence = adapter.evidence(id).expect("evidence");
        if predicate(&evidence) {
            return;
        }
        last_evidence = Some(evidence);
        thread::sleep(Duration::from_millis(10));
    }
    panic!("Servo fixture condition '{label}' was not observed within timeout: {last_evidence:?}");
}

#[test]
fn real_servo_software_surface_loads_input_resizes_and_shuts_down() {
    let fixture = FixtureServer::start();
    let adapter = RealServoAdapter::new();
    let id = EngineInstanceId("real-fixture".to_string());
    let initial_url = fixture.url();
    adapter
        .create(EngineInstanceSpec {
            instance_id: id.clone(),
            initial_url: Some(initial_url.clone()),
            surface: SurfaceSpec::software(640, 480),
            api_version: ENGINE_API_VERSION,
        })
        .expect("create real Servo instance");

    let initial_origin = initial_url.trim_end_matches('/').to_string();
    wait_for("initial HTTP load", &adapter, &id, |evidence| {
        evidence.frame_count > 0
            && evidence
                .current_url
                .as_deref()
                .is_some_and(|url| url.starts_with(&initial_origin))
            && evidence.load_complete
            && evidence.screenshot_ready
            && evidence.on_current_thread
    });

    adapter
        .send_command(
            &id,
            EngineCommand::Input {
                event: InputEvent::PointerMove { x: 20, y: 90 },
            },
        )
        .expect("move over fixture input");
    adapter
        .send_command(
            &id,
            EngineCommand::Input {
                event: InputEvent::PointerDown {
                    button: PointerButton::Left,
                    x: 20,
                    y: 90,
                },
            },
        )
        .expect("focus fixture input");
    adapter
        .send_command(
            &id,
            EngineCommand::Input {
                event: InputEvent::PointerUp {
                    button: PointerButton::Left,
                    x: 20,
                    y: 90,
                },
            },
        )
        .expect("release fixture input");
    for _ in 0..50 {
        adapter.pump().expect("pump focus event");
        thread::sleep(Duration::from_millis(10));
    }
    wait_for("input element focus", &adapter, &id, |evidence| {
        evidence.title.as_deref() == Some("focused")
    });
    adapter
        .send_command(
            &id,
            EngineCommand::Input {
                event: InputEvent::Text {
                    text: "typed".to_string(),
                },
            },
        )
        .expect("send text input");
    wait_for("text input title", &adapter, &id, |evidence| {
        evidence.title.as_deref() == Some("typed")
    });

    adapter
        .send_command(
            &id,
            EngineCommand::Input {
                event: InputEvent::PointerDown {
                    button: PointerButton::Left,
                    x: 20,
                    y: 20,
                },
            },
        )
        .expect("click link");
    adapter
        .send_command(
            &id,
            EngineCommand::Input {
                event: InputEvent::PointerUp {
                    button: PointerButton::Left,
                    x: 20,
                    y: 20,
                },
            },
        )
        .expect("release link");
    wait_for("link navigation", &adapter, &id, |evidence| {
        evidence
            .current_url
            .as_deref()
            .is_some_and(|url| url.ends_with("/clicked"))
    });

    let before_resize = adapter
        .evidence(&id)
        .expect("pre-resize evidence")
        .frame_count;
    adapter
        .send_command(
            &id,
            EngineCommand::SetViewport {
                width: 800,
                height: 600,
            },
        )
        .expect("resize real surface");
    wait_for("resize frame", &adapter, &id, |evidence| {
        evidence.viewport.width == 800
            && evidence.viewport.height == 600
            && evidence.frame_count >= before_resize
    });

    let final_evidence = adapter.evidence(&id).expect("final evidence");
    assert!(final_evidence.on_current_thread);
    assert!(final_evidence.load_complete);
    assert!(final_evidence.screenshot_ready);
    assert!(final_evidence.frame_digest.is_some());
    adapter.destroy(&id).expect("ordered Servo shutdown");
    write_evidence_artifact(&final_evidence);
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn write_evidence_artifact(evidence: &ServoEvidence) {
    let Some(path) = std::env::var_os("PR026_ARTIFACT_PATH") else {
        return;
    };
    let repository = std::env::var("PR026_REPOSITORY").expect("PR026_REPOSITORY for artifact");
    let commit_sha = std::env::var("PR026_COMMIT_SHA").expect("PR026_COMMIT_SHA for artifact");
    let tree_sha = std::env::var("PR026_TREE_SHA").expect("PR026_TREE_SHA for artifact");
    let os_and_arch = std::env::var("PR026_OS_ARCH").expect("PR026_OS_ARCH for artifact");
    let unsigned = json!({
        "status": "pass",
        "repository": repository,
        "commit_sha": commit_sha,
        "tree_sha": tree_sha,
        "servo_revision": evidence.servo_revision,
        "engine_revision": evidence.servo_revision,
        "os_and_arch": os_and_arch,
        "surface_strategy": evidence.surface_strategy,
        "thread_affinity": "engine-thread-only",
        "frame_digest": evidence.frame_digest.clone(),
        "load_complete": evidence.load_complete,
        "screenshot_ready": evidence.screenshot_ready,
        "frame_count": evidence.frame_count,
        "cases": {
            "http_fixture_load": "pass",
            "frame_ready_paint_present": "pass",
            "text_input": "pass",
            "click_link": "pass",
            "resize": "pass",
            "thread_affinity": "pass",
            "shutdown_no_deadlock": "pass"
        }
    });
    let unsigned_text = serde_json::to_vec(&unsigned).expect("serialize unsigned artifact");
    let digest = Sha256::digest(&unsigned_text);
    let artifact = json!({
        "status": "pass",
        "repository": unsigned["repository"],
        "commit_sha": unsigned["commit_sha"],
        "tree_sha": unsigned["tree_sha"],
        "servo_revision": unsigned["servo_revision"],
        "engine_revision": unsigned["engine_revision"],
        "os_and_arch": unsigned["os_and_arch"],
        "surface_strategy": unsigned["surface_strategy"],
        "thread_affinity": unsigned["thread_affinity"],
        "artifact_digest": hex_digest(digest),
        "frame_digest": unsigned["frame_digest"],
        "load_complete": unsigned["load_complete"],
        "screenshot_ready": unsigned["screenshot_ready"],
        "frame_count": unsigned["frame_count"],
        "cases": unsigned["cases"]
    });
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&artifact).expect("serialize artifact"),
    )
    .expect("write PR-026 artifact");
}
