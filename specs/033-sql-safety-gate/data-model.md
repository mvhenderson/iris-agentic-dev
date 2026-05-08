# Data Model: iris_query Read-Only SQL Safety Gate

## Types

### ValidationResult
```
Ok(())           — SQL is safe to forward
Err(String)      — blocked_keyword (e.g., "DROP", "DELETE", "SELECT INTO")
```

### QueryParams (updated)
| Field       | Type   | Default | Description |
|-------------|--------|---------|-------------|
| query       | String | —       | SQL query string |
| parameters  | Vec<String> | [] | Positional query parameters |
| namespace   | String | "USER"  | IRIS namespace |
| force       | bool   | false   | If true, skip SQL safety validation |

## Error Codes (new)

| Code | When | Response fields |
|------|------|----------------|
| `SQL_WRITE_BLOCKED` | Destructive keyword detected | `success: false`, `error_code`, `error` (human message), `blocked_keyword` |
| `EMPTY_QUERY` | SQL empty after comment strip | `success: false`, `error_code`, `error` |

## validate_read_only_sql() Contract

**Input**: `sql: &str`  
**Output**: `Result<(), String>` — Ok if safe, Err(keyword) if blocked  
**Side effects**: None — pure function  
**Performance**: O(n) in SQL length, < 1ms for typical queries up to 100KB

### Processing pipeline
1. Strip `/* ... */` block comments
2. Strip `-- ...` line comments  
3. Check for empty/whitespace-only → Err("EMPTY")
4. Walk remaining characters, skip `'...'` and `"..."` quoted content
5. For each unquoted token: check against blocked keyword list (case-insensitive, word-boundary)
6. Check for `SELECT ... INTO <identifier>` pattern
7. If any check fails: return Err(keyword)
8. Return Ok(())
