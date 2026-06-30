# Feature Specification: iris_global Tool

**Feature Branch**: `052-iris-global`
**Created**: 2026-06-29
**Status**: Implemented
**Depends on**: 051 (PHI policy and env gates — merged to master)

## Overview

iris-agentic-dev has no MCP tool for reading or writing IRIS globals. Globals are the
fundamental storage primitive in IRIS — nearly all production IRIS systems store critical
data in globals, including HL7 message bodies, scheduling data, clinical records, and
application state. AI agents working with IRIS need the ability to inspect and manipulate
globals to debug issues, investigate data, and perform guided operations.

This feature adds the `iris_global` MCP tool with four operations: `get` (read a node or
subtree), `set` (write a single node), `kill` (delete a node or subtree), and `list` (list
subscripts at a given node level). All operations pass through `dispatch_gate()` (spec 051)
which enforces the environment template gate, PHI data policy, system blocklist, and PHI
name pattern gate before any IRIS call is made.

**This is the tool that spec 051 was built to protect.**

---

## Clarifications

### Session 2026-06-29

- Q: How should subscript values be sanitized before ObjectScript interpolation? → A: Allowlist — subscripts must match `^[a-zA-Z0-9 _.:\-]+$`; reject with `INVALID_SUBSCRIPT` error before any IRIS call. No escape-and-proceed fallback.
- Q: How should subtree traversal be bounded beyond node count? → A: Count cap (`max_nodes` default 100, max 1000) plus 5-second wall-clock timeout; return `truncated: true` on either limit.
- Q: How should IRIS-side execute errors be reported? → A: Reuse existing codes — HTTP 5xx → `IRIS_UNREACHABLE`; ObjectScript `CATCH` output → `IRIS_EXECUTE_ERROR` with `message` field. Consistent with `iris_execute` tool.

---

## User Scenarios & Testing

### User Story 1 — Read a global node or subtree (Priority: P1)

A developer debugging an IRIS application wants to inspect the value of a global node or
browse a subtree to understand data structure.

**Independent Test**: Call `iris_global` with `action=get`, `global_name="MyApp.Config"`,
`subscripts=["DatabaseVersion"]`. Expect the node value or "undefined" if not set. Call with
no subscripts to get the top-level value.

**Acceptance Scenarios**:

1. **Given** `action=get` and a valid global name, **When** the node exists, **Then** the
   response includes `value` with the node's string value.
2. **Given** `action=get` and a node that does not exist, **Then** the response includes
   `value: null` and `defined: false` (not an error).
3. **Given** `action=get` with `subtree: true`, **Then** the response includes a `nodes`
   array of `{path, value}` objects for all descendant nodes, capped at `max_nodes`
   (default 100). `path` is the full global reference string as returned by `$Query`
   (e.g. `^MyApp("a","b")`).
4. **Given** `global_name` matches `SYSTEM_BLOCKLIST`, **Then** `SYSTEM_BLOCKLIST` error
   is returned before any IRIS call.
5. **Given** `global_name` matches a PHI pattern and no `acknowledgePhi`, **Then**
   `PHI_GATE_BLOCKED` error is returned.
6. **Given** `global_name` matches a PHI pattern and `acknowledgePhi: true`, **Then** the
   get proceeds normally.

---

### User Story 2 — Set a global node (Priority: P1)

A developer wants to set a specific global node value during debugging or test setup.

**Independent Test**: Call `iris_global` with `action=set`, `global_name="IrisDevTest.Temp"`,
`subscripts=["testKey"]`, `value="hello"`. Then `action=get` to verify the value was written.

**Acceptance Scenarios**:

1. **Given** `action=set`, a valid global name, and a `value`, **When** the operation
   succeeds, **Then** `success: true` is returned.
2. **Given** `action=set` on a `live` template connection, **Then** `ENV_GATE_BLOCKED`
   is returned (set is a write operation).
3. **Given** `action=set` with no `value` param, **Then** a structured error is returned
   explaining the missing parameter.

---

### User Story 3 — Kill a global node or subtree (Priority: P2)

A developer wants to delete a global node or entire subtree for cleanup or test teardown.

**Independent Test**: Call `iris_global` with `action=kill`, `global_name="IrisDevTest.Temp"`.
All nodes under that global are deleted.

**Acceptance Scenarios**:

1. **Given** `action=kill` and a valid non-blocklisted global, **Then** the node (and
   subtree if present) is deleted and `success: true` returned.
2. **Given** `action=kill` on a `live` template, **Then** `ENV_GATE_BLOCKED` (kill is
   a write operation).
3. **Given** `action=kill` on a system-blocklisted global, **Then** `SYSTEM_BLOCKLIST`
   error — unless the global is in `dataPolicyKillAllowlist`, in which case it proceeds.
4. **Given** `action=kill` on a global that doesn't exist, **Then** `success: true` (kill
   of non-existent node is a no-op in ObjectScript, not an error).

---

### User Story 4 — List subscripts at a node level (Priority: P2)

A developer wants to enumerate what subscripts exist under a global node without reading
all values — useful for navigating large global trees.

**Independent Test**: Call `iris_global` with `action=list`, `global_name="MyApp.Data"`.
Expect an array of first-level subscripts.

**Acceptance Scenarios**:

1. **Given** `action=list` and a valid global, **Then** the response includes `subscripts`
   array of string subscript values at that level, up to `max_subscripts` (default 50).
2. **Given** the node has no subscripts, **Then** `subscripts: []` (not an error).
3. **Given** `action=list` on a system-blocklisted global, **Then** `SYSTEM_BLOCKLIST`
   error returned.

---

### Edge Cases

- **Leading `^` optional**: `global_name = "MyApp"` and `global_name = "^MyApp"` are both
  valid — strip leading `^` for Atelier calls, normalize internally.
- **Subscripts as array or dot-path**: Accept subscripts as a JSON array `["a", "b"]` — do
  not support dot-path strings (too ambiguous for numeric subscripts).
- **Numeric subscripts**: Pass as strings in the array; IRIS coerces `"1"` to the numeric
  subscript correctly.
- **Subscript validation**: Each subscript must match `^[a-zA-Z0-9 _.:\-]+$`. Subscripts
  containing `"`, `^`, `)`, `(`, or other special characters are rejected with
  `INVALID_SUBSCRIPT` before any IRIS call — no escape-and-proceed fallback.
- **Empty subscript array**: Refers to the root node of the global (e.g., `^MyApp` itself).
- **Subtree cap**: Large globals can have millions of nodes. `max_nodes` default 100, max
  allowed 1000. A 5-second wall-clock traversal timeout also applies. Return
  `truncated: true` when either limit is hit; include `node_count` in the response.
- **Namespace**: Defaults to the connection's default namespace. Explicit `namespace` param
  overrides.
- **Non-string values**: IRIS globals store strings. Numbers stored as strings; binary data
  is base64-encoded in the response.
- **IRIS execute failures**: HTTP 5xx → `IRIS_UNREACHABLE`. ObjectScript runtime error
  (e.g., `<PROTECT>` on a protected global, `<SUBSCRIPT>` on bad subscript type) →
  `IRIS_EXECUTE_ERROR` with `message` from IRIS. Never surface raw stack traces.

---

## Requirements

### Functional Requirements

- **FR-001**: `iris_global` MUST support four `action` values: `get`, `set`, `kill`, `list`.
- **FR-002**: All four actions MUST pass through `dispatch_gate()` before any IRIS call,
  passing `global_name` and `action` as params so gate [3] (system blocklist) and gate [4]
  (PHI name gate) fire correctly.
- **FR-003**: `get` with `subtree: false` (default) returns the single node value.
  `get` with `subtree: true` returns all descendant nodes up to `max_nodes` (default 100,
  max 1000) OR a 5-second wall-clock traversal timeout, whichever triggers first. Either
  limit sets `truncated: true` in the response.
- **FR-004**: `set` requires a `value` string param; sets the node at the specified
  `global_name` + `subscripts` path.
- **FR-005**: `kill` deletes the node and all descendants at `global_name` + `subscripts`.
  Kill of a non-existent node is success.
- **FR-006**: `list` returns an array of subscripts at the specified node level, up to
  `max_subscripts` (default 50, max 500).
- **FR-007**: `global_name` MUST accept names with or without a leading `^`; strip `^`
  internally before calling IRIS.
- **FR-008**: `subscripts` param is an optional JSON array of strings. Absent or empty
  means the root global node. Each subscript string MUST match `^[a-zA-Z0-9 _.:\-]+$`;
  any subscript failing this check MUST be rejected with error code `INVALID_SUBSCRIPT`
  before any IRIS call is made.
- **FR-009**: `namespace` param is optional; defaults to the connection's active namespace.
- **FR-010**: `acknowledgePhi: true` param enables the PHI bypass per spec 051 gate [4].
- **FR-011**: When `subtree: true` returns a truncated result, the response MUST include
  `truncated: true` and the count of returned nodes.
- **FR-012**: Tool MUST be registered in `registered_tool_names()` in `Toolset::Merged`
  tier. For env-gate classification: `get` and `list` actions map to `ToolCategory::Query`
  (permitted by all templates); `set` and `kill` actions map to `ToolCategory::Execute`
  (blocked by live and test templates). No new `ToolCategory` variant is needed.
- **FR-013**: `check_config` response MUST list `iris_global` in the tool inventory.
- **FR-014**: HTTP 5xx from the IRIS execute endpoint MUST return `IRIS_UNREACHABLE`. An
  ObjectScript `CATCH` block triggered during the operation MUST return `IRIS_EXECUTE_ERROR`
  with a `message` field containing the IRIS error string (e.g., `<UNDEFINED>` or
  `<PROTECT>`). No new error codes are introduced for IRIS-side failures.

### Implementation Approach

Implement via IRIS `%Object.Execute` with inline ObjectScript:

```objectscript
// get single node
Set val = $Get(^GlobalName(sub1, sub2))
Set defined = $Data(^GlobalName(sub1, sub2)) > 0

// subtree get (using $Query)
Set ref = $Name(^GlobalName(sub1))
Set node = ref
For {
    Set node = $Query(@node)
    Quit:node=""
    Quit:$Extract(node,1,$Length(ref))'=ref
    // collect node and value
}

// set
Set ^GlobalName(sub1) = value

// kill
Kill ^GlobalName(sub1)

// list subscripts
Set sub = ""
For {
    Set sub = $Order(^GlobalName(sub))
    Quit:sub=""
    // collect sub
}
```

All executed via `iris_execute` HTTP endpoint (same path as `iris_execute` tool).

---

## Success Criteria

- **SC-001**: `iris_global get` on an existing node returns the correct value in < 500ms.
- **SC-002**: `iris_global get` with `subtree: true` on a 100-node tree returns all 100
  nodes correctly.
- **SC-003**: `iris_global set` followed by `iris_global get` round-trips a value correctly.
- **SC-004**: `iris_global kill` removes all descendant nodes.
- **SC-005**: All four actions on a PHI-named global without `acknowledgePhi` return
  `PHI_GATE_BLOCKED`.
- **SC-006**: All four actions on a system-blocklisted global return `SYSTEM_BLOCKLIST`.
- **SC-007**: `iris_global` with `mcpTemplate=live` returns `ENV_GATE_BLOCKED` for set and
  kill; permits get and list.

---

## Assumptions

- IRIS execute endpoint (`/api/atelier/v1/{namespace}/action/execute`) is used for all
  four operations — same transport as `iris_execute` tool.
- Global values are always strings in IRIS; no special binary handling needed for P1/P2.
- Subscripts are always strings in the JSON API; numeric coercion is IRIS's responsibility.
- The `Global` tool category is a new category to be added to `env_gate.rs`; `get` and
  `list` are read-only (not blocked by live template); `set` and `kill` are write operations
  (blocked by live template).
- `iris_global` does not need a separate `iris_global.rs` handler file — it can live in
  `tools/global.rs` as a new module.
