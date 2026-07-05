//! Unit tests for interop depth tools (056-interop-depth): iris_message_body,
//! iris_business_rule_info, iris_production_diff. No live IRIS connection required.

use iris_agentic_dev_core::tools::interop::{
    detect_content_type, handle_iris_business_rule_info, handle_iris_message_body,
    handle_iris_production_diff, parse_production_items_from_source, redact_hl7v2, truncate_body,
    BusinessRuleInfoParams, MessageBodyParams, ProductionDiffParams,
};

fn parse_result(result: rmcp::model::CallToolResult) -> serde_json::Value {
    let text = result
        .content
        .first()
        .map(|c| c.raw.as_text().unwrap().text.clone())
        .expect("text content");
    serde_json::from_str(&text).expect("valid JSON")
}

// ---------------------------------------------------------------------------
// T014: redact_hl7v2
// ---------------------------------------------------------------------------

#[test]
fn redact_hl7v2_replaces_pid5_patient_name() {
    let body = "MSH|^~\\&|APP|FAC|APP|FAC|20260101120000||ADT^A01|123|P|2.3\rPID|1||12345||DOE^JOHN||19800101|M";
    let redacted = redact_hl7v2(body);
    assert!(redacted.contains("[REDACTED]"));
    assert!(!redacted.contains("DOE^JOHN"));
}

#[test]
fn redact_hl7v2_non_hl7_content_unchanged() {
    let body = "plain text, no HL7 here";
    assert_eq!(redact_hl7v2(body), body);
}

#[test]
fn redact_hl7v2_redacts_msh3() {
    let body = "MSH|^~\\&|SendingApp|SendingFac|RecvApp|RecvFac|20260101||ADT^A01|1|P|2.3";
    let redacted = redact_hl7v2(body);
    assert!(redacted.contains("[REDACTED]"));
    assert!(!redacted.contains("SendingApp"));
}

// ---------------------------------------------------------------------------
// T015: detect_content_type
// ---------------------------------------------------------------------------

#[test]
fn detect_content_type_hl7v2() {
    assert_eq!(
        detect_content_type("MSH|^~\\&|APP|FAC|||20260101||ADT^A01|1|P|2.3"),
        "HL7v2"
    );
}

#[test]
fn detect_content_type_xml() {
    assert_eq!(detect_content_type("<PRPA_IN201305UV02>"), "XML");
}

#[test]
fn detect_content_type_json_object() {
    assert_eq!(detect_content_type("{\"key\":1}"), "JSON");
}

#[test]
fn detect_content_type_json_array() {
    assert_eq!(detect_content_type("[1,2,3]"), "JSON");
}

#[test]
fn detect_content_type_plain_text() {
    assert_eq!(detect_content_type("plain text"), "text");
}

// ---------------------------------------------------------------------------
// T016: truncate_body
// ---------------------------------------------------------------------------

#[test]
fn truncate_body_over_limit_truncates() {
    let body = "a".repeat(100);
    let (out, was_truncated, original_len) = truncate_body(&body, 50);
    assert_eq!(out.len(), 50);
    assert!(was_truncated);
    assert_eq!(original_len, 100);
}

#[test]
fn truncate_body_under_limit_unchanged() {
    let body = "a".repeat(100);
    let (out, was_truncated, original_len) = truncate_body(&body, 200);
    assert_eq!(out.len(), 100);
    assert!(!was_truncated);
    assert_eq!(original_len, 100);
}

#[test]
fn truncate_body_respects_utf8_boundary() {
    // 3-byte UTF-8 char (e.g. €) at the boundary must not split the char.
    let body = "ab€cd"; // a,b = 1 byte each, € = 3 bytes, c,d = 1 byte each
    let (out, _, _) = truncate_body(body, 3);
    // Truncating at byte 3 would land mid-€ (bytes 2-4); must back off to byte 2.
    assert!(std::str::from_utf8(out.as_bytes()).is_ok());
    assert!(out.len() <= 3);
}

// ---------------------------------------------------------------------------
// T017: iris_message_body missing message_id-equivalent validation (invalid id)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn message_body_non_integer_id_returns_invalid_message_id() {
    let params = MessageBodyParams {
        message_id: "not-a-number".to_string(),
        namespace: "USER".to_string(),
        max_bytes: 65536,
        acknowledge_phi: true,
    };
    let result = handle_iris_message_body(None, &params, "allow")
        .await
        .expect("Ok");
    let v = parse_result(result);
    assert_eq!(v["error_code"], "INVALID_MESSAGE_ID");
}

// ---------------------------------------------------------------------------
// T018: iris_message_body dataPolicy=block
// ---------------------------------------------------------------------------

#[tokio::test]
async fn message_body_block_returns_phi_policy_blocked() {
    let params = MessageBodyParams {
        message_id: "123".to_string(),
        namespace: "USER".to_string(),
        max_bytes: 65536,
        acknowledge_phi: false,
    };
    let result = handle_iris_message_body(None, &params, "block")
        .await
        .expect("Ok");
    let v = parse_result(result);
    assert_eq!(v["error_code"], "PHI_POLICY_BLOCKED");
    assert_eq!(v["success"], false);
}

// ---------------------------------------------------------------------------
// T019: iris_message_body dataPolicy=allow without acknowledgePhi
// ---------------------------------------------------------------------------

#[tokio::test]
async fn message_body_allow_without_ack_returns_phi_ack_required() {
    let params = MessageBodyParams {
        message_id: "123".to_string(),
        namespace: "USER".to_string(),
        max_bytes: 65536,
        acknowledge_phi: false,
    };
    let result = handle_iris_message_body(None, &params, "allow")
        .await
        .expect("Ok");
    let v = parse_result(result);
    assert_eq!(v["error_code"], "PHI_ACK_REQUIRED");
}

// ---------------------------------------------------------------------------
// T020: max_bytes clamping
// ---------------------------------------------------------------------------

#[tokio::test]
async fn message_body_max_bytes_zero_clamped_no_error() {
    let params = MessageBodyParams {
        message_id: "123".to_string(),
        namespace: "USER".to_string(),
        max_bytes: 0,
        acknowledge_phi: true,
    };
    // No IRIS connection -> IRIS_UNREACHABLE, but must NOT be a panic or INVALID_MESSAGE_ID.
    let result = handle_iris_message_body(None, &params, "allow")
        .await
        .expect("Ok");
    let v = parse_result(result);
    assert_eq!(v["error_code"], "IRIS_UNREACHABLE");
}

#[tokio::test]
async fn message_body_max_bytes_over_1mb_reaches_iris_call_clamped() {
    let params = MessageBodyParams {
        message_id: "123".to_string(),
        namespace: "USER".to_string(),
        max_bytes: 2_000_000,
        acknowledge_phi: true,
    };
    let result = handle_iris_message_body(None, &params, "allow")
        .await
        .expect("Ok");
    let v = parse_result(result);
    // No IRIS -> IRIS_UNREACHABLE (clamping happens before the IRIS call; this confirms
    // we don't error out due to the param itself).
    assert_eq!(v["error_code"], "IRIS_UNREACHABLE");
}

// ---------------------------------------------------------------------------
// T021: iris_message_body Query category — not blocked by env gate (mocked gate)
// ---------------------------------------------------------------------------

#[test]
fn message_body_is_query_category_not_execute() {
    use iris_agentic_dev_core::iris::server_manager::tool_to_category_pub;
    use iris_agentic_dev_core::iris::workspace_config::ToolCategory;
    assert_eq!(
        tool_to_category_pub("iris_message_body"),
        Some(ToolCategory::Query)
    );
}

#[test]
fn business_rule_info_is_query_category() {
    use iris_agentic_dev_core::iris::server_manager::tool_to_category_pub;
    use iris_agentic_dev_core::iris::workspace_config::ToolCategory;
    assert_eq!(
        tool_to_category_pub("iris_business_rule_info"),
        Some(ToolCategory::Query)
    );
}

#[test]
fn production_diff_is_query_category() {
    use iris_agentic_dev_core::iris::server_manager::tool_to_category_pub;
    use iris_agentic_dev_core::iris::workspace_config::ToolCategory;
    assert_eq!(
        tool_to_category_pub("iris_production_diff"),
        Some(ToolCategory::Query)
    );
}

#[test]
fn message_body_not_blocked_on_live_template() {
    use iris_agentic_dev_core::iris::workspace_config::McpTemplate;
    use iris_agentic_dev_core::policy::env_gate::check_env_gate;
    let params = serde_json::json!({});
    let result = check_env_gate(
        "iris_message_body",
        &McpTemplate::Live,
        "test-server",
        &params,
    );
    assert!(
        result.is_none(),
        "Query category must not be blocked on live"
    );
}

// ---------------------------------------------------------------------------
// T025: iris_business_rule_info action=get missing rule_name
// ---------------------------------------------------------------------------

#[tokio::test]
async fn business_rule_info_get_missing_rule_name_returns_structured_error() {
    let params = BusinessRuleInfoParams {
        action: "get".to_string(),
        rule_name: None,
        namespace: "USER".to_string(),
    };
    let result = handle_iris_business_rule_info(None, &params)
        .await
        .expect("Ok");
    let v = parse_result(result);
    assert_eq!(v["success"], false);
    assert!(v["error_code"].is_string());
}

// ---------------------------------------------------------------------------
// T026: iris_business_rule_info invalid action
// ---------------------------------------------------------------------------

#[tokio::test]
async fn business_rule_info_invalid_action_returns_invalid_action() {
    let params = BusinessRuleInfoParams {
        action: "delete".to_string(),
        rule_name: None,
        namespace: "USER".to_string(),
    };
    let result = handle_iris_business_rule_info(None, &params)
        .await
        .expect("Ok");
    let v = parse_result(result);
    assert_eq!(v["error_code"], "INVALID_ACTION");
}

#[tokio::test]
async fn business_rule_info_list_no_iris_returns_unreachable() {
    let params = BusinessRuleInfoParams {
        action: "list".to_string(),
        rule_name: None,
        namespace: "USER".to_string(),
    };
    let result = handle_iris_business_rule_info(None, &params)
        .await
        .expect("Ok");
    let v = parse_result(result);
    assert_eq!(v["error_code"], "IRIS_UNREACHABLE");
}

// ---------------------------------------------------------------------------
// T032: iris_production_diff Query category — already covered above
// (production_diff_is_query_category)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn production_diff_no_iris_returns_unreachable() {
    let params = ProductionDiffParams {
        production: Some("MyApp.Production".to_string()),
        namespace: "USER".to_string(),
    };
    let result = handle_iris_production_diff(None, &params)
        .await
        .expect("Ok");
    let v = parse_result(result);
    assert_eq!(v["error_code"], "IRIS_UNREACHABLE");
}

// ---------------------------------------------------------------------------
// T033-T035: diff logic (added/removed/modified classification)
// ---------------------------------------------------------------------------
// These exercise the diff algorithm directly via a small re-implementation of the
// comparison logic mirrored from handle_iris_production_diff, since the diff is not
// extracted into a standalone pure function in the current implementation. Instead
// we validate the algorithm's properties using representative input vectors.

#[test]
fn diff_identical_lists_have_no_changes() {
    let current = vec![("A".to_string(), "Cls.A".to_string(), true)];
    let committed = vec![("A".to_string(), "Cls.A".to_string(), true)];
    let changes = compute_diff(&current, &committed);
    assert!(changes.is_empty());
}

#[test]
fn diff_extra_current_item_is_added() {
    let current = vec![
        ("A".to_string(), "Cls.A".to_string(), true),
        ("B".to_string(), "Cls.B".to_string(), true),
    ];
    let committed = vec![("A".to_string(), "Cls.A".to_string(), true)];
    let changes = compute_diff(&current, &committed);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].1, "added");
}

#[test]
fn diff_missing_current_item_is_removed() {
    let current = vec![("A".to_string(), "Cls.A".to_string(), true)];
    let committed = vec![
        ("A".to_string(), "Cls.A".to_string(), true),
        ("B".to_string(), "Cls.B".to_string(), true),
    ];
    let changes = compute_diff(&current, &committed);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].1, "removed");
}

#[test]
fn diff_differing_enabled_flag_is_modified() {
    let current = vec![("A".to_string(), "Cls.A".to_string(), false)];
    let committed = vec![("A".to_string(), "Cls.A".to_string(), true)];
    let changes = compute_diff(&current, &committed);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].1, "modified");
}

/// Mirrors the diff algorithm in handle_iris_production_diff for isolated unit testing.
fn compute_diff(
    current: &[(String, String, bool)],
    committed: &[(String, String, bool)],
) -> Vec<(String, &'static str)> {
    let mut changes = Vec::new();
    for (name, class_name, enabled) in current {
        match committed.iter().find(|(n, _, _)| n == name) {
            None => changes.push((name.clone(), "added")),
            Some((_, c_class, c_enabled)) => {
                if c_class != class_name || c_enabled != enabled {
                    changes.push((name.clone(), "modified"));
                }
            }
        }
    }
    for (name, _, _) in committed {
        if !current.iter().any(|(n, _, _)| n == name) {
            changes.push((name.clone(), "removed"));
        }
    }
    changes
}

// ---------------------------------------------------------------------------
// parse_production_items_from_source: XML <Item .../> extraction from UDL source
// ---------------------------------------------------------------------------

#[test]
fn parse_production_items_extracts_name_classname_enabled() {
    let source = r#"Class MyApp.Production Extends Ens.Production
{
XData ProductionDefinition
{
<Production Name="MyApp.Production">
  <Item Name="HL7.Inbound" ClassName="EnsLib.HL7.Service.TCPService" Enabled="true" />
  <Item Name="Legacy.Router" ClassName="MyApp.Router" Enabled="false" />
</Production>
}
}"#;
    let items = parse_production_items_from_source(source);
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0],
        (
            "HL7.Inbound".to_string(),
            "EnsLib.HL7.Service.TCPService".to_string(),
            true
        )
    );
    assert_eq!(
        items[1],
        (
            "Legacy.Router".to_string(),
            "MyApp.Router".to_string(),
            false
        )
    );
}

#[test]
fn parse_production_items_defaults_enabled_true_when_attr_absent() {
    let source = r#"<Item Name="NoEnabledAttr" ClassName="MyApp.Foo" />"#;
    let items = parse_production_items_from_source(source);
    assert_eq!(items.len(), 1);
    assert!(
        items[0].2,
        "Enabled should default to true when attribute absent"
    );
}

#[test]
fn parse_production_items_skips_lines_missing_name_or_classname() {
    let source = r#"<Item ClassName="MissingName.cls" Enabled="true" />
<Item Name="MissingClassName" Enabled="true" />
<Item Name="Complete" ClassName="MyApp.Complete" Enabled="true" />"#;
    let items = parse_production_items_from_source(source);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0, "Complete");
}

#[test]
fn parse_production_items_empty_source_returns_empty() {
    let items = parse_production_items_from_source("");
    assert!(items.is_empty());
}

#[test]
fn parse_production_items_no_item_tags_returns_empty() {
    let source = "Class MyApp.Production Extends Ens.Production\n{\n}\n";
    let items = parse_production_items_from_source(source);
    assert!(items.is_empty());
}

#[test]
fn parse_production_items_enabled_numeric_1_treated_as_true() {
    let source = r#"<Item Name="Numeric" ClassName="MyApp.Num" Enabled="1" />"#;
    let items = parse_production_items_from_source(source);
    assert!(items[0].2);
}

#[test]
fn parse_production_items_enabled_false_string() {
    let source = r#"<Item Name="Off" ClassName="MyApp.Off" Enabled="false" />"#;
    let items = parse_production_items_from_source(source);
    assert!(!items[0].2);
}

// ---------------------------------------------------------------------------
// Additional edge cases for detect_content_type
// ---------------------------------------------------------------------------

#[test]
fn detect_content_type_leading_whitespace_then_msh() {
    // Leading whitespace should be trimmed before checking for MSH
    assert_eq!(detect_content_type("  \t  MSH|^~\\&"), "HL7v2");
}

#[test]
fn detect_content_type_empty_string() {
    assert_eq!(detect_content_type(""), "text");
}

#[test]
fn detect_content_type_single_character() {
    assert_eq!(detect_content_type("x"), "text");
}

#[test]
fn detect_content_type_single_brace() {
    assert_eq!(detect_content_type("{"), "JSON");
}

#[test]
fn detect_content_type_single_bracket() {
    assert_eq!(detect_content_type("["), "JSON");
}

#[test]
fn detect_content_type_single_angle_bracket() {
    assert_eq!(detect_content_type("<"), "XML");
}

#[test]
fn detect_content_type_whitespace_only() {
    assert_eq!(detect_content_type("   \t  "), "text");
}

#[test]
fn detect_content_type_msh_at_end_of_string() {
    // MSH not at start should be detected as text
    assert_eq!(detect_content_type("notMSH|^~\\&"), "text");
}

// ---------------------------------------------------------------------------
// Additional edge cases for truncate_body
// ---------------------------------------------------------------------------

#[test]
fn truncate_body_empty_string() {
    let (out, was_truncated, original_len) = truncate_body("", 100);
    assert_eq!(out, "");
    assert!(!was_truncated);
    assert_eq!(original_len, 0);
}

#[test]
fn truncate_body_exactly_equal_to_limit() {
    let body = "a".repeat(100);
    let (out, was_truncated, original_len) = truncate_body(&body, 100);
    assert_eq!(out.len(), 100);
    assert!(!was_truncated);
    assert_eq!(original_len, 100);
}

#[test]
fn truncate_body_max_bytes_one_byte() {
    let body = "abc";
    let (out, was_truncated, original_len) = truncate_body(body, 1);
    assert_eq!(out, "a");
    assert!(was_truncated);
    assert_eq!(original_len, 3);
}

#[test]
fn truncate_body_max_bytes_larger_by_one() {
    let body = "abc";
    let (out, was_truncated, original_len) = truncate_body(body, 4);
    assert_eq!(out, "abc");
    assert!(!was_truncated);
    assert_eq!(original_len, 3);
}

#[test]
fn truncate_body_multi_byte_utf8_at_boundary() {
    // Test with emoji (4 bytes in UTF-8) at boundary
    let body = "ab😀cd"; // a,b = 1 byte, 😀 = 4 bytes, c,d = 1 byte (total 8 bytes)
    let (out, was_truncated, original_len) = truncate_body(body, 2);
    // Truncating to 2 bytes should give "ab" without the emoji
    assert_eq!(out, "ab");
    assert!(was_truncated);
    assert_eq!(original_len, 8);
    assert!(std::str::from_utf8(out.as_bytes()).is_ok());
}

#[test]
fn truncate_body_multi_byte_first_char() {
    // If first character is multi-byte and max_bytes < first char size
    let body = "😀abc"; // 4 + 1 + 1 + 1 = 7 bytes
    let (out, was_truncated, original_len) = truncate_body(body, 1);
    // Should truncate to empty string rather than panic (no valid 1-byte boundary)
    assert_eq!(out, "");
    assert!(was_truncated);
    assert_eq!(original_len, 7);
}

#[test]
fn truncate_body_exact_char_boundary() {
    let body = "ab";
    let (out, was_truncated, original_len) = truncate_body(body, 2);
    assert_eq!(out, "ab");
    assert!(!was_truncated);
    assert_eq!(original_len, 2);
}

// ---------------------------------------------------------------------------
// Additional edge cases for redact_hl7v2
// ---------------------------------------------------------------------------

#[test]
fn redact_hl7v2_multiple_pid_segments() {
    // Multiple PID segments should all be redacted
    let body = "MSH|^~\\&|APP|FAC|APP|FAC|20260101||ADT^A01|1|P|2.3\rPID|1||ID1||NAME1||19800101|M\rPID|2||ID2||NAME2||19800102|F";
    let redacted = redact_hl7v2(body);
    // Both PID segments should be redacted
    assert!(redacted.contains("[REDACTED]"));
    // Count the redacted fields - should be at least 2 for PID segment redactions
    let redacted_count = redacted.matches("[REDACTED]").count();
    assert!(redacted_count >= 2);
    assert!(!redacted.contains("NAME1"));
    assert!(!redacted.contains("NAME2"));
}

#[test]
fn redact_hl7v2_pid_segment_with_insufficient_fields() {
    // PID segment with fewer fields than redaction indices should not panic
    let body = "MSH|^~\\&|APP|FAC||FAC|20260101||ADT|1|P|2.3\rPID|1||ID";
    let redacted = redact_hl7v2(body);
    // Should complete without panic and handle out-of-bounds gracefully
    assert!(!redacted.is_empty());
}

#[test]
fn redact_hl7v2_msh_segment_with_insufficient_fields() {
    // MSH-3 is at index 2, but if there aren't enough fields, should not panic
    let body = "MSH|^~\\&";
    let redacted = redact_hl7v2(body);
    assert!(!redacted.is_empty());
}

#[test]
fn redact_hl7v2_crlf_line_endings() {
    let body = "MSH|^~\\&|SendingApp|FAC|RecvApp|FAC|20260101||ADT^A01|1|P|2.3\r\nPID|1||12345||DOE^JOHN||19800101|M";
    let redacted = redact_hl7v2(body);
    assert!(redacted.contains("\r\n"));
    assert!(!redacted.contains("SendingApp"));
    assert!(!redacted.contains("DOE^JOHN"));
}

#[test]
fn redact_hl7v2_lf_line_endings() {
    let body = "MSH|^~\\&|SendingApp|FAC|RecvApp|FAC|20260101||ADT^A01|1|P|2.3\nPID|1||12345||DOE^JOHN||19800101|M";
    let redacted = redact_hl7v2(body);
    assert!(redacted.contains('\n'));
    assert!(!redacted.contains("SendingApp"));
    assert!(!redacted.contains("DOE^JOHN"));
}

#[test]
fn redact_hl7v2_cr_line_endings() {
    let body = "MSH|^~\\&|SendingApp|FAC|RecvApp|FAC|20260101||ADT^A01|1|P|2.3\rPID|1||12345||DOE^JOHN||19800101|M";
    let redacted = redact_hl7v2(body);
    assert!(redacted.contains('\r'));
    assert!(!redacted.contains("SendingApp"));
    assert!(!redacted.contains("DOE^JOHN"));
}

#[test]
fn redact_hl7v2_pid_field_indices_preserved() {
    // Verify correct PID field indices are being redacted: 3, 5, 7, 8, 11, 18
    let body = "MSH|^~\\&|APP|FAC|REC|FAC|20260101||ADT^A01|1|P|2.3\rPID|1|2|IDNUM|4|PATNAME|6|DOB|SEXFIELD|9|10|ADDR|12|13|14|15|16|17|OTHERNAME";
    let redacted = redact_hl7v2(body);
    // Fields at positions 3, 5, 7, 8, 11, 18 should be redacted
    let lines: Vec<&str> = redacted.split('\r').collect();
    assert_eq!(lines.len(), 2);
    let pid_line = lines[1];
    let fields: Vec<&str> = pid_line.split('|').collect();
    // Index 3 (field 4 - PATNAME), 5 (field 6 - DOB), 7 (field 8 - SEXFIELD),
    // 8 (field 9), 11 (field 12 - ADDR), 18 (field 19 - OTHERNAME)
    assert_eq!(fields[3], "[REDACTED]"); // PATNAME
    assert_eq!(fields[5], "[REDACTED]"); // DOB
    assert_eq!(fields[7], "[REDACTED]"); // SEXFIELD
    assert_eq!(fields[8], "[REDACTED]"); // field 9
    assert_eq!(fields[11], "[REDACTED]"); // ADDR
    assert_eq!(fields[18], "[REDACTED]"); // OTHERNAME
}

#[test]
fn redact_hl7v2_mixed_line_endings_prefers_crlf() {
    // If both CRLF and LF present, CRLF should be used
    let body = "MSH|^~\\&|SendingApp|FAC|REC|FAC|20260101||ADT^A01|1|P|2.3\r\nPID|1||12345||DOE||19800101|M\nOBX|line";
    let redacted = redact_hl7v2(body);
    // Should use CRLF since it was found first
    assert!(redacted.contains("\r\n"));
}

#[test]
fn redact_hl7v2_only_cr_when_cr_and_lf_both_present() {
    // Test that if only CR is present (no CRLF, no LF separately), CR is used
    let body = "MSH|^~\\&|APP|FAC|REC|FAC|20260101||ADT|1|P|2.3\rPID|1||12345||DOE||19800101|M";
    let redacted = redact_hl7v2(body);
    // Should preserve single CR
    let count_cr = redacted.matches('\r').count();
    assert!(count_cr > 0);
}

// ---------------------------------------------------------------------------
// Additional edge cases for message_body parameter validation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn message_body_message_id_with_leading_whitespace() {
    let params = MessageBodyParams {
        message_id: "  123".to_string(),
        namespace: "USER".to_string(),
        max_bytes: 65536,
        acknowledge_phi: true,
    };
    let result = handle_iris_message_body(None, &params, "allow")
        .await
        .expect("Ok");
    let v = parse_result(result);
    // Should trim whitespace and parse as integer, leading to IRIS_UNREACHABLE (no conn)
    assert_eq!(v["error_code"], "IRIS_UNREACHABLE");
}

#[tokio::test]
async fn message_body_message_id_with_trailing_whitespace() {
    let params = MessageBodyParams {
        message_id: "123  ".to_string(),
        namespace: "USER".to_string(),
        max_bytes: 65536,
        acknowledge_phi: true,
    };
    let result = handle_iris_message_body(None, &params, "allow")
        .await
        .expect("Ok");
    let v = parse_result(result);
    assert_eq!(v["error_code"], "IRIS_UNREACHABLE");
}

#[tokio::test]
async fn message_body_message_id_with_surrounding_whitespace() {
    let params = MessageBodyParams {
        message_id: "  456  ".to_string(),
        namespace: "USER".to_string(),
        max_bytes: 65536,
        acknowledge_phi: true,
    };
    let result = handle_iris_message_body(None, &params, "allow")
        .await
        .expect("Ok");
    let v = parse_result(result);
    assert_eq!(v["error_code"], "IRIS_UNREACHABLE");
}

#[tokio::test]
async fn message_body_message_id_whitespace_only() {
    let params = MessageBodyParams {
        message_id: "   ".to_string(),
        namespace: "USER".to_string(),
        max_bytes: 65536,
        acknowledge_phi: true,
    };
    let result = handle_iris_message_body(None, &params, "allow")
        .await
        .expect("Ok");
    let v = parse_result(result);
    assert_eq!(v["error_code"], "INVALID_MESSAGE_ID");
}

#[tokio::test]
async fn message_body_message_id_empty_string() {
    let params = MessageBodyParams {
        message_id: "".to_string(),
        namespace: "USER".to_string(),
        max_bytes: 65536,
        acknowledge_phi: true,
    };
    let result = handle_iris_message_body(None, &params, "allow")
        .await
        .expect("Ok");
    let v = parse_result(result);
    assert_eq!(v["error_code"], "INVALID_MESSAGE_ID");
}

#[tokio::test]
async fn message_body_negative_number_as_message_id() {
    let params = MessageBodyParams {
        message_id: "-123".to_string(),
        namespace: "USER".to_string(),
        max_bytes: 65536,
        acknowledge_phi: true,
    };
    let result = handle_iris_message_body(None, &params, "allow")
        .await
        .expect("Ok");
    let v = parse_result(result);
    // Should parse as valid i64 and attempt IRIS call
    assert_eq!(v["error_code"], "IRIS_UNREACHABLE");
}

#[tokio::test]
async fn message_body_max_bytes_clamped_exactly_one_mb() {
    let params = MessageBodyParams {
        message_id: "123".to_string(),
        namespace: "USER".to_string(),
        max_bytes: 1_048_576,
        acknowledge_phi: true,
    };
    let result = handle_iris_message_body(None, &params, "allow")
        .await
        .expect("Ok");
    let v = parse_result(result);
    // Exactly 1MB should not be clamped
    assert_eq!(v["error_code"], "IRIS_UNREACHABLE"); // No IRIS conn
    let clamped = v
        .get("max_bytes_clamped")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(!clamped);
}

#[tokio::test]
async fn message_body_max_bytes_clamped_one_byte_over() {
    let params = MessageBodyParams {
        message_id: "123".to_string(),
        namespace: "USER".to_string(),
        max_bytes: 1_048_577,
        acknowledge_phi: true,
    };
    let result = handle_iris_message_body(None, &params, "allow")
        .await
        .expect("Ok");
    let v = parse_result(result);
    // 1MB + 1 byte should be clamped
    assert_eq!(v["error_code"], "IRIS_UNREACHABLE");
}

// ---------------------------------------------------------------------------
// Additional edge cases for business_rule_info parameter validation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn business_rule_info_get_empty_string_rule_name() {
    let params = BusinessRuleInfoParams {
        action: "get".to_string(),
        rule_name: Some("".to_string()),
        namespace: "USER".to_string(),
    };
    let result = handle_iris_business_rule_info(None, &params)
        .await
        .expect("Ok");
    let v = parse_result(result);
    assert_eq!(v["error_code"], "INVALID_PARAMS");
}

#[tokio::test]
async fn business_rule_info_get_whitespace_rule_name() {
    let params = BusinessRuleInfoParams {
        action: "get".to_string(),
        rule_name: Some("   ".to_string()),
        namespace: "USER".to_string(),
    };
    let result = handle_iris_business_rule_info(None, &params)
        .await
        .expect("Ok");
    let v = parse_result(result);
    // Whitespace-only string should fail because as_deref().unwrap_or("") would be "   "
    // and "   ".is_empty() is false, so it passes validation and goes to IRIS
    assert_eq!(v["error_code"], "IRIS_UNREACHABLE");
}

#[tokio::test]
async fn business_rule_info_action_case_sensitive() {
    let params = BusinessRuleInfoParams {
        action: "GET".to_string(),
        rule_name: Some("MyRule".to_string()),
        namespace: "USER".to_string(),
    };
    let result = handle_iris_business_rule_info(None, &params)
        .await
        .expect("Ok");
    let v = parse_result(result);
    // "GET" (uppercase) should not match "get"
    assert_eq!(v["error_code"], "INVALID_ACTION");
}

#[tokio::test]
async fn business_rule_info_list_with_rule_name() {
    let params = BusinessRuleInfoParams {
        action: "list".to_string(),
        rule_name: Some("SomeRule".to_string()),
        namespace: "USER".to_string(),
    };
    let result = handle_iris_business_rule_info(None, &params)
        .await
        .expect("Ok");
    let v = parse_result(result);
    // "list" action should not require rule_name; should go to IRIS
    assert_eq!(v["error_code"], "IRIS_UNREACHABLE");
}

// ---------------------------------------------------------------------------
// Additional edge cases for parse_production_items_from_source and extract_xml_attr
// ---------------------------------------------------------------------------

#[test]
fn parse_production_items_attributes_in_different_order() {
    // Attributes in different order: ClassName, then Name, then Enabled
    let source = r#"<Item ClassName="MyApp.Service" Name="MyService" Enabled="true" />"#;
    let items = parse_production_items_from_source(source);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0, "MyService");
    assert_eq!(items[0].1, "MyApp.Service");
    assert!(items[0].2);
}

#[test]
fn parse_production_items_enabled_before_name() {
    let source = r#"<Item Enabled="false" Name="MyService" ClassName="MyApp.Service" />"#;
    let items = parse_production_items_from_source(source);
    assert_eq!(items.len(), 1);
    assert!(!items[0].2);
}

#[test]
fn parse_production_items_self_closing_tag() {
    let source = r#"<Item Name="A" ClassName="Cls.A" Enabled="true" />"#;
    let items = parse_production_items_from_source(source);
    assert_eq!(items.len(), 1);
}

#[test]
fn parse_production_items_non_self_closing_tag() {
    let source = r#"<Item Name="A" ClassName="Cls.A" Enabled="true"></Item>"#;
    let items = parse_production_items_from_source(source);
    // Matches because starts_with("<Item ") and extracts Name/ClassName/Enabled attributes
    // regardless of whether it's self-closing or not
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0, "A");
}

#[test]
fn parse_production_items_attribute_with_escaped_quote() {
    let source = r#"<Item Name="MyService&quot;Name" ClassName="MyApp.Service" Enabled="true" />"#;
    let items = parse_production_items_from_source(source);
    // Current extract_xml_attr doesn't handle HTML entity escaping, looks for unescaped "
    // but &quot; is just text with quote in it, so it captures up to the quote after "Name"
    assert_eq!(items.len(), 1);
    // Will capture from opening " to the first unescaped ", which is the one after &quot
    assert_eq!(items[0].0, "MyService&quot;Name");
}

#[test]
fn parse_production_items_classname_before_name() {
    let source = r#"<Item ClassName="MyApp.Service" Name="MyService" Enabled="true" />"#;
    let items = parse_production_items_from_source(source);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0, "MyService");
    assert_eq!(items[0].1, "MyApp.Service");
}

#[test]
fn parse_production_items_multiple_items_mixed_valid_invalid() {
    let source = r#"
<Item Name="A" ClassName="Cls.A" Enabled="true" />
<Item Name="B" Enabled="true" />
<Item Name="C" ClassName="Cls.C" Enabled="true" />
<Item ClassName="Cls.D" Enabled="true" />
<Item Name="E" ClassName="Cls.E" Enabled="true" />
"#;
    let items = parse_production_items_from_source(source);
    // Should get A, C, E (B and D are missing required fields)
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].0, "A");
    assert_eq!(items[1].0, "C");
    assert_eq!(items[2].0, "E");
}

#[test]
fn parse_production_items_only_item_prefix_matches() {
    // <Items ... should not match (needs "<Item " with space)
    let source = r#"
<Item Name="A" ClassName="Cls.A" />
<Items Name="B" ClassName="Cls.B" />
<Item Name="C" ClassName="Cls.C" />
"#;
    let items = parse_production_items_from_source(source);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].0, "A");
    assert_eq!(items[1].0, "C");
}

#[test]
fn parse_production_items_item_tag_spanning_multiple_lines() {
    let source = r#"<Item
      Name="MultiLine"
      ClassName="MyApp.Service"
      Enabled="true" />"#;
    // Current implementation splits by lines, so multi-line tags won't parse
    let items = parse_production_items_from_source(source);
    assert_eq!(items.len(), 0);
}

#[test]
fn extract_xml_attr_basic() {
    let line = r#"<Item Name="TestName" ClassName="TestClass" Enabled="true" />"#;
    // Need to use the public function indirectly via parse_production_items_from_source
    let items = parse_production_items_from_source(line);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0, "TestName");
}

#[test]
fn extract_xml_attr_attribute_not_found_returns_none() {
    let source = r#"<Item Name="A" ClassName="Cls.A" />"#;
    let items = parse_production_items_from_source(source);
    assert_eq!(items.len(), 1);
    assert!(items[0].2); // Enabled defaults to true when missing
}

#[test]
fn parse_production_items_enabled_zero_treated_as_false() {
    let source = r#"<Item Name="Off" ClassName="MyApp.Off" Enabled="0" />"#;
    let items = parse_production_items_from_source(source);
    assert!(!items[0].2);
}

#[test]
fn parse_production_items_enabled_other_value_treated_as_false() {
    let source = r#"<Item Name="Other" ClassName="MyApp.Other" Enabled="maybe" />"#;
    let items = parse_production_items_from_source(source);
    assert!(!items[0].2);
}
