# Implementation Plan: iris_execute &sql Macro Translation

**Branch**: `035-sql-macro-translate` | **Date**: 2026-05-08 | **Spec**: `specs/035-sql-macro-translate/spec.md`
**Input**: Feature specification from `/specs/035-sql-macro-translate/spec.md`

## Summary

Add `translate_sql: bool` (default `true`) to `ExecuteParams`. Before sending code to IRIS via `execute_via_generator`, scan for `&sql(...)` macros and rewrite them to `%SQL.Statement` class method calls. The translation is a pure Rust string transformation — no IRIS call, no new crates. The response includes `sql_translated: true` and `translated_code` when translation fires.

**Key research finding**: The actual IRIS `&sql` preprocessor generates cached query classes (`%sqlcq.*`) via `$classmethod` — not `%SQL.Statement`. However, `%SQL.Statement` is the correct *runtime* equivalent for `execute_via_generator` since that path bypasses the IRIS preprocessor entirely. `%SQL.Statement` has been verified to produce identical results.

## Technical Context

**Language/Version**: Rust 1.92 (`crates/iris-dev-core/src/tools/mod.rs`)
**Primary Dependencies**: No new crates. Pure `std` string processing.
**Storage**: N/A — pure in-memory transformation
**Testing**: `cargo test` — unit tests (no IRIS), `#[ignore]` E2E tests against `iris-dev-iris`
**Target Platform**: macOS arm64/x86_64, Linux x86_64, Windows x86_64
**Performance Goals**: Translation < 1ms for typical code blocks; no overhead when no `&sql(...)` present
**Constraints**: No new crate dependencies (Constitution VII). No IRIS call during translation.
**Scale/Scope**: One new function `translate_sql_macros()` + `ExecuteParams.translate_sql` field. Single file: `crates/iris-dev-core/src/tools/mod.rs`.

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Zero-Install Binary | ✅ PASS | No new crates, no new runtime |
| II. ObjectScript Sanity | ✅ PASS | `%SQL.Statement` API verified against live IRIS (see research.md). `.%New()`, `.%Prepare()`, `.%Execute()`, `.%Next()`, `.%Get()`, `.%ExecDirect()` all confirmed to exist. |
| III. HTTP-First Execution | ✅ PASS | Translation happens before the HTTP call; no new execution path added |
| IV. Test-First, Fixture-Driven | ✅ PASS | ≥15 unit tests (pure Rust, no IRIS) + E2E tests `#[ignore]` |
| V. Output Shape Parity | ✅ PASS | `sql_translated` and `translated_code` are additive fields; existing response shape unchanged |
| VI. Environment Guard | ✅ PASS | `iris_execute` is already a write-capable tool; no change to its gate status |
| VII. Dependency Minimalism | ✅ PASS | Zero new crates; regex-free string parsing in ≤150 lines |

**Post-design re-check**: All gates pass. No violations.

## Project Structure

### Documentation

```text
specs/035-sql-macro-translate/
├── plan.md              # This file
├── research.md          # Phase 0 — &sql INT expansion, %SQL.Statement verification
├── data-model.md        # Phase 1 — TranslationResult, ExecuteParams
├── quickstart.md        # Phase 1 — usage examples
├── contracts/
│   └── iris_execute.md  # Updated tool contract with translate_sql param
└── tasks.md             # Phase 2 output
```

### Source Code

```text
crates/iris-dev-core/src/tools/mod.rs
  ├── translate_sql_macros(code: &str) -> TranslationResult   # NEW pure function
  ├── ExecuteParams.translate_sql: bool                        # NEW field (default true)
  └── iris_execute handler — call translate_sql_macros() before execute_via_generator

crates/iris-dev-core/tests/unit/test_sql_translate.rs          # NEW — unit tests (no IRIS)
crates/iris-dev-core/tests/integration/test_sql_translate_e2e.rs  # NEW — #[ignore] E2E
```

**Structure Decision**: `translate_sql_macros()` is a free function (same pattern as `validate_read_only_sql()`). `TranslationResult` is a struct with `translated_code: String`, `found: bool`, `warnings: Vec<String>`. No new module needed.

## Complexity Tracking

| Decision | Why Needed | Simpler Alternative Rejected Because |
|----------|------------|--------------------------------------|
| `%SQL.Statement` as translation target (not `%sqlcq.*`) | `execute_via_generator` bypasses IRIS preprocessor; cached query classes require compile-time generation | `%sqlcq.*` classes are generated at compile time and namespace-specific; not available at runtime |
| Regex-free parser for `&sql(...)` | Constitution VII — no `regex` crate | Paren-depth counting is sufficient for well-formed ObjectScript |
| Scope SQLCODE/`%msg` rewrite to next line only | Clarification Q1 — avoid corrupting unrelated error handling | Broader rewrite risks breaking `%Save()` error checks, `$$$ThrowOnError` etc. |
| Host variables → positional `?` params | `%SQL.Statement` uses positional params | Named params would require different API path |
