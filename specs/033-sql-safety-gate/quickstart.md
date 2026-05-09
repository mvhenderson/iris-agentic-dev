# Quickstart: iris_query SQL Safety Gate

## Normal use (validation active by default)
```json
{"name": "iris_query", "arguments": {"query": "SELECT ID, Name FROM MyApp.Patient TOP 10"}}
```
→ Returns rows normally.

## Blocked query
```json
{"name": "iris_query", "arguments": {"query": "DELETE FROM MyApp.Patient WHERE Status = 'test'"}}
```
→ Returns `{"success": false, "error_code": "SQL_WRITE_BLOCKED", "blocked_keyword": "DELETE"}`.
No network call to IRIS is made.

## Force bypass (admin use only)
```json
{"name": "iris_query", "arguments": {
  "query": "DELETE FROM MyApp.Temp WHERE Session = '12345'",
  "force": true
}}
```
→ Forwards to IRIS. IRIS may still reject if the user lacks DELETE permission.

## Comment stripping example
```json
{"name": "iris_query", "arguments": {"query": "/* cleanup */ SELECT * FROM MyApp.Log"}}
```
→ Comment stripped → `SELECT * FROM MyApp.Log` → allowed, returns rows.
