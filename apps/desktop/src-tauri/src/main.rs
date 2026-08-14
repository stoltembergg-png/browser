#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run browser desktop shell");
}

#[cfg(test)]
mod security_tests {
    use serde_json::Value;

    const CONFIG: &str = include_str!("../tauri.conf.json");
    const CAPABILITY: &str = include_str!("../capabilities/main.json");
    const FRONTEND: &str = include_str!("../../frontend/index.html");

    fn json(source: &str) -> Value {
        serde_json::from_str(source).expect("security fixture must be valid JSON")
    }

    #[test]
    fn csp_is_local_only_and_has_no_inline_execution() {
        let config = json(CONFIG);
        let csp = config["app"]["security"]["csp"]
            .as_str()
            .expect("string CSP");
        for required in [
            "default-src 'self'",
            "script-src 'self'",
            "style-src 'self'",
            "connect-src 'none'",
            "object-src 'none'",
            "base-uri 'none'",
            "frame-ancestors 'none'",
            "form-action 'none'",
        ] {
            assert!(csp.contains(required), "missing CSP directive: {required}");
        }
        assert!(!csp.contains("unsafe-inline"));
        assert!(!csp.contains("unsafe-eval"));
        assert!(!csp.contains("http://"));
        assert!(!csp.contains("https://"));
        assert!(!FRONTEND.contains("<style>"));
        assert!(!FRONTEND.contains("<script>"));
        assert!(FRONTEND.contains("./styles.css"));
        assert!(FRONTEND.contains("./app.js"));
    }

    #[test]
    fn capability_is_window_scoped_and_denies_privileged_plugins() {
        let capability = json(CAPABILITY);
        assert_eq!(capability["identifier"], "main-window");
        assert_eq!(capability["windows"], serde_json::json!(["main"]));
        assert!(capability["permissions"]
            .as_array()
            .expect("permissions array")
            .is_empty());
        let serialized = capability.to_string();
        for denied_prefix in ["fs:", "http:", "process:", "shell:", "sql:"] {
            assert!(!serialized.contains(denied_prefix));
        }
    }

    #[test]
    fn remote_navigation_is_not_configured() {
        let config = json(CONFIG);
        assert!(config["build"].get("devUrl").is_none());
        assert!(config["app"]["windows"][0].get("url").is_none());
        assert!(!FRONTEND.contains("<iframe"));
    }
}
