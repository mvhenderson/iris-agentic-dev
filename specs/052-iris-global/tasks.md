# Tasks: iris_global Tool

**Input**: Design documents from `/specs/052-iris-global/`
**Prerequisites**: plan.md ✓, spec.md ✓ (clarified 2026-06-29)

**Organization**: Tasks grouped by user story. US1 = get (P1), US2 = set (P1),
US3 = kill (P2), US4 = list (P2). Phase 2 foundational wiring blocks all US phases.

## Format: `[ID] [P?] [Story] Description`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: New handler file, module registration, tool category classification.

**⚠️ CRITICAL**: All user story phases depend on these — complete before any US work.

- [X] T001 Create `crates/iris-agentic-dev-core/src/tools/global.rs` — empty module with
      `pub async fn handle_iris_global(...)` stub that returns `not_implemented` JSON
- [X] T002 Add `pub mod global;` to `crates/iris-agentic-dev-core/src/tools/mod.rs` and
      route `"iris_global"` tool name to `global::handle_iris_global` in the tool dispatch
      match arm
- [X] T003 Add `"iris_global"` to `registered_tool_names()` in
      `crates/iris-agentic-dev-core/src/tools/mod.rs`
- [X] T004 Add `iris_global` → `ToolCategory::Query` default mapping in `tool_to_category()`
      in `crates/iris-agentic-dev-core/src/iris/server_manager.rs` (that is where
      `tool_to_category_pub` / `tool_to_category` live)
- [X] T005 Add MCP tool schema for `iris_global` in
      `crates/iris-agentic-dev-core/src/tools/mod.rs` (or wherever tool schemas are declared) —
      params: `action` (enum: get/set/kill/list, required), `global_name` (string, required),
      `subscripts` (array of strings, optional), `value` (string, optional), `namespace`
      (string, optional), `subtree` (bool, optional, default false), `max_nodes` (integer,
      optional, default 100), `max_subscripts` (integer, optional, default 50),
      `acknowledgePhi` (bool, optional, default false)
- [X] T006 Add `iris_global` to the `Toolset::Merged` tier in
      `crates/iris-agentic-dev-core/src/tools/mod.rs` — add to both the
      `with_registry_and_toolset()` Merged removal list AND confirm it is in
      `registered_tool_names()` (T003); these two lists must stay in sync per constitution
- [X] T007b Run `cargo build -p iris-agentic-dev-core` — confirm clean compile with stub

**Checkpoint**: `iris_global` registered in Merged tier, routes to stub, compiles clean.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: `dispatch_gate()` env-gate action-aware classification for `iris_global`;
shared param-parsing helpers; subscript validation.

**⚠️ CRITICAL**: Gate wiring must be correct before any IRIS calls can be tested.

- [X] T007 Update `check_env_gate()` signature in
      `crates/iris-agentic-dev-core/src/policy/env_gate.rs` to accept an additional
      `params: &serde_json::Value` argument. After the `tool_to_category_pub()` call returns
      a category, add: if `tool_name == "iris_global"` AND `params["action"].as_str()` is
      `"set"` or `"kill"`, override `category` to `ToolCategory::Execute`. Update all
      existing call sites of `check_env_gate()` (currently only in `gate.rs`) to pass the
      real `params` or `&serde_json::Value::Null` for non-`iris_global` paths.
- [X] T008 Update the `check_env_gate(...)` call site inside `dispatch_gate()` in
      `crates/iris-agentic-dev-core/src/policy/gate.rs` to forward the existing `params`
      argument (already present in `dispatch_gate` signature from spec 051). No signature
      change to `dispatch_gate` itself is needed — only the internal call site changes.
- [X] T009 Implement `validate_subscripts(subscripts: &[String]) -> Result<(), serde_json::Value>`
      in `crates/iris-agentic-dev-core/src/tools/global.rs` — checks each subscript against
      regex `^[a-zA-Z0-9 _.:\-]+$`; returns `INVALID_SUBSCRIPT` error JSON on first failure
- [X] T010 Implement `normalize_global_name(name: &str) -> String` in
      `crates/iris-agentic-dev-core/src/tools/global.rs` — strips leading `^`, returns bare
      name
- [X] T011 Implement `build_global_ref(name: &str, subscripts: &[String]) -> String` in
      `crates/iris-agentic-dev-core/src/tools/global.rs` — produces `^Name("sub1","sub2")`
      string for interpolation into ObjectScript (subscripts already validated)
- [X] T012 Run `cargo test -p iris-agentic-dev-core` — confirm all pre-existing tests still
      pass after signature change to `check_env_gate`

**Checkpoint**: Gate wires correctly, subscript validation exists, global ref builder ready.

---

## Phase 3: User Story 1 — get (Priority: P1) 🎯 MVP

**Goal**: Read a single global node value or a bounded subtree.

**Independent Test**: Call `iris_global` with `action=get`, `global_name="IrisDevTest.Tmp"`,
`subscripts=["k1"]` on a live IRIS. Verify value returned or `defined: false`.

### Tests for US1

> Write FIRST. Must FAIL before T020.

- [X] T013 [US1] Create `crates/iris-agentic-dev-core/tests/unit/test_iris_global_unit.rs` —
      test `normalize_global_name`: `"^MyApp"` → `"MyApp"`, `"MyApp"` → `"MyApp"`,
      `"^%SYS"` → `"%SYS"`
- [X] T014 [P] [US1] Add unit test to `test_iris_global_unit.rs` — `validate_subscripts`
      passes `["a", "b_1", "hello world"]`; fails on `["bad\"sub"]`, `["^inject"]`,
      `["a)b"]`; returns `INVALID_SUBSCRIPT` error code
- [X] T015 [P] [US1] Add unit test to `test_iris_global_unit.rs` — `build_global_ref`
      with no subscripts → `^MyApp`; with `["a","b"]` → `^MyApp("a","b")`
- [X] T016 [P] [US1] Add unit test to `test_iris_global_unit.rs` — `action=get` with
      missing `global_name` param → structured error with `error_code` field (not panic)
- [X] T017 [P] [US1] Add unit test to `test_iris_global_unit.rs` — `action=get` on
      `mcpTemplate=live` policy → `ENV_GATE_BLOCKED` (get is Query, permitted by live)
- [X] T018 [P] [US1] Add unit test to `test_iris_global_unit.rs` — `action=get` with
      invalid subscript `["bad\"char"]` → `INVALID_SUBSCRIPT` before gate fires
- [X] T019 [US1] Create `crates/iris-agentic-dev-core/tests/integration/test_iris_global_live.rs`
      — `#[ignore]`; round-trip: set node via `iris_execute` directly, then `iris_global get`,
      assert value matches; verify `defined: true`; verify `defined: false` for absent node

### Implementation for US1

- [X] T020 [US1] Implement `get` action in `handle_iris_global` in
      `crates/iris-agentic-dev-core/src/tools/global.rs`:
  - Parse `global_name`, `subscripts` (default `[]`), `namespace`, `subtree` (default
    false), `max_nodes` (default 100, clamp to 1000)
  - Call `validate_subscripts` and `normalize_global_name`
  - Call `dispatch_gate()` with `action=get` and `global_name` in params
  - Build ObjectScript: single-node uses `$Get` + `$Data`; subtree uses `$Query` loop
    with 5-second timeout guard and `max_nodes` cap
  - POST to IRIS execute endpoint, parse JSON response
  - Return `{success: true, value, defined}` (single) or
    `{success: true, nodes: [...], truncated, node_count}` (subtree)
- [X] T021 [US1] Add 5-second subtree traversal timeout to the ObjectScript generated for
      `subtree: true` — use `$ZH` (horolog time) as a wall-clock check inside the `$Query`
      loop; set `truncated=true` and break when `$ZH - startTime > 5`
- [X] T022 [US1] Run `cargo test -p iris-agentic-dev-core test_iris_global` — all US1 unit
      tests must pass

**Checkpoint**: US1 complete. `iris_global get` returns value/defined/subtree correctly.
`INVALID_SUBSCRIPT` fires before any IRIS call.

---

## Phase 4: User Story 2 — set (Priority: P1)

**Goal**: Write a value to a single global node.

**Independent Test**: Call `iris_global` with `action=set`, `global_name="IrisDevTest.Tmp"`,
`subscripts=["k1"]`, `value="hello"`. Then get to verify.

### Tests for US2

> Write FIRST. Must FAIL before T027.

- [X] T023 [P] [US2] Add unit test to `test_iris_global_unit.rs` — `action=set` missing
      `value` param → structured error (not `INVALID_SUBSCRIPT`, not panic)
- [X] T024 [P] [US2] Add unit test to `test_iris_global_unit.rs` — `action=set` on
      `mcpTemplate=live` policy → `ENV_GATE_BLOCKED` (set is Execute, blocked by live)
- [X] T025 [P] [US2] Add unit test to `test_iris_global_unit.rs` — `action=set` on
      `mcpTemplate=test` policy → `ENV_GATE_BLOCKED` (Execute blocked on test too)
- [X] T026 [US2] Add integration test to `test_iris_global_live.rs` — `#[ignore]`;
      set `IrisDevTest.GlobSet("u1")` = `"val42"`, get same path, assert `value == "val42"`,
      kill to clean up

### Implementation for US2

- [X] T027 [US2] Implement `set` action in `handle_iris_global` in
      `crates/iris-agentic-dev-core/src/tools/global.rs`:
  - Parse `global_name`, `subscripts`, `value` (required — return error if absent),
    `namespace`
  - Call `validate_subscripts`, `normalize_global_name`, `dispatch_gate(action=set)`
  - Build ObjectScript: `Set ^Name(subs) = "value"`
  - Return `{success: true}`
- [X] T028 [US2] Run `cargo test -p iris-agentic-dev-core test_iris_global` — all US1+US2
      unit tests pass

**Checkpoint**: US2 complete. `iris_global set` writes node; `ENV_GATE_BLOCKED` on live/test.

---

## Phase 5: User Story 3 — kill (Priority: P2)

**Goal**: Delete a global node and all its descendants.

**Independent Test**: Write nodes under `IrisDevTest.GlobKill`, call `iris_global kill`,
verify nodes are gone via `get`.

### Tests for US3

> Write FIRST. Must FAIL before T033.

- [X] T029 [P] [US3] Add unit test to `test_iris_global_unit.rs` — `action=kill` on
      `mcpTemplate=live` → `ENV_GATE_BLOCKED`
- [X] T030 [P] [US3] Add unit test to `test_iris_global_unit.rs` — `action=kill` on
      system-blocklisted global `%SYS` (with `dataPolicy=allow`) → `SYSTEM_BLOCKLIST`
- [X] T031 [P] [US3] Add unit test to `test_iris_global_unit.rs` — `action=kill` on
      non-existent global returns `{success: true}` (no-op, not an error)
- [X] T032 [US3] Add integration test to `test_iris_global_live.rs` — `#[ignore]`;
      set three nodes under `IrisDevTest.GlobKill`, kill root, verify all three are
      `defined: false` via get

### Implementation for US3

- [X] T033 [US3] Implement `kill` action in `handle_iris_global` in
      `crates/iris-agentic-dev-core/src/tools/global.rs`:
  - Parse `global_name`, `subscripts`, `namespace`
  - Call `validate_subscripts`, `normalize_global_name`, `dispatch_gate(action=kill)`
  - Build ObjectScript: `Kill ^Name(subs)` (Kill of non-existent is no-op in ObjectScript)
  - Return `{success: true}`
- [X] T034 [US3] Run `cargo test -p iris-agentic-dev-core test_iris_global` — all US1–US3
      unit tests pass

**Checkpoint**: US3 complete. `kill` removes node/subtree; kill of absent node succeeds.

---

## Phase 6: User Story 4 — list (Priority: P2)

**Goal**: Enumerate subscripts at a global node level without reading values.

**Independent Test**: Write nodes under `IrisDevTest.GlobList`, call `iris_global list`,
verify subscript array matches what was written.

### Tests for US4

> Write FIRST. Must FAIL before T039.

- [X] T035 [P] [US4] Add unit test to `test_iris_global_unit.rs` — `action=list` on
      node with no subscripts → `{success: true, subscripts: [], truncated: false}`
- [X] T036 [P] [US4] Add unit test to `test_iris_global_unit.rs` — `action=list` on
      system-blocklisted global `^oddDEF` → `SYSTEM_BLOCKLIST`
- [X] T037 [P] [US4] Add unit test to `test_iris_global_unit.rs` — `action=list` on
      `mcpTemplate=live` → permitted (list is Query category)
- [X] T038 [US4] Add integration test to `test_iris_global_live.rs` — `#[ignore]`;
      set 5 nodes under `IrisDevTest.GlobList`, list root, assert `subscripts` length == 5,
      kill to clean up; verify `max_subscripts=2` returns 2 with `truncated: true`

### Implementation for US4

- [X] T039 [US4] Implement `list` action in `handle_iris_global` in
      `crates/iris-agentic-dev-core/src/tools/global.rs`:
  - Parse `global_name`, `subscripts`, `namespace`, `max_subscripts` (default 50,
    clamp to 500)
  - Call `validate_subscripts`, `normalize_global_name`, `dispatch_gate(action=list)`
  - Build ObjectScript: `$Order` loop collecting subscripts up to `max_subscripts`
  - Return `{success: true, subscripts: [...], truncated}`
- [X] T040 [US4] Run `cargo test -p iris-agentic-dev-core test_iris_global` — all US1–US4
      unit tests pass

**Checkpoint**: US4 complete. `list` returns subscript array with truncation support.

---

## Phase 6b: Coverage Gap Tests

**Purpose**: Unit tests for logic paths not covered by US1–US4 test tasks.

- [X] T040b [P] Add unit test to `test_iris_global_unit.rs` — `build_set_objectscript`
      helper (or inline generated code for `set`) produces exactly `Set ^MyApp("a","b") = "hello"`
      given `name="MyApp"`, `subscripts=["a","b"]`, `value="hello"`; confirms ObjectScript
      string is correctly formed before any IRIS call
- [X] T040c [P] Add unit test to `test_iris_global_unit.rs` — `IRIS_EXECUTE_ERROR` parsing:
      when the output string from `execute_via_generator` starts with `"ERROR: "`, the handler
      returns `{success: false, error_code: "IRIS_EXECUTE_ERROR", message: "<PROTECT>..."}`
      (test by passing mock output string directly to the output-parsing function)
- [X] T040d [P] Add unit tests to `test_iris_global_unit.rs` — clamp behavior:
      `max_nodes=9999` is treated as `1000`; `max_nodes=0` is treated as `1` (not zero);
      `max_subscripts=9999` is treated as `500`

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: PHI/blocklist integration tests, `check_config` inventory, AGENTS.md update,
final test run and fmt/clippy pass.

- [X] T041 Add PHI gate integration test to `test_iris_global_live.rs` — `#[ignore]`;
      call `iris_global get` with `global_name="PAPMI"` and no `acknowledgePhi` →
      `PHI_GATE_BLOCKED`; repeat with `acknowledgePhi: true` → attempt proceeds (may fail
      with `IRIS_UNREACHABLE` if global absent, but gate must not block)
- [X] T042 [P] Add system blocklist integration test to `test_iris_global_live.rs` —
      `#[ignore]`; call `iris_global get` with `global_name="%SYS"` and `dataPolicy=allow`
      → `SYSTEM_BLOCKLIST`
- [X] T043 [P] Verify `iris_global` appears in `check_config` tool inventory response —
      add assertion to existing `test_server_manager.rs` that `registered_tool_names()`
      contains `"iris_global"` in
      `crates/iris-agentic-dev-core/tests/unit/test_server_manager.rs`
- [X] T044 [P] Update `light-skills/AGENTS.md` — add `iris_global` to the MCP tool
      reference section (Section 2, "Via MCP tools") with usage example showing all four
      actions and the `acknowledgePhi` parameter
- [X] T045 Run full test suite: `cargo test -p iris-agentic-dev-core` — all non-ignored
      tests pass, zero regressions
- [X] T046 Run `cargo fmt --all -- --check` — no formatting diff
- [X] T047 Run `cargo clippy -p iris-agentic-dev-core -- -D warnings` — zero warnings
- [X] T048 [P] Update spec status to `Status: Implemented` in
      `specs/052-iris-global/spec.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 (needs `global.rs` to exist)
- **Phase 3 (US1 get)**: Depends on Phase 2 (needs validated gate + helpers)
- **Phase 4 (US2 set)**: Depends on Phase 2; can run in parallel with Phase 3
- **Phase 5 (US3 kill)**: Depends on Phase 2; can run after Phase 3 (reuses unit test file)
- **Phase 6 (US4 list)**: Depends on Phase 2; can run after Phase 3 (reuses unit test file)
- **Phase 7 (Polish)**: Depends on all US phases complete

### User Story Dependencies

- **US1 (get)**: Foundational complete
- **US2 (set)**: Foundational complete; independent of US1
- **US3 (kill)**: Foundational complete; independent of US1/US2
- **US4 (list)**: Foundational complete; independent of US1/US2/US3

### Within Each Phase

- Tests written FIRST, must FAIL before implementation
- `validate_subscripts` + `normalize_global_name` + `build_global_ref` (Phase 2) must
  exist before any action implementation
- Action-aware env-gate (T007–T008) must compile before gate tests run

### Parallel Opportunities

- T013–T018 (US1 unit tests) — T014–T018 parallel after T013 creates the file
- T023–T025 (US2 unit tests) — all parallel (appending to existing file)
- T029–T031 (US3 unit tests) — all parallel
- T035–T037 (US4 unit tests) — all parallel
- T041–T044 (Polish) — T042–T044 parallel after T041 creates integration test entries

---

## Parallel Example: Phase 3 (US1)

```
# Write tests first (T013 creates file, T014–T018 parallel):
T013 → [T014, T015, T016, T017, T018 in parallel] → T019

# Then implement (T020–T021 sequential, T022 validates):
T020 → T021 → T022
```

---

## Implementation Strategy

### MVP First (US1 + US2 only — the two P1 stories)

1. Complete Phase 1: Setup (T001–T006)
2. Complete Phase 2: Foundational (T007–T012)
3. Complete Phase 3: US1 get (T013–T022)
4. Complete Phase 4: US2 set (T023–T028)
5. **STOP and VALIDATE**: `cargo test test_iris_global` green; round-trip set→get works live
6. Ship — read and write are the highest-value operations

### Incremental Delivery

1. Setup + Foundational → registered tool, gate wired, helpers ready
2. US1 get → single-node reads and subtree browse
3. US2 set → node writes
4. US3 kill → node/subtree deletion
5. US4 list → subscript enumeration
6. Polish → PHI/blocklist integration coverage, docs

---

## Notes

- `dispatch_gate()` signature change (adding `params` to `check_env_gate`) may require
  updating all existing call sites in `gate.rs` — verify with `cargo build` after T008
- The 5-second subtree timeout uses IRIS `$ZH` (seconds since midnight) — safe for
  short traversals; does not handle midnight rollover for traversals that span midnight
  (acceptable for this use case)
- `INVALID_SUBSCRIPT` is a new error code — add to the error code registry comment in
  `gate.rs` and to `light-skills/AGENTS.md` Section 6 during Polish phase
- All integration tests use `IrisDevTest.*` globals to avoid polluting production namespaces
