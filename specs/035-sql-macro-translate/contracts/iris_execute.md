# Contract: iris_execute (updated — translate_sql parameter)

## Change from previous behavior
Added `translate_sql: bool` (default `true`). When `true` and code contains `&sql(...)` macros, they are rewritten to `%SQL.Statement` calls before execution. Response gains `sql_translated`, `translated_code`, and optionally `translation_warning`.

## Request Parameters (updated)
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| code | string | yes | — | ObjectScript code to execute |
| namespace | string | no | "USER" | IRIS namespace |
| timeout | int | no | 30 | Execution timeout in seconds |
| translate_sql | bool | no | **true** | If true, rewrite `&sql(...)` macros before executing |

## Response — Success, no &sql
```json
{"success": true, "output": "...", "namespace": "USER", "method": "http"}
```
Identical to current behavior.

## Response — Success, &sql translated
```json
{
  "success": true,
  "output": "...",
  "namespace": "USER",
  "method": "http",
  "sql_translated": true,
  "translated_code": "set _sqlrs1 = ##class(%SQL.Statement).%New()\n..."
}
```

## Response — Success, &sql translated with warnings
```json
{
  "success": true,
  "output": "...",
  "sql_translated": true,
  "translated_code": "...",
  "translation_warning": ["&sql(CALL MyProc()) at line 5 was not translated (CALL statements are not supported — use ##class(MyProc).Execute() directly)"]
}
```

## Response — translate_sql: false
```json
{"success": false, "error_code": "EXECUTION_FAILED", "error": "...IRIS error about &sql..."}
```
`sql_translated` absent. Raw IRIS error returned.
