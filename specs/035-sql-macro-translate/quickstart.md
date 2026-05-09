# Quickstart: &sql Macro Translation

## It just works (default behavior)
```json
{"name": "iris_execute", "arguments": {
  "code": "set id=\"%ASQ.AST\"\nset name=\"\"\n&sql(SELECT Name INTO :name FROM %Dictionary.ClassDefinition WHERE ID = :id)\nwrite name,!"
}}
```
Response:
```json
{"success": true, "output": "%ASQ.AST", "sql_translated": true, "translated_code": "..."}
```

## Inspect what was sent to IRIS
The `translated_code` field shows the exact ObjectScript executed:
```objectscript
set _sqlrs1 = ##class(%SQL.Statement).%New()
set _sqlsc1 = _sqlrs1.%Prepare("SELECT Name FROM %Dictionary.ClassDefinition WHERE ID = ?")
set _sqlrs1 = _sqlrs1.%Execute(id)
if _sqlrs1.%Next() {
  set name = _sqlrs1.%Get("Name")
} else {
  set name = ""
  set _sqlSQLCODE1 = _sqlrs1.%SQLCODE
}
```

## Opt out to debug
```json
{"name": "iris_execute", "arguments": {
  "code": "...",
  "translate_sql": false
}}
```
Code sent as-is — raw IRIS error for `&sql`.

## DML example
```json
{"name": "iris_execute", "arguments": {
  "code": "&sql(INSERT INTO MyApp.Log (Msg) VALUES (:msg))\nset msg=\"hello\""
}}
```
Translates to:
```objectscript
set _sqlrs1 = ##class(%SQL.Statement).%ExecDirect(, "INSERT INTO MyApp.Log (Msg) VALUES (?)", msg)
```
