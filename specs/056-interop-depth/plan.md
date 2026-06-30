# Implementation Plan: Interoperability Depth Tools

**Branch**: `056-interop-depth`
**Spec**: spec.md
**Depends on**: 051 (dispatch_gate, DataPolicy, PHI gates — merged), 017 (iris_production,
iris_interop_query — merged)

## Tech Stack

- Rust 1.92 (workspace edition 2021)
- `iris-agentic-dev-core` crate — all three tool handlers live here
- `serde_json` — param parsing, response building
- `reqwest` — HTTP calls to IRIS execute endpoint (same as iris_execute)
- ObjectScript execution via `IrisConnection::execute_via_generator()` (compile-and-query
  pattern)
- Policy gate: `dispatch_gate()` from `crates/iris-agentic-dev-core/src/policy/gate.rs`
- All three tools extend `crates/iris-agentic-dev-core/src/tools/interop.rs`

## File Structure

```text
crates/iris-agentic-dev-core/src/tools/interop.rs           # MODIFY — add three new handlers
crates/iris-agentic-dev-core/src/tools/mod.rs                # MODIFY — register tools + schemas
crates/iris-agentic-dev-core/src/policy/env_gate.rs          # VERIFY — Query category covers all three
tests/unit/test_interop_depth_unit.rs                        # NEW — unit tests
tests/integration/test_interop_depth_live.rs                 # NEW — integration tests (#[ignore])
```

## Tool Category Classification

All three tools are read-only (`Query` category) — no changes to `tool_to_category()` or
`check_env_gate()` are needed for category overrides (unlike spec 052 which needed
action-aware `Execute` gating for `iris_global set/kill`).

Add to `tool_to_category()` in `server_manager.rs`:

```text
"iris_message_body"       => ToolCategory::Query
"iris_business_rule_info" => ToolCategory::Query
"iris_production_diff"    => ToolCategory::Query
```

## ObjectScript Execution Strategy

### iris_message_body

Two-phase query: first fetch body class/type from `Ens.MessageHeader`; then fetch the
body content, handling stream vs. non-stream:

```objectscript
Set header = ##class(Ens.MessageHeader).%OpenId(msgId)
If '$IsObject(header) { Write "{""error"":""MESSAGE_NOT_FOUND""}", ! Quit }
Set body = header.MessageBody
If '$IsObject(body) { Write "{""error"":""MESSAGE_NOT_FOUND""}", ! Quit }
// Check if it's a stream container
If body.%IsA("Ens.StreamContainer") || body.%IsA("%Stream.Object") {
    // read stream content up to maxBytes
} Else {
    // use $Get on the body property or serialize to string
}
```

**PHI policy enforcement**: Enforced in Rust before any ObjectScript is generated —
if `dataPolicy == "block"`, return `PHI_POLICY_BLOCKED` immediately. HL7 v2 redaction
(for `dataPolicy == "redact"`) applied to the returned string in Rust after IRIS returns
the body, using the same regex table from spec 051 audit log scrubbing.

**Stream reading**: Use `%ReadLine(maxBytes)` loop accumulating into a string buffer in
ObjectScript; track byte count; set `truncated` flag when limit is reached.

### iris_business_rule_info

**list action**: SQL query against `EnsLib_Rules.Definition` table:

```sql
SELECT Name, Description, TimeModified
FROM EnsLib_Rules.Definition
ORDER BY Name
```

**get action**: Open the rule definition by class name, serialize conditions and actions
by querying `EnsLib_Rules.RuleDefinition` extent or via `%XML.Writer` to get the rule XML,
then parse in Rust.

### iris_production_diff

Query the current production config via `Ens.Config.Production` SQL extent, then compare
against the SCM-stored version. Use the same `%Studio.SourceControl` interface used by
`iris_source_control` to retrieve the last-committed version.

Diff algorithm (in Rust): build two sets of `{item_name, item_type}` from current and
SCM version; classify each item as added/removed/modified by comparing properties.

## Response Format

### iris_message_body

```json
{
  "success": true,
  "message_id": "12345",
  "content_type": "HL7v2",
  "body": "MSH|^~\\&|...",
  "truncated": false,
  "actual_size": 1024
}
```

Error:

```json
{"success": false, "error_code": "MESSAGE_NOT_FOUND", "message": "No body for ID 12345"}
{"success": false, "error_code": "PHI_POLICY_BLOCKED", "message": "..."}
```

### iris_business_rule_info

```json
// list
{
  "success": true,
  "rules": [
    {"name": "MyApp.RoutingRule", "class_name": "MyApp.RoutingRule", "description": "...", "modified": "2026-01-15 10:00:00"}
  ]
}

// get
{
  "success": true,
  "name": "MyApp.RoutingRule",
  "description": "Route HL7 ADT messages",
  "conditions": [{"when": "msgType = \"ADT\"", "constraint": "...", "actions": [...]}],
  "actions": [{"send_to": "HL7.Processor", "transform": "..."}]
}
```

### iris_production_diff

```json
{
  "success": true,
  "in_sync": false,
  "changes": [
    {"item_name": "HL7.Inbound", "item_type": "BusinessService", "status": "modified"},
    {"item_name": "Legacy.Router", "item_type": "BusinessProcess", "status": "removed"}
  ]
}
```

## Test Strategy

### Unit tests (no IRIS needed)

- Param validation: missing `message_id` → error; `max_bytes=0` → clamped to 1
- PHI gate: `dataPolicy=block` → `PHI_POLICY_BLOCKED` before any IRIS call (mock gate)
- PHI ack: `dataPolicy=allow`, no `acknowledgePhi` → `PHI_ACK_REQUIRED`
- HL7 redaction: sample HL7 string with PID-5 → PID-5 replaced with `[REDACTED]`
- Stream truncation logic: content longer than max_bytes → truncated response shape
- `business_rule_info` missing `rule_name` on `action=get` → structured error
- `production_diff` NO_SCM error shape
- Query category: all three tools NOT blocked by `mcpTemplate=live` gate (mock gate)

### Integration tests (#[ignore])

- `iris_message_body`: fetch body for a known test message ID; verify content_type detected
- `iris_business_rule_info list`: non-empty on any production namespace
- `iris_business_rule_info get`: returns rule structure for a known rule
- `iris_production_diff`: in_sync on fresh production; detects one change after mutation

## Toolset Registration

All three tools use `execute_via_generator` (HTTP-only). They belong in **`Toolset::Merged`**
per the constitution. Both of the following must be updated in sync:

1. `registered_tool_names()` — add all three tool names
2. `with_registry_and_toolset()` router — add all three to the `Merged` toolset removal
   list

---

## Constitution Check

| Principle                      | Status  | Notes                                                                                                        |
| ------------------------------ | ------- | ------------------------------------------------------------------------------------------------------------ |
| I. Zero-Install Binary         | ✅ Pass | Uses `execute_via_generator` (HTTP); no new install step                                                     |
| II. ObjectScript Sanity        | ✅ Pass | `%OpenId`, `%IsA`, `%ReadLine`, SQL on `EnsLib_Rules.Definition` — all standard IRIS APIs                   |
| III. HTTP-First                | ✅ Pass | `execute_via_generator` is HTTP-only; no `IRIS_CONTAINER` required                                          |
| IV. Test-First, Fixture-Driven | ✅ Pass | Unit tests precede implementation in all phases; integration tests in `tests/integration/`                  |
| V. Output Shape Parity         | ✅ Pass | All three response shapes documented above; new error codes registered                                      |
| VI. Environment Guard          | ✅ Pass | All three classified `Query`; pass `dispatch_gate()` before any IRIS call                                   |
| VII. Dependency Minimalism     | ✅ Pass | No new crates; `serde_json`, `reqwest`, `regex` (already in workspace for spec 051 audit log scrubbing)     |

---

## Phase Structure

1. **Setup**: Register three tool stubs + tool schemas in `mod.rs`; add `tool_to_category`
   entries; compile clean
2. **Foundational**: Shared helpers in `interop.rs` — HL7 redaction fn, stream truncation
   helper, `detect_content_type` fn
3. **US1 (message_body)**: Unit tests → implementation
4. **US2 (business_rule_info)**: Unit tests → implementation
5. **US3 (production_diff)**: Unit tests → implementation
6. **Polish**: Integration tests, AGENTS.md update, `check_config` inventory, fmt/clippy
