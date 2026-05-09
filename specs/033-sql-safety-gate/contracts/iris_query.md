# Contract: iris_query (updated)

## Tool Description (updated)
Execute a SQL SELECT query against IRIS and return rows as JSON. By default, destructive SQL
(DROP, DELETE, INSERT, UPDATE, ALTER, CREATE, MERGE, TRUNCATE, EXEC, EXECUTE, BULK, LOAD, KILL, LOCK,
SELECT INTO) is blocked before reaching IRIS. Set `force: true` to bypass validation — use only for
administrative tasks where destructive SQL is intentional.

## Request Parameters

| Parameter  | Type    | Required | Default | Description |
|------------|---------|----------|---------|-------------|
| query      | string  | yes      | —       | SQL query to execute |
| parameters | string[] | no      | []      | Positional parameters (replace `?` placeholders) |
| namespace  | string  | no       | "USER"  | IRIS namespace |
| force      | bool    | no       | false   | Bypass SQL safety validation |

## Response — Success
```json
{
  "success": true,
  "rows": [...],
  "count": 42,
  "namespace": "USER"
}
```

## Response — SQL_WRITE_BLOCKED
```json
{
  "success": false,
  "error_code": "SQL_WRITE_BLOCKED",
  "error": "Destructive SQL keyword 'DROP' is not allowed. Use force: true to override.",
  "blocked_keyword": "DROP"
}
```

## Response — EMPTY_QUERY
```json
{
  "success": false,
  "error_code": "EMPTY_QUERY",
  "error": "SQL query is empty after removing comments."
}
```

## Response — SQL_ERROR (unchanged)
```json
{
  "success": false,
  "error_code": "SQL_ERROR",
  "error": "<IRIS error message>"
}
```
