# Implementation Plan: iris_query Read-Only SQL Safety Gate

**Branch**: `033-sql-safety-gate` | **Date**: 2026-05-07 | **Spec**: `specs/033-sql-safety-gate/spec.md`
**Input**: Feature specification from `/specs/033-sql-safety-gate/spec.md`

## Summary

Add `validate_read_only_sql()` to the `iris_query` handler — a pure-Rust comment-stripping
keyword gate that blocks destructive SQL (DROP, DELETE, INSERT, etc.) before any network call.
Extend `QueryParams` with `force: bool` (default false) as an explicit bypass. Zero new crate
dependencies. All validation logic is in-process; no IRIS connection required.

## Technical Context

**Language/Version**: Rust 1.92 (`crates/iris-dev-core`)  
**Primary Dependencies**: None new — uses only `std` (regex-free string scanning). All
existing workspace crates (serde_json, rmcp) already present.  
**Storage**: N/A — pure in-memory validation, no persistence  
**Testing**: `cargo test` — unit tests (no IRIS), integration `#[ignore]` E2E test  
**Target Platform**: macOS arm64/x86_64, Linux x86_64, Windows x86_64  
**Performance Goals**: Validation < 1ms per query regardless of SQL length  
**Constraints**: No new crate dependencies (Constitution VII). No IRIS call during validation. `force: true` must respect `self.write_tools_enabled` (issue #26 prod guard — if write tools disabled, force is ignored).  
**Scale/Scope**: Single function + parameter addition. One file changed: `crates/iris-dev-core/src/tools/mod.rs`.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Zero-Install Binary | ✅ PASS | No new crates, no new runtime. Single-file Rust change. |
| II. ObjectScript Sanity | ✅ PASS | No ObjectScript APIs used — pure Rust string processing. |
| III. HTTP-First Execution | ✅ PASS | `iris_query` already HTTP-only. No Docker dependency added. |
| IV. Test-First, Fixture-Driven | ✅ PASS | ≥20 unit tests written first; `#[ignore]` E2E; no fixtures needed (SQL strings are inline). |
| V. Output Shape Parity | ✅ PASS | `SQL_WRITE_BLOCKED` and `EMPTY_QUERY` are additive new error codes. Existing success shape unchanged. |
| VI. Environment Guard | ✅ PASS | `iris_query` is a read tool. The safety gate makes it *more* read-safe, not write-capable. Not subject to prod-guard gating. |
| VII. Dependency Minimalism | ✅ PASS | Zero new crates. Comment stripping and keyword matching implemented with `std` in ≤80 lines. |

**Post-design re-check**: All gates pass. No violations.

## Project Structure

### Documentation (this feature)

```text
specs/033-sql-safety-gate/
├── plan.md              # This file
├── research.md          # Phase 0 — keyword list, comment stripping decisions, force param rationale
├── data-model.md        # Phase 1 — ValidationResult type, error codes, QueryParams extension
├── quickstart.md        # Phase 1 — usage examples
├── contracts/
│   └── iris_query.md    # Updated tool contract with force param and new error codes
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code

```text
crates/iris-dev-core/src/tools/mod.rs   # validate_read_only_sql() + QueryParams.force + iris_query handler update
crates/iris-dev-core/tests/unit/
└── test_sql_safety.rs                  # NEW — ≥20 unit tests, all run without IRIS
crates/iris-dev-core/tests/integration/
└── test_sql_safety_e2e.rs              # NEW — #[ignore] E2E: blocked, force bypass, normal SELECT
```

**Structure Decision**: Single-file implementation. `validate_read_only_sql()` is a free
function in `mod.rs` (same pattern as `build_test_run_from_sql()`). No new module needed.

## Complexity Tracking

| Decision | Why Needed | Simpler Alternative Rejected Because |
|----------|------------|-------------------------------------|
| Regex-free keyword matching | Constitution VII — no `regex` crate | `regex` would work but adds a compile dep for trivial word-boundary matching |
| Strip comments before keyword scan | Bypass prevention | Without stripping, `/* DROP */ SELECT` falsely blocks |
| `force: bool` escape hatch | Legitimate admin use cases | No escape hatch = unusable for migrations/setup scripts |
| Scan for keywords as word tokens (not substrings) | Avoid false positives on `DROPPED`, `CREATING`, etc. | Substring match would block valid column names containing blocked words |
