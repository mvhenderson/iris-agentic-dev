# Feature Specification: iris_query Read-Only SQL Safety Gate

**Feature Branch**: `033-sql-safety-gate`  
**Created**: 2026-05-07  
**Status**: Implemented — merged to master  
**Closes**: #33

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Destructive SQL is blocked before reaching IRIS (Priority: P1)

An AI agent working on an ObjectScript project calls `iris_query` with a destructive SQL statement — by mistake, hallucination, or prompt injection. The tool catches this before forwarding to IRIS and returns a clear error identifying the offending keyword.

**Why this priority**: This is the core safety guarantee. Without it, an agent can irreversibly destroy data. Protection must be reliable, fast, and work with no IRIS connection required.

**Independent Test**: Call `iris_query` with `DROP TABLE MyApp.Orders` when no IRIS instance is running. Expect a `SQL_WRITE_BLOCKED` error with the offending keyword — no network call made.

**Acceptance Scenarios**:

1. **Given** `iris_query` is called with `DELETE FROM MyApp.Orders`, **When** the tool processes the request, **Then** it returns `{success: false, error_code: "SQL_WRITE_BLOCKED", error: "Destructive SQL keyword 'DELETE' is not allowed. Use force: true to override.", blocked_keyword: "DELETE"}` without contacting IRIS.
2. **Given** `iris_query` is called with `DROP TABLE MyApp.Orders`, **When** the tool processes the request, **Then** it returns `SQL_WRITE_BLOCKED` with `blocked_keyword: "DROP"`.
3. **Given** `iris_query` is called with `/* DROP TABLE MyApp.Orders */ SELECT 1`, **When** the tool processes the request, **Then** the query is allowed — comment contents stripped before keyword check.
4. **Given** `iris_query` is called with `SELECT name INTO #temp FROM MyApp.Orders`, **When** the tool processes the request, **Then** it returns `SQL_WRITE_BLOCKED` with `blocked_keyword: "SELECT INTO"`.
5. **Given** `iris_query` is called with `SELECT * FROM MyApp.Orders`, **When** the tool processes the request, **Then** it proceeds normally to IRIS.

---

### User Story 2 — Developer bypasses the gate with an explicit override (Priority: P2)

A developer needs to run a non-SELECT statement (migration, cleanup script). They set `force: true` explicitly and the query runs.

**Why this priority**: Without an escape hatch the safety gate blocks legitimate use. The `force` flag makes the override deliberate rather than silent.

**Independent Test**: Call `iris_query` with `DELETE FROM MyApp.Temp` and `force: true`. Verify the query reaches IRIS (or returns an IRIS error, not a block error).

**Acceptance Scenarios**:

1. **Given** `iris_query` is called with a destructive statement AND `force: true`, **When** processed, **Then** the query is forwarded to IRIS without blocking.
2. **Given** `iris_query` is called with `force: true` and a normal SELECT, **When** processed, **Then** it behaves identically to `force: false`.
3. **Given** `force` is not specified, **When** `iris_query` is called, **Then** `force` defaults to `false`.

---

### User Story 3 — Comment-obfuscated and case-varied attacks are caught (Priority: P1)

An attacker hides destructive keywords in SQL comments or uses case variations. The gate catches these regardless.

**Why this priority**: A bypassable safety gate provides false confidence. Comment stripping and case-insensitive matching are non-negotiable.

**Independent Test**: Call with `DeLeTe FROM foo` — verify blocked. Call with `SELECT 1; DROP TABLE foo` — verify blocked.

**Acceptance Scenarios**:

1. **Given** SQL is `-- DROP TABLE foo\nSELECT 1`, **When** processed, **Then** line comment stripped — SELECT allowed.
2. **Given** SQL is `SELECT /* DELETE */ * FROM foo`, **When** processed, **Then** block comment stripped — SELECT allowed.
3. **Given** SQL is `DeLeTe FROM foo`, **When** processed, **Then** blocked — matching is case-insensitive.
4. **Given** SQL is `SELECT 1; DROP TABLE foo`, **When** processed, **Then** `DROP` detected — query blocked.

---

### Edge Cases

- SQL that is purely whitespace or empty after comment stripping — return `EMPTY_QUERY` rather than forwarding.
- `SELECT INTO` inside a subquery (e.g., `SELECT * FROM (SELECT id INTO #t FROM foo)`) — still blocked.
- Very long SQL (> 100KB) — validation must complete in < 10ms.
- Keyword as a quoted identifier (e.g., `SELECT "DROP" FROM foo`) — allowed; quoted identifiers not scanned.
- Keyword inside a string literal (e.g., `SELECT 'CALL me' FROM foo`) — allowed; string literals not scanned.
- `force: true` on a production instance — `force` is silently ignored; `SQL_WRITE_BLOCKED` returned with `force_ignored: true`. No IRIS call made.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Before forwarding SQL to IRIS, `iris_query` MUST pass the SQL through `validate_read_only_sql()`. If validation fails, the query MUST NOT be sent to IRIS.
- **FR-002**: `validate_read_only_sql()` MUST strip `--` line comments and `/* */` block comments before keyword analysis.
- **FR-003**: `validate_read_only_sql()` MUST reject SQL containing any of the following 14 unquoted keywords (case-insensitive): `INSERT`, `UPDATE`, `DELETE`, `DROP`, `ALTER`, `CREATE`, `MERGE`, `TRUNCATE`, `EXEC`, `EXECUTE`, `BULK`, `LOAD`, `KILL`, `LOCK`. Note: `CALL` is intentionally excluded — see research.md for rationale (CALL calls stored procedures but does not itself modify data; blocking it would break legitimate procedure queries).
- **FR-004**: `validate_read_only_sql()` MUST reject `SELECT ... INTO <identifier>` patterns (DDL via SELECT INTO).
- **FR-005**: When `iris_query` is called with `force: true` AND the server is connected to a non-production IRIS instance, validation MUST be skipped and the query forwarded as-is.
- **FR-005b**: When `iris_query` is called with `force: true` AND the server is connected to a production IRIS instance (write tools disabled per issue #26), `force: true` MUST be ignored — validation still applies. The tool MUST include `"force_ignored": true` in the blocked response to signal this.
- **FR-006**: `force` MUST default to `false`.
- **FR-007**: When a query is blocked, the response MUST include `error_code: "SQL_WRITE_BLOCKED"`, a human-readable `error` message, and a `blocked_keyword` field.
- **FR-008**: Empty SQL (whitespace-only after comment stripping) MUST return `error_code: "EMPTY_QUERY"` without contacting IRIS.
- **FR-009**: Quoted identifiers and string literals MUST NOT be scanned for keywords.
- **FR-010**: The `iris_query` tool description MUST document the `force` parameter and warn it bypasses SQL safety validation.

### Key Entities

- **ValidationResult**: Outcome of `validate_read_only_sql()` — Ok (allowed) or Err(blocked_keyword).
- **QueryParams**: Extended with `force: bool` (default false).

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All blocked keyword types (14 keywords + SELECT INTO) are rejected in under 1ms per query — pure in-process, no IRIS call.
- **SC-002**: A suite of ≥ 20 unit tests covering keyword variants, comment stripping, case variations, quoted identifiers, empty SQL, and `force` bypass all pass.
- **SC-003**: Zero false positives on a representative set of 50 real-world SELECT queries.
- **SC-004**: Existing `iris_query` callers using only SELECT queries experience zero behavior or performance change.
- **SC-E2E**: End-to-end test confirms: blocked query returns `SQL_WRITE_BLOCKED` without IRIS call; `force: true` query reaches IRIS; normal SELECT returns rows unchanged.

---

## Assumptions

- The Atelier REST `/action/query` endpoint can execute arbitrary SQL including DDL — client-side validation is not redundant.
- Quoted identifiers use standard SQL quoting: `'single quotes'` for strings, `"double quotes"` for identifiers.
- The `force: true` escape hatch is trusted — no additional audit logging at this time. Follow-on feature if needed.
- Multi-statement SQL (semicolon-separated) is scanned as a whole string — any blocked keyword anywhere triggers a block.
- IRIS-specific syntax injected into SQL is not specially handled; only standard SQL keywords are blocked.
