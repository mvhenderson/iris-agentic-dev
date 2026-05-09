# Tasks: iris_query Read-Only SQL Safety Gate

**Input**: Design documents from `/specs/033-sql-safety-gate/`
**Repo**: `~/ws/iris-dev` (Rust — `crates/iris-dev-core`)
**Constitution**: Principle IV — unit tests first, no-IRIS path; Principle VII — zero new crates

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Add `force: bool` to `QueryParams` and create test file stubs — no behavior change.

- [x] T001 Add `force: bool` field with `#[serde(default)]` to `QueryParams` struct in `crates/iris-dev-core/src/tools/mod.rs`
- [x] T002 [P] Create empty test stub `crates/iris-dev-core/tests/unit/test_sql_safety.rs`
- [x] T003 [P] Create empty E2E test stub `crates/iris-dev-core/tests/integration/test_sql_safety_e2e.rs`
- [x] T004 [P] Add `[[test]]` entries for both new test files to `crates/iris-dev-core/Cargo.toml`
- [x] T005 Verify `cargo check -p iris-dev-core` passes with the new `force` field

**Checkpoint**: `cargo check` passes. `force` field present on `QueryParams`.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Implement `validate_read_only_sql()` as a pure function — shared by all user stories. Tests first.

### Tests for Phase 2 (write first — must FAIL before implementation)

- [x] T006 [P] Write unit test: `validate_read_only_sql("SELECT * FROM foo")` → Ok(()) in `tests/unit/test_sql_safety.rs` (WRITE FIRST, must FAIL)
- [x] T007 [P] Write unit test: blocked keywords — one test per keyword (DELETE, DROP, INSERT, UPDATE, ALTER, CREATE, MERGE, TRUNCATE, EXEC, EXECUTE, BULK, LOAD, KILL, LOCK) each returns `Err(keyword)` in `test_sql_safety.rs` (WRITE FIRST, must FAIL)
- [x] T008 [P] Write unit test: comment stripping — `/* DROP */ SELECT 1` → Ok(()), `-- DROP\nSELECT 1` → Ok(()) in `test_sql_safety.rs` (WRITE FIRST, must FAIL)
- [x] T009 [P] Write unit test: quoted identifiers — `SELECT "DROP" FROM foo` → Ok(()), `SELECT 'DELETE' FROM foo` → Ok(()) in `test_sql_safety.rs` (WRITE FIRST, must FAIL)
- [x] T010 [P] Write unit test: SELECT INTO — `SELECT name INTO #temp FROM foo` → Err("SELECT INTO"), `SELECT * FROM (SELECT id FROM foo)` → Ok(()) in `test_sql_safety.rs` (WRITE FIRST, must FAIL)
- [x] T011 [P] Write unit test: empty SQL — `""` and `"   "` and `"/* comment */"` each → Err("EMPTY") in `test_sql_safety.rs` (WRITE FIRST, must FAIL)
- [x] T012 [P] Write unit test: case insensitivity — `DeLeTe FROM foo`, `dRoP TABLE foo` each blocked in `test_sql_safety.rs` (WRITE FIRST, must FAIL)
- [x] T013 [P] Write unit test: semicolon injection — `SELECT 1; DROP TABLE foo` → blocked (DROP detected) in `test_sql_safety.rs` (WRITE FIRST, must FAIL)
- [x] T014 [P] Write unit test: word boundary — `CREATED_AT`, `DROPPED`, `EXECUTOR_ID` as column names in SELECT → Ok(()) in `test_sql_safety.rs` (WRITE FIRST, must FAIL)

### TDD Gate

- [x] T015 **GATE**: Confirm `cargo test --test test_sql_safety` produces FAILURES (function doesn't exist yet). Do not proceed until all T006–T014 tests fail to compile.

### Implementation for Phase 2

- [x] T016 Implement `validate_read_only_sql(sql: &str) -> Result<(), String>` in `crates/iris-dev-core/src/tools/mod.rs`:
  - Strip `/* ... */` block comments (non-greedy)
  - Strip `-- ...` line comments
  - Return `Err("EMPTY".to_string())` if result is whitespace-only
  - Walk remaining chars with quote-depth tracking; skip `'...'` and `"..."` content
  - Check each unquoted token against blocked keyword list with word-boundary check
  - Check for `SELECT ... INTO <non-paren>` pattern
  - Return `Ok(())` if all checks pass
- [x] T017 Verify T006–T014 all pass GREEN: `cargo test --test test_sql_safety`

**Checkpoint**: All foundational unit tests green. `validate_read_only_sql()` works correctly.

---

## Phase 3: User Story 1 — Destructive SQL blocked before reaching IRIS (Priority: P1) 🎯 MVP

**Goal**: `iris_query` calls `validate_read_only_sql()` before any IRIS network call. Blocked queries return `SQL_WRITE_BLOCKED` with `blocked_keyword`. Normal SELECTs unaffected.

**Independent Test**: Call `iris_query` with `DROP TABLE foo` with no IRIS running — expect `SQL_WRITE_BLOCKED` returned immediately (no network timeout).

### Tests for US1 (write first — must FAIL before implementation)

- [x] T018 [P] [US1] Write unit test: `iris_query` handler with `DROP TABLE foo` and `iris=None` → returns `SQL_WRITE_BLOCKED` (not `IRIS_UNREACHABLE`) in `test_sql_safety.rs` (WRITE FIRST, must FAIL)
- [x] T019 [P] [US1] Write unit test: `iris_query` handler with `SELECT * FROM foo` and `iris=None` → returns `IRIS_UNREACHABLE` (not blocked) in `test_sql_safety.rs` (WRITE FIRST, must FAIL)
- [x] T020 [P] [US1] Write unit test: `iris_query` handler with `/* DROP */ SELECT 1` and `iris=None` → returns `IRIS_UNREACHABLE` (comment stripped, SELECT allowed, but no IRIS) in `test_sql_safety.rs` (WRITE FIRST, must FAIL)
- [x] T021 [US1] Write E2E test (`#[ignore]`): blocked query against live `iris-dev-iris` returns `SQL_WRITE_BLOCKED` JSON with `blocked_keyword` field, no IRIS error in `tests/integration/test_sql_safety_e2e.rs` (WRITE FIRST, must FAIL)
- [x] T022 [US1] Write E2E test (`#[ignore]`): normal SELECT against live `iris-dev-iris` returns rows and `success: true` in `test_sql_safety_e2e.rs` (WRITE FIRST, must FAIL)

### TDD Gate

- [x] T023 [US1] **GATE**: Confirm T018–T022 all FAIL before writing implementation below

### Implementation for US1

- [x] T024 [US1] Wire `validate_read_only_sql()` into `iris_query` handler in `crates/iris-dev-core/src/tools/mod.rs` — call at top of handler before any network call; map `Err(keyword)` to `err_json("SQL_WRITE_BLOCKED", ...)` with `blocked_keyword` field; map `Err("EMPTY")` to `err_json("EMPTY_QUERY", ...)`
- [x] T025 [US1] **GATE-GREEN**: Run `IRIS_HOST=localhost IRIS_WEB_PORT=52780 cargo test --test test_sql_safety_e2e -- --ignored` — T021 and T022 must pass

**Phase gate**: T021 + T022 E2E tests pass. `iris_query` blocks destructive SQL before IRIS call.

---

## Phase 4: User Story 2 — `force: true` bypass (Priority: P2)

**Goal**: `force: true` skips validation on non-prod instances. On prod instances (`write_tools_enabled = false`), `force: true` is ignored and `force_ignored: true` is added to the blocked response.

**Independent Test**: Call `iris_query` with `DELETE FROM MyApp.Temp` and `force: true` against dev IRIS — query reaches IRIS (IRIS may reject on permissions, but not a block error).

### Tests for US2 (write first — must FAIL before implementation)

- [x] T026 [P] [US2] Write unit test: `force: true` + destructive SQL + `write_tools_enabled=true` → query NOT blocked (falls through to `IRIS_UNREACHABLE` since `iris=None`) in `test_sql_safety.rs` (WRITE FIRST, must FAIL)
- [x] T027 [P] [US2] Write unit test: `force: true` + destructive SQL + `write_tools_enabled=false` (prod) → still returns `SQL_WRITE_BLOCKED` with `force_ignored: true` in `test_sql_safety.rs` (WRITE FIRST, must FAIL)
- [x] T028 [P] [US2] Write unit test: `force: false` (default) + destructive SQL → blocked regardless of `write_tools_enabled` in `test_sql_safety.rs` (WRITE FIRST, must FAIL)
- [x] T029 [US2] Write E2E test (`#[ignore]`): `force: true` + `DELETE FROM NonExistentTable` against dev `iris-dev-iris` → response is NOT `SQL_WRITE_BLOCKED` (IRIS error or success, but not our block) in `test_sql_safety_e2e.rs` (WRITE FIRST, must FAIL)

### TDD Gate

- [x] T030 [US2] **GATE**: Confirm T026–T029 all FAIL before writing implementation below

### Implementation for US2

- [x] T031 [US2] Update `iris_query` handler in `mod.rs`: check `p.force` — if `true` AND `self.write_tools_enabled`, skip `validate_read_only_sql()` call; if `true` AND `!self.write_tools_enabled`, still run validation but add `"force_ignored": true` to the blocked response
- [x] T032 [US2] **GATE-GREEN**: Run E2E T029 against `iris-dev-iris` — must pass

**Phase gate**: T029 E2E passes. `force: true` bypass works on dev; silently overridden on prod.

---

## Phase 5: User Story 3 — Comment-obfuscation attacks caught (Priority: P1)

**Note**: US3 tests largely overlap with T006–T014 (foundational). This phase adds integration-level verification that the comment stripping and case-insensitive matching work end-to-end through the full `iris_query` handler, not just the validator function in isolation.

### Tests for US3 (write first — must FAIL before implementation)

- [x] T033 [P] [US3] Write unit test: full handler path — `/* DELETE */ SELECT 1` with `iris=None` → `IRIS_UNREACHABLE` (not blocked) in `test_sql_safety.rs` (WRITE FIRST, must FAIL)
- [x] T034 [P] [US3] Write unit test: full handler path — `SELECT 1; DROP TABLE foo` with `iris=None` → `SQL_WRITE_BLOCKED` with `blocked_keyword: "DROP"` in `test_sql_safety.rs` (WRITE FIRST, must FAIL)
- [x] T035 [US3] Write E2E test (`#[ignore]`): `DeLeTe FROM NonExistentTable` against live IRIS → `SQL_WRITE_BLOCKED` (not IRIS error) in `test_sql_safety_e2e.rs` (WRITE FIRST, must FAIL)

### TDD Gate

- [x] T036 [US3] **GATE**: Confirm T033–T035 FAIL before implementation (should auto-pass once US1 implementation is done — this gate confirms the handler is wired correctly end-to-end)

### Implementation for US3

- [x] T037 [US3] No new implementation needed if T033–T035 already pass after Phase 3. If any fail, diagnose and fix `validate_read_only_sql()` or handler wiring in `mod.rs`.
- [x] T038 [US3] **GATE-GREEN**: Run E2E T035 — must pass

**Phase gate**: T035 E2E passes. Comment-stripped and case-varied attacks caught end-to-end.

---

## Phase 6: Polish & Cross-Cutting

- [x] T039 [P] Update `iris_query` tool description string in `mod.rs` — document `force` param; warn it bypasses SQL safety validation; list blocked keyword categories
- [x] T040 [P] Register `SQL_WRITE_BLOCKED` and `EMPTY_QUERY` error codes in `specs/033-sql-safety-gate/data-model.md` error code registry (already present, confirm alignment with implementation)
- [x] T041 [P] Run `cargo clippy --all-targets -- -D warnings` — must be clean
- [x] T042 [P] Run `cargo fmt --all -- --check` — must be clean
- [x] T043 [P] Run full unit test suite: `cargo test -p iris-dev-core` — all unit tests pass, no regressions in `test_toolset`, `interop_unit_tests`, etc.
- [x] T044 [P] Run all E2E tests: `IRIS_HOST=localhost IRIS_WEB_PORT=52780 cargo test --test test_sql_safety_e2e -- --ignored` — all 4 E2E tests pass
- [x] T045 [P] Write unit test: ≥ 50 representative SELECT queries (drawn from existing benchmark harness SQL and iris-dev integration tests) — all return `Ok(())` from `validate_read_only_sql()` with zero false positives (SC-003) in `crates/iris-dev-core/tests/unit/test_sql_safety.rs`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — blocks all user story phases
- **Phase 3 (US1)**: Depends on Phase 2 — MVP gate
- **Phase 4 (US2)**: Depends on Phase 3 (force bypass needs validation wired first)
- **Phase 5 (US3)**: Depends on Phase 3 (comment/case tests verify handler end-to-end)
- **Phase 6 (Polish)**: Depends on all phases complete

### Critical Path

```
T001-T005 (setup) → T006-T017 (validate_read_only_sql) → T018-T025 (US1 handler wiring)
                                                        → T033-T038 (US3 — parallel with US4)
T025 (US1 gate) → T026-T032 (US2 force bypass)
T032 (US2 gate) → T039-T044 (polish)
```

### Parallel Opportunities

**Phase 2 tests** (all touch same file but are independent test functions — write concurrently):
```
T006: SELECT passes
T007: 14 blocked keywords
T008: comment stripping
T009: quoted identifiers
T010: SELECT INTO
T011: empty SQL
T012: case insensitivity
T013: semicolon injection
T014: word boundary
```

**Phase 6** — all tasks [P], run simultaneously.

---

## Implementation Strategy

### MVP: Phases 1–3 (validation wired, blocking works)

1. Setup: add `force` field (Phase 1)
2. Write all foundational unit tests, confirm RED (Phase 2)
3. Implement `validate_read_only_sql()`, confirm GREEN (Phase 2)
4. Wire into `iris_query` handler (Phase 3)
5. **VALIDATE**: `iris_query` blocks destructive SQL with no IRIS running

### Full Feature

6. Phase 4: `force: true` bypass with prod guard
7. Phase 5: E2E verification of comment/case handling
8. Phase 6: Polish, clippy, fmt, full suite
