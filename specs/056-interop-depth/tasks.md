# Tasks: Interoperability Depth Tools

**Input**: Design documents from `/specs/056-interop-depth/`
**Prerequisites**: plan.md ✓, spec.md ✓ (clarified 2026-06-29)

**Organization**: Tasks grouped by user story. US1 = iris_message_body (P1),
US2 = iris_business_rule_info (P2), US3 = iris_production_diff (P2).
Phase 2 foundational helpers block all US phases.

## Format: `[ID] [P?] [Story] Description`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Register three tool stubs, add to toolset, tool schemas, compile clean.

**⚠️ CRITICAL**: All user story phases depend on these — complete before any US work.

- [ ] T001 Add `pub async fn handle_iris_message_body(...)` stub in
      `crates/iris-agentic-dev-core/src/tools/interop.rs` — returns `not_implemented` JSON
- [ ] T002 Add `pub async fn handle_iris_business_rule_info(...)` stub in
      `crates/iris-agentic-dev-core/src/tools/interop.rs` — returns `not_implemented` JSON
- [ ] T003 Add `pub async fn handle_iris_production_diff(...)` stub in
      `crates/iris-agentic-dev-core/src/tools/interop.rs` — returns `not_implemented` JSON
- [ ] T004 Route `"iris_message_body"`, `"iris_business_rule_info"`, `"iris_production_diff"`
      in the tool dispatch match arm in `crates/iris-agentic-dev-core/src/tools/mod.rs`
- [ ] T005 Add all three tool names to `registered_tool_names()` in
      `crates/iris-agentic-dev-core/src/tools/mod.rs`
- [ ] T006 Add `"iris_message_body"`, `"iris_business_rule_info"`, `"iris_production_diff"`
      → `ToolCategory::Query` in `tool_to_category()` in
      `crates/iris-agentic-dev-core/src/iris/server_manager.rs`
- [ ] T007 Add MCP tool schemas for all three tools in `mod.rs`:
      - `iris_message_body`: `message_id` (string, required), `namespace` (string, optional),
        `max_bytes` (integer, optional, default 65536), `acknowledgePhi` (bool, optional)
      - `iris_business_rule_info`: `action` (enum: list|get, required), `rule_name` (string,
        required for get), `namespace` (string, optional)
      - `iris_production_diff`: `production` (string, optional), `namespace` (string, optional)
- [ ] T008 Add all three tools to the `Toolset::Merged` tier in `mod.rs` — both
      `with_registry_and_toolset()` Merged removal list AND `registered_tool_names()` must
      stay in sync
- [ ] T009 Run `cargo build -p iris-agentic-dev-core` — confirm clean compile with stubs

**Checkpoint**: All three tools registered in Merged tier, route to stubs, compile clean.

---

## Phase 2: Foundational (Shared Helpers)

**Purpose**: Shared helper functions used by multiple tools — HL7 redaction, stream
truncation shape, content type detection.

**⚠️ CRITICAL**: US1 phase depends on these helpers.

- [ ] T010 Implement `redact_hl7v2(body: &str) -> String` in `interop.rs` — replaces
      PHI fields (PID-3, PID-5, PID-7, PID-8, PID-11, PID-18, MSH-3) with `[REDACTED]`
      using the same segment/field parsing as spec 051 audit log scrubbing. Returns input
      unchanged for non-HL7 content.
- [ ] T011 Implement `detect_content_type(body: &str) -> &'static str` in `interop.rs` —
      returns `"HL7v2"` if body starts with `"MSH|"`, `"JSON"` if starts with `{` or `[`,
      `"XML"` if starts with `<`, `"binary"` if non-UTF8, else `"text"`.
- [ ] T012 Implement `truncate_body(body: &str, max_bytes: usize) -> (String, bool, usize)`
      in `interop.rs` — returns `(truncated_content, was_truncated, original_byte_len)`.
      Truncates at UTF-8 char boundary at or before `max_bytes`.
- [ ] T013 Run `cargo test -p iris-agentic-dev-core` — confirm all pre-existing tests still
      pass

**Checkpoint**: Shared helpers exist and compile. HL7 redaction, truncation, content type
detection ready for US1.

---

## Phase 3: User Story 1 — iris_message_body (Priority: P1) 🎯 MVP

**Goal**: Read the body content of a message by ID, with PHI policy enforcement.

**Independent Test**: Call `iris_message_body` with a valid message ID on a live IRIS
instance. Verify HL7 content returned. Call with an unknown ID — verify `MESSAGE_NOT_FOUND`.

### Tests for US1

> Write FIRST. Must FAIL before T025.

- [ ] T014 [US1] Create `crates/iris-agentic-dev-core/tests/unit/test_interop_depth_unit.rs`
      — test `redact_hl7v2`: PID segment with PID-5 set → PID-5 replaced with `[REDACTED]`;
      non-HL7 string → returned unchanged
- [ ] T015 [P] [US1] Add unit test to `test_interop_depth_unit.rs` — `detect_content_type`:
      `"MSH|^~\\&|..."` → `"HL7v2"`; `"<PRPA>"` → `"XML"`; `"{\"key\":1}"` → `"JSON"`;
      `"plain text"` → `"text"`
- [ ] T016 [P] [US1] Add unit test to `test_interop_depth_unit.rs` — `truncate_body` with
      100-byte string and `max_bytes=50` → returns 50-byte string with `was_truncated=true`
      and `original_byte_len=100`; with `max_bytes=200` → returns full string with
      `was_truncated=false`
- [ ] T017 [P] [US1] Add unit test to `test_interop_depth_unit.rs` — `iris_message_body`
      with missing `message_id` param → structured error with `error_code` field (not panic)
- [ ] T018 [P] [US1] Add unit test to `test_interop_depth_unit.rs` — `iris_message_body`
      with `dataPolicy=block` (mocked gate) → returns `PHI_POLICY_BLOCKED` before any IRIS
      call
- [ ] T019 [P] [US1] Add unit test to `test_interop_depth_unit.rs` — `iris_message_body`
      with `dataPolicy=allow` and no `acknowledgePhi` → returns `PHI_ACK_REQUIRED` error
- [ ] T020 [P] [US1] Add unit test to `test_interop_depth_unit.rs` — `iris_message_body`
      with `max_bytes=0` → clamped to 1 (no error); `max_bytes=2000000` → clamped to
      1048576 with `max_bytes_clamped: true`
- [ ] T021 [P] [US1] Add unit test to `test_interop_depth_unit.rs` — `iris_message_body`
      on `mcpTemplate=live` policy (mocked gate, `dataPolicy=allow`, `acknowledgePhi=true`)
      → NOT blocked (Query category)
- [ ] T022 [US1] Create `crates/iris-agentic-dev-core/tests/integration/test_interop_depth_live.rs`
      — `#[ignore]`; fetch a message with known ID on live IRIS; assert `content_type` is
      non-empty; assert `body` is non-empty; assert `success: true`

### Implementation for US1

- [ ] T023 [US1] Implement `handle_iris_message_body` in `interop.rs`:
  - Parse `message_id` (required — return `INVALID_MESSAGE_ID` if non-integer),
    `namespace`, `max_bytes` (default 65536, clamp to 1MB), `acknowledgePhi`
  - Enforce PHI policy: `block` → `PHI_POLICY_BLOCKED`; `allow` without `acknowledgePhi`
    → `PHI_ACK_REQUIRED`
  - Call `dispatch_gate()` with `ToolCategory::Query`
  - Build ObjectScript: open `Ens.MessageHeader` by ID, get body object, check `%IsA`
    for stream types, read content up to `max_bytes`, set `truncated`/`actual_size`
  - Apply `detect_content_type()` to classify the body string
  - For `dataPolicy=redact`: apply `redact_hl7v2()` to the body before returning
  - Return `{success, message_id, content_type, body, truncated, actual_size}`
- [ ] T024 [US1] Run `cargo test -p iris-agentic-dev-core test_interop_depth` — all US1
      unit tests must pass

**Checkpoint**: US1 complete. `iris_message_body` returns body/content_type with PHI gates
enforced. `MESSAGE_NOT_FOUND` fires for unknown IDs.

---

## Phase 4: User Story 2 — iris_business_rule_info (Priority: P2)

**Goal**: List business rules in a production or inspect a specific rule's conditions/actions.

**Independent Test**: Call `iris_business_rule_info` with `action=list` on a namespace
with Ensemble configured. Expect a non-empty `rules` array.

### Tests for US2

> Write FIRST. Must FAIL before T030.

- [ ] T025 [P] [US2] Add unit test to `test_interop_depth_unit.rs` — `iris_business_rule_info`
      with `action=get` and missing `rule_name` → structured error (not panic)
- [ ] T026 [P] [US2] Add unit test to `test_interop_depth_unit.rs` — `iris_business_rule_info`
      with invalid `action` value → structured error with `error_code: "INVALID_ACTION"`
- [ ] T027 [P] [US2] Add unit test to `test_interop_depth_unit.rs` — `iris_business_rule_info`
      on `mcpTemplate=live` (mocked gate) → NOT blocked (Query category)
- [ ] T028 [US2] Add integration test to `test_interop_depth_live.rs` — `#[ignore]`;
      `action=list` on namespace with Ensemble → `rules` array non-empty OR empty (both
      valid); `success: true`; no IRIS exception
- [ ] T029 [US2] Add integration test to `test_interop_depth_live.rs` — `#[ignore]`;
      `action=get` on non-existent rule name → `RULE_NOT_FOUND` error code (not panic,
      not generic IRIS exception)

### Implementation for US2

- [ ] T030 [US2] Implement `handle_iris_business_rule_info` in `interop.rs`:
  - Parse `action` (required, enum list|get — return `INVALID_ACTION` for unknown values),
    `rule_name` (required for `get`), `namespace`
  - Call `dispatch_gate()` with `ToolCategory::Query`
  - `list` action: execute SQL `SELECT Name, Description, TimeModified FROM EnsLib_Rules.Definition`
    via IRIS; serialize to `{rules: [...]}` response; return `INTEROP_NOT_AVAILABLE` if
    table does not exist
  - `get` action: open rule by name (attempt `##class(EnsLib.Rules.RuleDefinition).%OpenId`
    or query by `Name`); return `RULE_NOT_FOUND` if absent; serialize `conditions` and
    `actions` arrays from rule definition properties
- [ ] T031 [US2] Run `cargo test -p iris-agentic-dev-core test_interop_depth` — all US1+US2
      unit tests pass

**Checkpoint**: US2 complete. `iris_business_rule_info list` returns rule catalog;
`get` returns conditions and actions; `RULE_NOT_FOUND` for missing rules.

---

## Phase 5: User Story 3 — iris_production_diff (Priority: P2)

**Goal**: Compare running production config against last SCM-committed version.

**Independent Test**: Call `iris_production_diff` on a system with SCM configured and no
recent changes. Expect `in_sync: true`, `changes: []`.

### Tests for US3

> Write FIRST. Must FAIL before T037.

- [ ] T032 [P] [US3] Add unit test to `test_interop_depth_unit.rs` — `iris_production_diff`
      on `mcpTemplate=live` (mocked gate) → NOT blocked (Query category)
- [ ] T033 [P] [US3] Add unit test to `test_interop_depth_unit.rs` — diff logic:
      two identical item lists → `in_sync: true`, `changes: []`
- [ ] T034 [P] [US3] Add unit test to `test_interop_depth_unit.rs` — diff logic:
      current has one extra item not in SCM → `changes` contains one entry with
      `status: "added"`
- [ ] T035 [P] [US3] Add unit test to `test_interop_depth_unit.rs` — diff logic:
      SCM has one item not in current → `changes` contains one entry with
      `status: "removed"`
- [ ] T036 [US3] Add integration test to `test_interop_depth_live.rs` — `#[ignore]`;
      call `iris_production_diff` on namespace with no SCM → `NO_SCM` or `NO_SCM_VERSION`
      error code (acceptable; validates error path not panic); OR on SCM-enabled namespace
      with no changes → `in_sync: true`

### Implementation for US3

- [ ] T037 [US3] Implement `handle_iris_production_diff` in `interop.rs`:
  - Parse `production` (optional — defaults to running production), `namespace`
  - Call `dispatch_gate()` with `ToolCategory::Query`
  - Build ObjectScript: check SCM configuration via `%Studio.SourceControl.GetStatus`
    equivalent; return `NO_SCM` if not configured
  - Fetch current production config items via `Ens.Config.Production` extent SQL
  - Fetch SCM version via `%Studio.SourceControl` export of the production class
  - Diff: compute added/removed/modified using item name as key; modified if properties
    differ (enabled flag, class, settings)
  - Return `{success, in_sync, changes: [{item_name, item_type, status}]}`
- [ ] T038 [US3] Run `cargo test -p iris-agentic-dev-core test_interop_depth` — all
      US1–US3 unit tests pass

**Checkpoint**: US3 complete. `iris_production_diff` returns change set; `NO_SCM` on
unconfigured instances; `in_sync: true` when clean.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: PHI/gate integration tests, check_config inventory, AGENTS.md update, final
test run and fmt/clippy pass.

- [ ] T039 Add PHI gate integration test to `test_interop_depth_live.rs` — `#[ignore]`;
      call `iris_message_body` with `dataPolicy=block` (set via env) → `PHI_POLICY_BLOCKED`;
      call with `dataPolicy=allow`, `acknowledgePhi=true` → attempt proceeds (may fail with
      `MESSAGE_NOT_FOUND` if no test message, but gate must not block)
- [ ] T040 [P] Verify all three tools appear in `check_config` tool inventory — add assertions
      to `test_server_manager.rs` that `registered_tool_names()` contains `"iris_message_body"`,
      `"iris_business_rule_info"`, `"iris_production_diff"`
- [ ] T041 [P] Update `light-skills/AGENTS.md` — add all three tools to the MCP tool
      reference section with usage examples; include PHI policy notes for `iris_message_body`
- [ ] T042 Run full test suite: `cargo test -p iris-agentic-dev-core` — all non-ignored
      tests pass, zero regressions
- [ ] T043 Run `cargo fmt --all -- --check` — no formatting diff
- [ ] T044 Run `cargo clippy -p iris-agentic-dev-core -- -D warnings` — zero warnings
- [ ] T045 [P] Update spec status to `Status: Implemented` in
      `specs/056-interop-depth/spec.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 (stubs must exist)
- **Phase 3 (US1 message_body)**: Depends on Phase 2 (needs shared helpers)
- **Phase 4 (US2 business_rule_info)**: Depends on Phase 1; independent of Phase 3
- **Phase 5 (US3 production_diff)**: Depends on Phase 1; independent of Phases 3–4
- **Phase 6 (Polish)**: Depends on all US phases complete

### User Story Dependencies

- **US1 (message_body)**: Requires foundational helpers (Phase 2)
- **US2 (business_rule_info)**: Requires Phase 1 only; independent of US1
- **US3 (production_diff)**: Requires Phase 1 only; independent of US1/US2

### Within Each Phase

- Tests written FIRST, must FAIL before implementation
- Shared helpers (`redact_hl7v2`, `detect_content_type`, `truncate_body`) in Phase 2
  must exist before US1 implementation but not before US1 test writing

### Parallel Opportunities

- T015–T021 (US1 unit tests) — all parallel after T014 creates the file
- T025–T027 (US2 unit tests) — all parallel (appending to existing file)
- T032–T035 (US3 unit tests) — all parallel
- T040–T041, T043–T045 (Polish) — parallel after T042 test suite passes

---

## Implementation Strategy

### MVP First (US1 only — the P1 story)

1. Complete Phase 1: Setup (T001–T009)
2. Complete Phase 2: Foundational (T010–T013)
3. Complete Phase 3: US1 message_body (T014–T024)
4. **STOP and VALIDATE**: `cargo test test_interop_depth` green; live test fetches a real
   message body; PHI gate blocks on `dataPolicy=block`
5. Ship US1 — message body retrieval is the highest-value operation

### Incremental Delivery

1. Setup + Foundational → three stubs registered, shared helpers ready
2. US1 message_body → full body retrieval with PHI gates
3. US2 business_rule_info → rule catalog and inspection
4. US3 production_diff → drift detection against SCM
5. Polish → integration coverage, docs

---

## Notes

- `iris_message_body` PHI enforcement happens at two levels: (1) `dispatch_gate()` enforces
  the standard `dataPolicy=block` gate; (2) `acknowledgePhi=true` requirement for
  `dataPolicy=allow` is an additional check in the handler before any IRIS call.
- HL7 v2 redaction in Rust (post-fetch) is simpler than pre-fetch redaction and avoids
  ObjectScript string manipulation complexity. The tradeoff is that the raw body is
  briefly in Rust memory before redaction — acceptable since we never log it.
- `iris_production_diff` v1 does not surface property-level diffs within modified items.
  Only the presence/absence of items is compared. This is documented in spec Assumptions §6.
- New error codes to register in `gate.rs` and AGENTS.md: `MESSAGE_NOT_FOUND`,
  `PHI_ACK_REQUIRED`, `INVALID_MESSAGE_ID`, `RULE_NOT_FOUND`, `NO_SCM`, `NO_SCM_VERSION`,
  `PRODUCTION_NOT_FOUND`, `STREAM_READ_ERROR`, `INTEROP_NOT_AVAILABLE`.
