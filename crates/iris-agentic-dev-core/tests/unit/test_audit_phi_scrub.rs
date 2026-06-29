// Tests for audit log PHI scrubbing (051-phi-policy-env-gates, US4, FR-010).
//
// Verifies:
// - PHI-named global name in params is replaced with [REDACTED-PHI]
// - Non-PHI params are not modified
// - Credential/password fields are scrubbed (existing behaviour, not regressed)
// - Scrubbing applies to allowed calls (PHI may appear in params even when permitted)
// - Scrubbing applies to blocked calls too

use iris_agentic_dev_core::iris::audit_log::{scrub_params, AuditLog, AuditLogEntry};
use tempfile::TempDir;

fn make_entry_with_params(params: serde_json::Value) -> AuditLogEntry {
    AuditLogEntry {
        ts: "2026-06-29T11:00:00Z".to_string(),
        tool: "iris_global".to_string(),
        connection: "iris-health".to_string(),
        namespace: "HSLIB".to_string(),
        status: "allowed".to_string(),
        gate: None,
        allowed_categories: None,
        params,
    }
}

// ── scrub_params function tests ───────────────────────────────────────────────

#[test]
fn scrub_phi_global_name_field() {
    let params = serde_json::json!({"global_name": "PAPMI", "keys": []});
    let scrubbed = scrub_params(params);
    assert_eq!(
        scrubbed["global_name"], "[REDACTED-PHI]",
        "PHI global name must be redacted"
    );
}

#[test]
fn scrub_phi_global_name_with_hat_prefix() {
    // Some callers may pass ^PAPMI with the ^ prefix
    let params = serde_json::json!({"global_name": "^PAPMI"});
    let scrubbed = scrub_params(params);
    assert_eq!(scrubbed["global_name"], "[REDACTED-PHI]");
}

#[test]
fn scrub_phi_global_name_case_insensitive() {
    let params = serde_json::json!({"global_name": "papmi1234"});
    let scrubbed = scrub_params(params);
    assert_eq!(scrubbed["global_name"], "[REDACTED-PHI]");
}

#[test]
fn no_scrub_non_phi_global_name() {
    let params = serde_json::json!({"global_name": "MyAppData", "keys": ["k1"]});
    let scrubbed = scrub_params(params);
    assert_eq!(
        scrubbed["global_name"], "MyAppData",
        "non-PHI global name must not be modified"
    );
}

#[test]
fn scrub_password_field() {
    let params = serde_json::json!({"host": "prod.example.com", "password": "secret123"});
    let scrubbed = scrub_params(params);
    assert_eq!(
        scrubbed["password"], "[REDACTED]",
        "password field must be scrubbed"
    );
    assert_eq!(
        scrubbed["host"], "prod.example.com",
        "non-sensitive field must be preserved"
    );
}

#[test]
fn scrub_token_field() {
    let params = serde_json::json!({"token": "eyJhbGciOiJ...", "namespace": "USER"});
    let scrubbed = scrub_params(params);
    assert_eq!(scrubbed["token"], "[REDACTED]");
    assert_eq!(scrubbed["namespace"], "USER");
}

#[test]
fn scrub_api_key_field() {
    let params = serde_json::json!({"api_key": "sk-abc123", "model": "gpt-4"});
    let scrubbed = scrub_params(params);
    assert_eq!(scrubbed["api_key"], "[REDACTED]");
}

#[test]
fn no_scrub_unrelated_params() {
    let params = serde_json::json!({"target": "User.Foo.cls", "flags": 0, "namespace": "USER"});
    let scrubbed = scrub_params(params.clone());
    assert_eq!(scrubbed["target"], "User.Foo.cls");
    assert_eq!(scrubbed["flags"], 0);
    assert_eq!(scrubbed["namespace"], "USER");
}

// ── Integration: write() calls scrub before serializing ───────────────────────

#[test]
fn written_entry_has_phi_global_name_redacted() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("audit.jsonl");
    let log = AuditLog::new(path.clone());

    let entry = make_entry_with_params(serde_json::json!({"global_name": "PAPMI", "keys": []}));
    log.write(&entry).unwrap();

    let line = std::fs::read_to_string(&path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(
        parsed["params"]["global_name"], "[REDACTED-PHI]",
        "written entry must have PHI global name redacted"
    );
}

#[test]
fn written_entry_preserves_non_phi_params() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("audit.jsonl");
    let log = AuditLog::new(path.clone());

    let entry = make_entry_with_params(serde_json::json!({"target": "User.Foo.cls"}));
    log.write(&entry).unwrap();

    let line = std::fs::read_to_string(&path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(parsed["params"]["target"], "User.Foo.cls");
}

#[test]
fn written_entry_has_password_redacted() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("audit.jsonl");
    let log = AuditLog::new(path.clone());

    let entry =
        make_entry_with_params(serde_json::json!({"password": "secret", "host": "prod.com"}));
    log.write(&entry).unwrap();

    let line = std::fs::read_to_string(&path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(parsed["params"]["password"], "[REDACTED]");
    assert_eq!(parsed["params"]["host"], "prod.com");
}
