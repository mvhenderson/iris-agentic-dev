# Feature Specification: Interoperability Depth Tools

**Feature Branch**: `056-interop-depth`
**Created**: 2026-06-29
**Status**: Draft
**Depends on**: 051 (PHI policy and env gates — merged to master)

## Overview

iris-agentic-dev's existing interoperability tools (`iris_production`, `iris_interop_query`,
`iris_production_item`) cover production lifecycle and message header search, but leave three
common debugging and operational workflows unsupported:

1. **Message body retrieval** — agents can find a message by ID via `iris_interop_query` but
   cannot read its body. HL7 v2, FHIR, XML, and custom message bodies are where the real
   diagnostic content lives.
2. **Business rule inspection** — `EnsLib.Rules.Definition` rules govern routing, transformation,
   and decision logic in Ensemble productions. There is no tool to list or inspect them.
3. **Production diff** — teams running source-controlled productions want to know whether the
   running configuration matches what is saved in source control, without manually diffing
   exports.

This feature adds three new read-only tools that extend the interop surface: `iris_message_body`,
`iris_business_rule_info`, and `iris_production_diff`. All three are Query-category operations
and pass through `dispatch_gate()` (spec 051).

**Primary drivers:**

- **HL7/FHIR debugging**: Message body content is essential for diagnosing interoperability
  pipeline failures. Without it, agents must ask operators to fetch content manually.
- **Rule governance**: Business rules encode clinical and operational logic. Inspecting them
  is necessary for change-impact analysis and debugging unexpected routing behavior.
- **Drift detection**: Uncontrolled changes to a running production are a patient-safety risk
  in healthcare environments. A diff tool surfaces drift immediately.

---

## Clarifications

### Session 2026-06-29

- Q: Message bodies can be arbitrarily large (HL7 batch files, large XML). What size limit applies? → A: Default max_size=64KB, configurable up to 1MB via max_bytes param. Beyond the limit, return truncated content with truncated:true and actual_size in bytes. Consistent with stream_inspect pattern described in the roadmap.
- Q: HL7 and FHIR message bodies contain PHI. Does dataPolicy=block prevent message_body entirely, or only redact fields? → A: dataPolicy=block: message_body returns PHI_POLICY_BLOCKED error regardless of message content — we cannot know in advance whether a message contains PHI. dataPolicy=redact: attempt HL7 v2 field redaction (same table as spec 051 audit log scrubbing). dataPolicy=allow: full content returned. The caller must set acknowledgePhi:true when dataPolicy=allow to confirm awareness.
- Q: What is the reference for production_diff — source control, another namespace, or a saved snapshot? → A: Source control only for v1 — compare current in-memory production config against the last source-controlled version (same mechanism as iris_source_control status check). Cross-namespace diff is deferred to a future spec. If no SCM is configured, return NO_SCM error.

---

## User Scenarios & Testing

### User Story 1 — Read an Interoperability Message Body (Priority: P1)

A developer debugging an HL7 integration failure has found the failing message header ID
via `iris_interop_query`. They want to read the actual message body — the HL7 v2 segment
content, FHIR resource, or custom XML — to understand why it failed.

**Why this priority**: Message body content is the primary diagnostic artifact in
interoperability debugging. Agents that can search for messages but not read them require
the operator to manually fetch content, defeating the purpose of the tool.

**Independent Test**: Call `iris_message_body` with a known message ID on a running IRIS
instance that has Ensemble configured. Expect the body content string (HL7 MSH segment
visible for HL7 messages). Call with an unknown ID and expect `MESSAGE_NOT_FOUND`.

**Acceptance Scenarios**:

1. **Given** a valid message ID for a plain-text body, **When** `iris_message_body` is
   called, **Then** the response includes `body` (string content) and `content_type`
   (e.g. `HL7v2`, `XML`, `JSON`, `text`).
2. **Given** a message body stored as a stream object (`Ens.StreamContainer` or
   `%Stream.Object`), **When** `iris_message_body` is called, **Then** the stream is read
   up to `max_bytes` (default 65536) and returned as `body`.
3. **Given** a message body exceeding `max_bytes`, **Then** the response includes
   `truncated: true` and `actual_size` in bytes alongside the truncated `body`.
4. **Given** `dataPolicy = "block"`, **When** `iris_message_body` is called, **Then**
   `PHI_POLICY_BLOCKED` is returned regardless of message content — no body data is read.
5. **Given** `dataPolicy = "allow"` and `acknowledgePhi: true`, **When** called, **Then**
   full body content is returned.
6. **Given** `dataPolicy = "redact"`, **When** called with an HL7 v2 message, **Then**
   PHI fields (PID-3, PID-5, PID-7, PID-8, PID-11, PID-18, MSH-3) are replaced with
   `[REDACTED]` before the response.
7. **Given** a message ID that does not exist, **Then** `MESSAGE_NOT_FOUND` error is
   returned (not an IRIS exception).
8. **Given** `max_bytes` exceeds 1MB (1048576), **Then** the param is clamped to 1MB
   and a `max_bytes_clamped: true` field is added to the response.

---

### User Story 2 — Browse and Inspect Business Rules (Priority: P2)

A developer troubleshooting unexpected message routing wants to list the business rules
defined in a production and inspect the conditions and actions of a specific rule set to
understand why a message was routed to the wrong target.

**Why this priority**: Business rules are opaque without tooling — they exist as
`EnsLib.Rules.Definition` class instances that are not accessible via existing tools.
Routing bugs are among the most common interoperability issues.

**Independent Test**: Call `iris_business_rule_info` with `action=list` on a namespace
with a running production. Expect a list of rule set names. Call with `action=get` and
one of those names to get rule conditions and actions.

**Acceptance Scenarios**:

1. **Given** `action=list` and a namespace with business rules, **When** called, **Then**
   the response includes `rules` array of `{name, class_name, description, modified}` objects.
2. **Given** `action=list` on a namespace with no business rules, **Then** `rules` is an
   empty array (not an error).
3. **Given** `action=get` with a valid `rule_name`, **When** called, **Then** the response
   includes `name`, `description`, `conditions` (array), and `actions` (array) parsed from
   the rule definition.
4. **Given** `action=get` with a `rule_name` that does not exist, **Then**
   `RULE_NOT_FOUND` error is returned.
5. **Given** any call, **When** `dataPolicy = "block"`, **Then** the call is NOT blocked
   — business rule definitions do not contain PHI.
6. **Given** `mcpTemplate = "live"`, **When** called, **Then** the call succeeds —
   business rule info is Query category and permitted on live instances.

---

### User Story 3 — Diff Running Production Against Source Control (Priority: P2)

A DevOps engineer wants to verify that the running production configuration exactly
matches the version saved in source control before approving a change window. They want
a list of items that have drifted — modified, added, or removed since the last SCM commit.

**Why this priority**: Configuration drift is a patient-safety risk in healthcare IRIS
deployments. Detecting it before a change window is standard practice. The alternative
(manual export and diff) is slow and error-prone.

**Independent Test**: On a system with SCM configured, call `iris_production_diff` with
a production name. If no drift, expect `changes: []`. Manually modify a business host
(e.g., toggle `Enabled`), call again, expect the modified item in `changes`.

**Acceptance Scenarios**:

1. **Given** a production with no drift from SCM, **When** `iris_production_diff` is
   called, **Then** `changes` is an empty array and `in_sync: true`.
2. **Given** a production where one business host was modified since last SCM commit,
   **Then** `changes` includes that item with `status: "modified"` and `item_name`.
3. **Given** a production where a business host was added (not in SCM), **Then** `changes`
   includes that item with `status: "added"`.
4. **Given** a production where a business host was removed from the running config but
   exists in SCM, **Then** `changes` includes that item with `status: "removed"`.
5. **Given** no SCM is configured in the namespace, **Then** `NO_SCM` error is returned.
6. **Given** `mcpTemplate = "live"`, **When** called, **Then** the call succeeds — diff
   is Query category.
7. **Given** a production name that does not exist, **Then** `PRODUCTION_NOT_FOUND` error
   is returned.

---

### Edge Cases

- What happens when a stream container message is corrupt or the stream object is not readable?
  Return `STREAM_READ_ERROR` with the ObjectScript exception message; do not return partial content.
- How does `iris_message_body` handle binary (non-text) stream content?
  Return `content_type: "binary"` and base64-encode the body (up to max_bytes). Indicate
  `encoding: "base64"` in the response.
- What if `iris_business_rule_info` is called on a namespace that does not have Ensemble/Interop
  installed? Return `INTEROP_NOT_AVAILABLE` error (same as existing interop tools).
- What if `iris_production_diff` is called when SCM is configured but no version of the
  production is yet committed? Return `NO_SCM_VERSION` distinct from `NO_SCM`.
- What if `max_bytes=0` is passed to `iris_message_body`? Clamp to 1 byte minimum; return
  the first byte with `truncated: true`. Do not error.
- What if a business rule's definition class is missing from the namespace? Return partial
  info with a `warning` field noting the missing class, rather than failing entirely.

---

## Requirements

### Functional Requirements

#### iris\_message\_body

- **FR-001**: `iris_message_body` MUST accept `message_id` (string, required), `namespace`
  (string, optional, default `USER`), `max_bytes` (integer, optional, default 65536),
  and `acknowledgePhi` (bool, optional, default false).
- **FR-002**: `iris_message_body` MUST retrieve the body via `Ens.MessageBody` class,
  handling both direct string storage and stream containers (`Ens.StreamContainer`,
  `%Stream.Object`) by reading the stream content.
- **FR-003**: `iris_message_body` MUST truncate content at `max_bytes` and set
  `truncated: true` with `actual_size` when the body exceeds the limit.
- **FR-004**: `iris_message_body` MUST clamp `max_bytes` to a maximum of 1048576 (1MB).
- **FR-005**: `iris_message_body` MUST return `PHI_POLICY_BLOCKED` when `dataPolicy = "block"`,
  regardless of message content.
- **FR-006**: `iris_message_body` MUST perform HL7 v2 field redaction when `dataPolicy = "redact"`,
  replacing PHI fields (PID-3, PID-5, PID-7, PID-8, PID-11, PID-18, MSH-3) with `[REDACTED]`.
- **FR-007**: `iris_message_body` MUST require `acknowledgePhi: true` when `dataPolicy = "allow"`,
  returning `PHI_ACK_REQUIRED` error if absent.
- **FR-008**: `iris_message_body` MUST return `MESSAGE_NOT_FOUND` when the given ID has no
  corresponding body record (not an IRIS ObjectScript exception).
- **FR-009**: `iris_message_body` MUST pass through `dispatch_gate()` before any IRIS call,
  with `ToolCategory::Query` classification.

#### iris\_business\_rule\_info

- **FR-010**: `iris_business_rule_info` MUST accept `action` (enum: `list` | `get`, required),
  `rule_name` (string, required for `get`), and `namespace` (string, optional, default `USER`).
- **FR-011**: `iris_business_rule_info` with `action=list` MUST query `EnsLib.Rules.Definition`
  and return all rule sets as `{name, class_name, description, modified}` objects.
- **FR-012**: `iris_business_rule_info` with `action=get` MUST return rule conditions and
  actions parsed from the `EnsLib.Rules.Definition` class for the named rule set.
- **FR-013**: `iris_business_rule_info` MUST return `RULE_NOT_FOUND` when `rule_name` does
  not match any `EnsLib.Rules.Definition` in the namespace.
- **FR-014**: `iris_business_rule_info` MUST return `INTEROP_NOT_AVAILABLE` if
  `EnsLib.Rules.Definition` does not exist in the namespace.
- **FR-015**: `iris_business_rule_info` MUST pass through `dispatch_gate()` with
  `ToolCategory::Query` classification.

#### iris\_production\_diff

- **FR-016**: `iris_production_diff` MUST accept `production` (string, optional — defaults
  to the running production), and `namespace` (string, optional, default `USER`).
- **FR-017**: `iris_production_diff` MUST compare the current in-memory production config
  against the last source-controlled version using the same SCM query mechanism as
  `iris_source_control`.
- **FR-018**: `iris_production_diff` MUST return a `changes` array of
  `{item_name, item_type, status}` objects where `status` is `"modified"`, `"added"`, or
  `"removed"`.
- **FR-019**: `iris_production_diff` MUST set `in_sync: true` and `changes: []` when no
  drift is detected.
- **FR-020**: `iris_production_diff` MUST return `NO_SCM` when no source control is
  configured in the namespace.
- **FR-021**: `iris_production_diff` MUST return `NO_SCM_VERSION` when SCM is configured
  but no committed version of the production exists yet.
- **FR-022**: `iris_production_diff` MUST return `PRODUCTION_NOT_FOUND` when the named
  production does not exist.
- **FR-023**: `iris_production_diff` MUST pass through `dispatch_gate()` with
  `ToolCategory::Query` classification.

### Key Entities

- **MessageBody**: An Ensemble/Interoperability message body record — may be stored as a
  plain string value on the `EnsLib.MessageBody` extent, or as a stream reference in
  `Ens.StreamContainer`. Identified by integer message ID.
- **BusinessRule**: An `EnsLib.Rules.Definition` instance — contains a rule name, a
  description, and a set of conditions (when clauses) and actions (then clauses). Named
  by the ObjectScript class name of the rule definition.
- **ProductionItem**: A single business host entry in a production configuration — has an
  item name, class name, category (BusinessService/Process/Operation), and enabled flag.
- **SCMVersion**: The source-controlled snapshot of a production configuration, as
  retrievable via the existing SCM mechanism used by `iris_source_control`.

---

## Success Criteria

- **SC-001**: `iris_message_body` returns the correct body string for a known HL7 v2
  message on a live IRIS instance within 2 seconds.
- **SC-002**: `iris_message_body` with `dataPolicy = "block"` returns `PHI_POLICY_BLOCKED`
  without making any IRIS data read call — verified by unit test with mocked connection.
- **SC-003**: `iris_message_body` with `dataPolicy = "redact"` and an HL7 v2 message
  containing real PID segments returns a response where PID-5 (patient name) is `[REDACTED]`.
- **SC-004**: `iris_message_body` for a 500KB stream body with default `max_bytes=65536`
  returns exactly 65536 bytes with `truncated: true` and `actual_size >= 512000`.
- **SC-005**: `iris_business_rule_info` with `action=list` returns a non-empty `rules`
  array on any IRIS instance with at least one Ensemble production configured.
- **SC-006**: `iris_business_rule_info` with `action=get` and a valid rule name returns
  a response with non-empty `conditions` or `actions` arrays for any non-trivial rule.
- **SC-007**: `iris_business_rule_info` called with `rule_name` for a non-existent rule
  returns `RULE_NOT_FOUND` error — not an ObjectScript exception.
- **SC-008**: `iris_production_diff` on a freshly-deployed production with no changes
  returns `in_sync: true` and `changes: []`.
- **SC-009**: `iris_production_diff` after toggling `Enabled` on one business host returns
  exactly one entry in `changes` with `status: "modified"`.
- **SC-010**: All three tools are classified as `ToolCategory::Query` — confirmed by unit
  test asserting `dispatch_gate()` does NOT block them under `mcpTemplate = "live"`.
- **SC-011**: All three tools appear in `registered_tool_names()` and in the `Merged`
  toolset tier.
- **SC-012**: Zero regressions on existing interop tool tests after implementation.

---

## Assumptions

1. Ensemble/Interoperability is installed in the target namespace. All three tools return
   `INTEROP_NOT_AVAILABLE` when it is not — same pattern as existing interop tools.
2. Message body IDs are integers (or strings that parse as integers). Non-integer IDs
   return `INVALID_MESSAGE_ID` before any IRIS call.
3. The SCM mechanism available via `iris_source_control` is sufficient to retrieve the
   committed production config for diff. No additional SCM API surface is needed.
4. Business rule definitions are stored as `EnsLib.Rules.Definition` subclass instances —
   the standard Ensemble rules engine. Custom rule engines not built on this class are
   out of scope.
5. HL7 v2 redaction applies only to standard segment field positions (PID, MSH). FHIR
   JSON redaction (e.g., removing `Patient.name`) is deferred to a future spec.
6. `iris_production_diff` v1 does not surface configuration property diffs within a
   modified item — only the item name and status. Full property-level diff is a P3
   enhancement.
7. The three new tools are added to `interop.rs` (extending the existing module) rather
   than creating separate files, consistent with the existing interop tool grouping.
