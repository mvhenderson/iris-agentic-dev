# Requirements Checklist: 056-interop-depth

**Spec**: specs/056-interop-depth/spec.md
**Branch**: 056-interop-depth

## Functional Requirements

### iris_message_body

- [x] FR-001: Accept message_id, namespace, max_bytes, acknowledgePhi params
- [x] FR-002: Retrieve body via Ens.MessageBody, handling stream containers
- [x] FR-003: Truncate at max_bytes, set truncated:true and actual_size
- [x] FR-004: Clamp max_bytes to 1MB maximum
- [x] FR-005: Return PHI_POLICY_BLOCKED when dataPolicy=block
- [x] FR-006: Perform HL7 v2 field redaction when dataPolicy=redact
- [x] FR-007: Require acknowledgePhi:true when dataPolicy=allow
- [x] FR-008: Return MESSAGE_NOT_FOUND for unknown message IDs
- [x] FR-009: Pass through dispatch_gate() as ToolCategory::Query

### iris_business_rule_info

- [x] FR-010: Accept action (list|get), rule_name, namespace params
- [x] FR-011: list action queries EnsLib.Rules.Definition, returns {name,class_name,description,modified}
- [x] FR-012: get action returns conditions and actions for named rule set
- [x] FR-013: Return RULE_NOT_FOUND for unknown rule_name
- [x] FR-014: Return INTEROP_NOT_AVAILABLE if EnsLib.Rules.Definition not in namespace
- [x] FR-015: Pass through dispatch_gate() as ToolCategory::Query

### iris_production_diff

- [x] FR-016: Accept production (optional) and namespace params
- [x] FR-017: Compare current production config against last SCM version
- [x] FR-018: Return changes array of {item_name, item_type, status} objects
- [x] FR-019: Set in_sync:true and changes:[] when no drift detected
- [x] FR-020: Return NO_SCM when source control not configured
- [x] FR-021: Return NO_SCM_VERSION when SCM configured but no committed version
- [x] FR-022: Return PRODUCTION_NOT_FOUND for unknown production name
- [x] FR-023: Pass through dispatch_gate() as ToolCategory::Query

## Success Criteria

- [x] SC-001: message_body returns correct HL7 body within 2 seconds
- [x] SC-002: dataPolicy=block returns PHI_POLICY_BLOCKED without IRIS data read
- [x] SC-003: dataPolicy=redact redacts PID-5 (patient name) to [REDACTED]
- [x] SC-004: 500KB stream with default max_bytes returns 65536 bytes with truncated:true
- [x] SC-005: business_rule_info list returns non-empty rules on production with Ensemble
- [x] SC-006: business_rule_info get returns non-empty conditions or actions
- [x] SC-007: business_rule_info get for non-existent rule returns RULE_NOT_FOUND
- [x] SC-008: production_diff on unmodified production returns in_sync:true, changes:[]
- [x] SC-009: production_diff after one host change returns exactly one modified entry
- [x] SC-010: All three tools classified as ToolCategory::Query — not blocked by live template
- [x] SC-011: All three tools in registered_tool_names() and Merged toolset tier
- [x] SC-012: Zero regressions on existing interop tool tests

## Clarifications Applied

- [x] CL-001: max_bytes default 64KB, configurable up to 1MB via max_bytes param
- [x] CL-002: dataPolicy=block hard-blocks message_body; redact applies HL7 v2 field table; allow requires acknowledgePhi:true
- [x] CL-003: production_diff compares against SCM only (v1); NO_SCM if not configured
