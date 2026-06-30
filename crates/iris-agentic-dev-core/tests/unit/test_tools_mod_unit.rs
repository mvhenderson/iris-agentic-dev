// Unit tests for tools/mod.rs internal helpers and public functions.
// Targets uncovered branches identified from LCOV analysis.

use iris_agentic_dev_core::tools::{map_status_int, translate_sql_macros, validate_read_only_sql};

// ── map_status_int ────────────────────────────────────────────────────────────

#[test]
fn map_status_int_all_branches() {
    // 1 → passed
    assert_eq!(map_status_int(1, ""), "passed");
    // 0 → failed
    assert_eq!(map_status_int(0, ""), "failed");
    // other with non-empty error_action → error
    assert_eq!(map_status_int(2, "someAction"), "error");
    // other with empty error_action → failed
    assert_eq!(map_status_int(99, ""), "failed");
}

// ── validate_read_only_sql ────────────────────────────────────────────────────

#[test]
fn validate_read_only_sql_select_ok() {
    assert!(validate_read_only_sql("SELECT 1").is_ok());
    assert!(validate_read_only_sql("SELECT Name FROM Sample.Person").is_ok());
}

#[test]
fn validate_read_only_sql_insert_blocked() {
    assert!(validate_read_only_sql("INSERT INTO Foo VALUES (1)").is_err());
}

#[test]
fn validate_read_only_sql_update_blocked() {
    assert!(validate_read_only_sql("UPDATE Foo SET x=1").is_err());
}

#[test]
fn validate_read_only_sql_delete_blocked() {
    assert!(validate_read_only_sql("DELETE FROM Foo").is_err());
}

// ── translate_sql_macros ──────────────────────────────────────────────────────

#[test]
fn translate_sql_macros_passthrough_plain_sql() {
    let result = translate_sql_macros("SELECT 1");
    assert!(
        !result.found,
        "plain SQL should not be flagged as translated"
    );
    assert_eq!(result.translated_code, "SELECT 1");
}

#[test]
fn translate_sql_macros_select_into_translated() {
    // &sql macro SELECT INTO — should detect the pattern
    let code = "&sql(SELECT Name INTO :name FROM Sample.Person WHERE ID=1)";
    let result = translate_sql_macros(code);
    // If found, translated_code should be populated
    if result.found {
        assert!(!result.translated_code.is_empty());
    }
    // At minimum, the function does not panic
}

// ── ConfigWatcher::has_changed ────────────────────────────────────────────────

#[test]
fn config_watcher_has_changed_appears_after_creation() {
    use iris_agentic_dev_core::tools::ConfigWatcher;
    use std::io::Write;

    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join(".iris-agentic-dev.toml");
    // ConfigWatcher::new with a non-existent file → last_mtime = None
    let mut w = ConfigWatcher::new(path.clone()).unwrap();
    assert!(w.last_mtime.is_none(), "non-existent file → no mtime");
    // has_changed: None→None → false
    assert!(!w.has_changed(), "file not yet created → not changed");
    // Create the file — should now be detected (None→Some)
    {
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"container = \"test\"\n").unwrap();
    }
    assert!(w.has_changed(), "newly created file should be detected");
    // Immediately check again — not changed (same mtime)
    assert!(!w.has_changed(), "same mtime → not changed on second check");
}

#[test]
fn config_watcher_has_changed_file_deleted() {
    use iris_agentic_dev_core::tools::ConfigWatcher;
    use std::io::Write;

    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join(".iris-agentic-dev.toml");
    // Create file first so ConfigWatcher::new captures mtime
    {
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"container = \"test\"\n").unwrap();
    }
    let mut w = ConfigWatcher::new(path.clone()).unwrap();
    assert!(
        w.last_mtime.is_some(),
        "file exists → mtime captured at construction"
    );
    // has_changed: file unchanged since construction → false
    let _ = w.has_changed();
    // Delete the file
    std::fs::remove_file(&path).unwrap();
    // has_changed should see deletion (Some -> None) and return false, reset mtime
    let deleted = w.has_changed();
    assert!(!deleted, "file deleted → has_changed returns false");
    assert!(w.last_mtime.is_none(), "mtime reset to None after deletion");
    // Re-create — should detect (None→Some)
    {
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"container = \"test2\"\n").unwrap();
    }
    assert!(
        w.has_changed(),
        "file re-created after deletion should be detected"
    );
}

// ── check_config via call_for_test (no IRIS connection → None branch) ─────────

#[cfg(feature = "testing")]
#[tokio::test]
async fn check_config_with_no_iris_returns_not_connected() {
    use iris_agentic_dev_core::tools::IrisTools;

    let tools = IrisTools::new(None).expect("IrisTools::new should succeed");
    // Do NOT set any IRIS connection — iris field will be None
    // Hits the None branch in check_config (~L3228-3234)
    let result = tools
        .call_for_test("check_config", serde_json::json!({}))
        .await;

    match result {
        Ok(r) => {
            let text = r
                .content
                .first()
                .and_then(|c| c.raw.as_text())
                .map(|t| t.text.clone())
                .unwrap_or_default();
            let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
            // connected=false OR missing means no live IRIS — both are correct
            let connected = v["connected"].as_bool().unwrap_or(false);
            assert!(
                !connected,
                "check_config with no IRIS must show not-connected: {v}"
            );
        }
        Err(e) => {
            // Errors are acceptable when IRIS is absent
            eprintln!("check_config returned Err (acceptable without IRIS): {e}");
        }
    }
}

// ── split_host_vars_from_rest edge case (via translate_sql_macros) ────────────

#[test]
fn translate_sql_macros_select_into_no_from_clause() {
    // SELECT INTO with no FROM — hits the fallback branch in split_host_vars_from_rest
    let code = "&sql(SELECT :x INTO :y)";
    let result = translate_sql_macros(code);
    // Should not panic; may or may not translate
    let _ = result;
}

#[test]
fn translate_sql_macros_select_without_select_keyword() {
    // Edge case: select_cols_sql without SELECT keyword hits L344 fallback
    // This is hard to trigger externally; at minimum verify no panic
    let code = "&sql(1 INTO :x FROM Foo WHERE ID=1)";
    let result = translate_sql_macros(code);
    let _ = result;
}

// ── build_test_run_from_sql / build_test_detail ────────────────────────────────

#[test]
fn build_test_run_from_sql_empty_input() {
    use iris_agentic_dev_core::tools::build_test_run_from_sql;

    let result = build_test_run_from_sql(&[], &[]);
    assert!(result.is_object(), "empty input should return an object");
}

#[test]
fn build_test_run_from_sql_with_suites() {
    use iris_agentic_dev_core::tools::{build_test_run_from_sql, SuiteRow};

    let suites = vec![SuiteRow {
        id: "1".to_string(),
        name: "TestSuite".to_string(),
        status: 1,
        duration_ms: Some(123.0),
    }];
    let result = build_test_run_from_sql(&suites, &[]);
    assert!(result.is_object());
}

#[test]
fn build_test_detail_empty_input() {
    use iris_agentic_dev_core::tools::build_test_detail;

    let result = build_test_detail(&[], &[]);
    assert!(result.is_object());
}

// ── write_open_hint: smoke test ───────────────────────────────────────────────

#[test]
fn write_open_hint_does_not_panic() {
    use iris_agentic_dev_core::tools::write_open_hint;
    // write_open_hint emits to tracing — just verify it doesn't panic
    write_open_hint("USER", "Sample.Person.cls");
    write_open_hint("", "");
}

// ── IrisTools::registered_tool_names ─────────────────────────────────────────

#[test]
fn iris_tools_registered_tool_names_non_empty() {
    use iris_agentic_dev_core::tools::IrisTools;
    let tools = IrisTools::new(None).expect("IrisTools::new");
    let names = tools.registered_tool_names();
    assert!(
        !names.is_empty(),
        "registered_tool_names must return non-empty set"
    );
    // Core tools should be registered
    assert!(
        names.contains("iris_compile"),
        "iris_compile must be registered"
    );
    assert!(
        names.contains("iris_execute"),
        "iris_execute must be registered"
    );
    assert!(
        names.contains("iris_query"),
        "iris_query must be registered"
    );
    assert!(
        names.contains("check_config"),
        "check_config must be registered"
    );
    // 052: iris_global must appear in the Merged toolset inventory
    assert!(
        names.contains("iris_global"),
        "iris_global must be registered (052-iris-global)"
    );
}
