# Research: iris_query Read-Only SQL Safety Gate

## Keyword Blocklist

**Decision**: Block the following 13 keywords + SELECT INTO pattern:
`INSERT`, `UPDATE`, `DELETE`, `DROP`, `ALTER`, `CREATE`, `MERGE`, `TRUNCATE`,
`EXEC`, `EXECUTE`, `BULK`, `LOAD`, `KILL`, `LOCK`, and `SELECT...INTO <ident>`.

**Rationale**: Derived from `esalas-devel/iris-mcp-atelier` reference implementation.
All 13 represent statement types that can modify or destroy data, crash sessions, or
alter schema. `CALL` is excluded — IRIS SQL `CALL` calls stored procedures but does not
itself modify data; a procedure called may modify data, but blocking `CALL` would break
legitimate stored-procedure queries. This is an acceptable residual risk.

**Alternatives considered**: Block all non-SELECT statements by checking the first token.
Rejected: `WITH ... AS (SELECT ...) INSERT ...` starts with WITH not INSERT — first-token
check is insufficient for CTEs.

## Comment Stripping Algorithm

**Decision**: Strip comments in two passes before keyword analysis:
1. Remove `/* ... */` block comments (including nested content, non-greedy)
2. Remove `-- ...` line comments (to end of line)

**Rationale**: Without stripping, `/* DROP TABLE foo */ SELECT 1` would falsely trigger.
With stripping, the cleaned SQL is `SELECT 1` — correctly allowed.

**Quoted identifier protection**: After comment stripping, skip content inside `'...'`
and `"..."` when scanning for keywords. A state machine tracks quote depth while walking
character-by-character.

**Alternatives considered**: Regex-based stripping. Rejected: Constitution VII prohibits
adding the `regex` crate. Manual state-machine stripping in ~50 lines of Rust is sufficient
and has no dependencies.

## Word-Boundary Matching

**Decision**: Match keywords only at word boundaries — the keyword must be preceded and
followed by a non-alphanumeric, non-underscore character (or string start/end).

**Rationale**: Prevents false positives on identifiers like `CREATED_AT` (contains CREATE),
`DROPPED` (contains DROP), `EXECUTOR_ID` (contains EXEC). Word-boundary check is a simple
character comparison, no regex needed.

**Implementation**: After finding a keyword substring match in normalized SQL, check that
`sql[match_start-1]` and `sql[match_start+keyword.len()]` are both non-word characters.

## SELECT INTO Detection

**Decision**: After comment and quote stripping, scan for the pattern `SELECT ... INTO` where
the token after INTO is an identifier (not a parenthesis — `INTO (subquery)` is allowed).

**Rationale**: `SELECT col INTO #temp FROM table` is T-SQL DDL (creates a temp table). IRIS
SQL supports this. The pattern is: keyword `INTO` followed by whitespace + non-paren character.

## `force` Parameter

**Decision**: Add `force: bool` with `#[serde(default)]` to `QueryParams`. When true,
skip validation entirely.

**Rationale**: Legitimate use cases exist (migrations, test setup, admin scripts). Making
the bypass explicit prevents accidental writes while preserving escape hatch.

**Risk**: An AI agent could set `force: true` to bypass the gate. Acceptable — the agent
must explicitly choose to override, which is a higher bar than simply sending bad SQL.

## Error Codes

**New codes** (registered per constitution error code registry):
- `SQL_WRITE_BLOCKED` — destructive keyword detected, query not forwarded
- `EMPTY_QUERY` — SQL is empty or whitespace-only after comment stripping

## Constitution II Verification

No ObjectScript APIs used. This feature is pure Rust string processing. Constitution II N/A.
