# Feature Specification: iris_execute &sql Macro Translation

**Feature Branch**: `035-sql-macro-translate`  
**Created**: 2026-05-08  
**Status**: Implemented — merged to master  

## Clarifications

### Session 2026-05-08

- Q: What is the correct scope for SQLCODE/`%msg` rewriting after a translated `&sql(...)`? → A: Rewrite only the SQLCODE/`%msg` reference on the immediately following line after the `&sql(...)` macro (standard IRIS idiom). All other SQLCODE references elsewhere in the code are left untouched to avoid corrupting unrelated error handling.
- Q: What should translated SELECT INTO do when the query returns no rows? → A: Match `&sql` semantics exactly — host variables are set to `""` (empty string) on no-match, and SQLCODE on the next line is rewritten to 100. This matches what the IRIS preprocessor would generate.

---

## Overview

ObjectScript code uses `&sql(...)` as shorthand for embedded SQL — a preprocessor macro that the IRIS compiler expands at compile time. The `iris_execute` tool runs code via a runtime mechanism that bypasses the IRIS preprocessor, so `&sql(...)` silently fails or produces an error.

This feature adds transparent translation: when `iris_execute` receives code containing `&sql(...)`, it rewrites those macros to equivalent runtime-compatible SQL calls before sending to IRIS. The translation is visible in the response so agents can inspect exactly what ran.

---

## User Scenarios & Testing

### User Story 1 — Agent writes &sql code and it just works (Priority: P1)

An agent generates ObjectScript that queries IRIS using `&sql(SELECT Name INTO :name FROM MyApp.Patient WHERE ID = :id)`. Without knowing whether the execution path supports embedded SQL, the code runs correctly and returns the expected output.

**Why this priority**: `&sql` is idiomatic ObjectScript that agents naturally produce — especially when working with existing codebases that use it. Requiring agents to avoid it or manually rewrite every occurrence creates unnecessary friction.

**Independent Test**: Call `iris_execute` with code containing `&sql(SELECT 1 INTO :x)` followed by `write x`. Verify output is `1` without any error.

**Acceptance Scenarios**:

1. **Given** `iris_execute` is called with code containing `&sql(SELECT Name INTO :name FROM MyApp.Patient WHERE ID = :id)`, **When** the tool runs, **Then** the correct Name value is returned and `sql_translated: true` appears in the response.
2. **Given** `iris_execute` is called with code containing `&sql(INSERT INTO MyApp.Log (Message) VALUES (:msg))`, **When** the tool runs, **Then** the insert executes successfully and `sql_translated: true` appears in the response.
3. **Given** `iris_execute` is called with code that checks `SQLCODE` after `&sql(...)`, **When** translated, **Then** the SQLCODE check is correctly rewritten to use the result set's status field.
4. **Given** `iris_execute` is called with code containing no `&sql(...)`, **When** the tool runs, **Then** behavior is identical to today — no translation occurs, `sql_translated` is absent from the response.

---

### User Story 2 — Agent opts out of translation to debug the raw code (Priority: P2)

An agent suspects the translation is producing incorrect results and wants to send the raw `&sql(...)` code to IRIS directly to compare. It calls `iris_execute` with `translate_sql: false` to bypass translation.

**Why this priority**: Transparency and debuggability. Agents must be able to reason about what runs vs. what was written. Hiding translation permanently would make debugging harder.

**Independent Test**: Call `iris_execute` with `&sql(SELECT 1)` and `translate_sql: false`. Verify the response includes an IRIS error (not translated output) and `sql_translated` is absent.

**Acceptance Scenarios**:

1. **Given** `translate_sql: false` and code containing `&sql(...)`, **When** the tool runs, **Then** the code is sent to IRIS as-is and `sql_translated` is absent from the response.
2. **Given** `translate_sql` is not specified, **When** any `iris_execute` call is made, **Then** `translate_sql` defaults to `true`.
3. **Given** `translate_sql: true` and code with no `&sql(...)`, **When** the tool runs, **Then** behavior is identical to `translate_sql: false` — no overhead, no response field.

---

### User Story 3 — Agent inspects the translated code (Priority: P2)

An agent receives output from a translated `&sql(...)` call and wants to understand exactly what ObjectScript was sent to IRIS — either to verify correctness, reuse the pattern, or troubleshoot unexpected output.

**Why this priority**: The `translated_code` field makes translation auditable. Agents can learn idiomatic `%SQL.Statement` patterns from successful translations.

**Independent Test**: Call `iris_execute` with a multi-variable `&sql(SELECT ...)` and verify `translated_code` in the response contains valid `%SQL.Statement` ObjectScript.

**Acceptance Scenarios**:

1. **Given** translation fires, **When** the tool returns, **Then** the response includes `translated_code` containing the complete ObjectScript that was actually executed (the rewritten code, not the original).
2. **Given** translation fires with a SELECT INTO statement, **When** the tool returns, **Then** `translated_code` shows `%SQL.Statement.%New()`, `.%Prepare(...)`, `.%Execute(...)`, and `.%Get(...)` calls corresponding to the original `&sql(...)`.
3. **Given** translation encounters a construct it cannot safely translate (e.g., stored procedure CALL with OUT parameters), **When** the tool returns, **Then** the response includes `translation_warning` describing what was skipped, and the untranslatable `&sql(...)` is left in place with a comment.

---

### Edge Cases

- Code containing multiple `&sql(...)` calls — each translated independently in order.
- `&sql(...)` inside a loop or conditional — translation preserves surrounding code structure.
- Host variable `:varname` appearing in multiple `&sql(...)` calls — each translation gets its own result set variable to avoid collisions.
- `SQLCODE` and `%msg` referenced after `&sql(...)` — rewritten to read from the result set object created by the translation.
- `&sql(CALL MyProc(...))` — translation not attempted; left as-is with `translation_warning` in response.
- Code with no `&sql(...)` and `translate_sql: true` — no translation overhead, no `sql_translated` field in response.
- Very long SQL string (> 4KB) — translation must still complete; no length limit on input.
- `SQLCODE` used after a non-`&sql` operation (e.g., `%Save()`) — NOT rewritten; only the line immediately following an `&sql(...)` macro is in scope for SQLCODE/`%msg` rewriting.
- SELECT INTO returns no rows — host variables set to `""`, SQLCODE on next line resolves to 100 (matching `&sql` preprocessor semantics).

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: When `iris_execute` is called with `translate_sql: true` (the default) and the code contains one or more `&sql(...)` macros, the tool MUST rewrite those macros to runtime-compatible SQL calls before executing.
- **FR-002**: When translation fires, the response MUST include `"sql_translated": true` and `"translated_code"` containing the full rewritten ObjectScript that was sent to IRIS.
- **FR-003**: `translate_sql` MUST default to `true`. Agents that do not specify the parameter receive translation automatically.
- **FR-004**: When `translate_sql: false`, the code MUST be sent to IRIS unchanged and `sql_translated` MUST be absent from the response.
- **FR-005**: SELECT INTO translation MUST extract `:varname` host variables in order and generate the corresponding `%SQL.Statement` prepare/execute/get sequence. When the query returns no rows (`%Next()` returns false), host variables MUST be set to `""` (empty string) and the result set's `%SQLCODE` MUST be 100 — matching the IRIS `&sql` preprocessor behavior exactly.
- **FR-006**: INSERT, UPDATE, DELETE, and MERGE translation MUST use `%SQL.Statement.%ExecDirect` with positional `?` parameters replacing `:varname` host variables in order.
- **FR-007**: A `SQLCODE` reference on the line immediately following a translated `&sql(...)` MUST be rewritten to read the status from the generated result set object. `SQLCODE` references elsewhere in the code block MUST NOT be modified.
- **FR-008**: A `%msg` reference on the line immediately following a translated `&sql(...)` MUST be rewritten to read the message from the generated result set object. `%msg` references elsewhere in the code block MUST NOT be modified.
- **FR-009**: When a `&sql(...)` construct cannot be safely translated (e.g., CALL statements, subqueries with host variables in non-standard positions), the macro MUST be left unchanged and a `"translation_warning"` field MUST be added to the response describing what was skipped.
- **FR-010**: Translation MUST add no IRIS network calls — it is pure text transformation applied before the code is sent.
- **FR-011**: When no `&sql(...)` is present in the code, translation MUST be a no-op with zero overhead regardless of `translate_sql` value.

### Key Entities

- **TranslationResult**: Outcome of the translation pass — original code, translated code, whether any macros were found, and any warnings for untranslatable constructs.
- **ExecuteParams**: Extended with `translate_sql: bool` (default true).

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A representative set of ≥ 15 `&sql(...)` patterns (SELECT INTO single/multi var, INSERT, UPDATE, DELETE, SQLCODE check, %msg check) all translate correctly and execute successfully against a live IRIS instance.
- **SC-002**: Zero false translations — code with no `&sql(...)` is never modified regardless of `translate_sql` value.
- **SC-003**: Untranslatable constructs (CALL, complex host variables) produce a `translation_warning` and do not cause `iris_execute` to fail — they fall through to IRIS unchanged.
- **SC-004**: Translation adds no measurable latency overhead for code without `&sql(...)` — the check is a fast string scan with early exit.
- **SC-E2E**: End-to-end test against a live IRIS instance: code with `&sql(SELECT 1 INTO :x)` followed by `write x` returns `1` with `sql_translated: true` and valid `translated_code`.

---

## Assumptions

- `&sql(...)` syntax follows the standard IRIS ObjectScript preprocessor convention: the macro opens with `&sql(` and closes with the matching `)`.
- Host variables use the `:varname` syntax (colon prefix, alphanumeric name).
- `SQLCODE` and `%msg` are the only special post-`&sql` variables that require rewriting; other IRIS special variables are not affected.
- The translation handles single `&sql(...)` statements (one SQL statement per macro invocation). Multi-statement SQL inside a single `&sql(...)` is not supported and falls through with a warning.
- Generated result set variable names use a collision-avoidance pattern (e.g., `_rs1`, `_rs2`) to avoid conflicting with user-defined variables.
- The `translate_sql` parameter applies to the entire code block — there is no per-`&sql` opt-out within a single call.
- `&sql(CALL ...)` stored procedure calls are explicitly out of scope for this feature due to OUT parameter complexity.
