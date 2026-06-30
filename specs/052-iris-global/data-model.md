# Data Model: iris_global Tool

**Branch**: `052-iris-global`

---

## Error Code Registry

New error codes introduced by this feature (per constitution Error Code Registry clause):

| Code | Severity | Meaning | Key Fields |
|---|---|---|---|
| `INVALID_SUBSCRIPT` | Client error | A subscript value failed the allowlist check `^[a-zA-Z0-9 _.:\-]+$` | `subscript` (the failing value), `pattern` (the allowlist regex) |

**Inherited codes** (no re-definition; listed for reference):

| Code | From | Meaning |
|---|---|---|
| `IRIS_UNREACHABLE` | Standard | HTTP 5xx or network failure reaching IRIS |
| `IRIS_EXECUTE_ERROR` | Standard | ObjectScript `CATCH` block triggered; `message` contains IRIS error string |
| `ENV_GATE_BLOCKED` | 051 | `mcpTemplate` blocks the tool's action category |
| `DATA_POLICY_BLOCKED` | 051 | `dataPolicy=block` on bulk-PHI tool |
| `SYSTEM_BLOCKLIST` | 051 | Global name matches system or custom blocklist |
| `PHI_GATE_BLOCKED` | 051 | Global name matches PHI pattern; pass `acknowledgePhi=true` to bypass |
| `INVALID_PARAMS` | Standard | Required parameter missing (e.g. `value` absent on `set`) |

---

## Response Shapes

### `action=get` — single node

```json
{
  "success": true,
  "value": "string_or_null",
  "defined": true
}
```

| Field | Type | Notes |
|---|---|---|
| `success` | bool | Always `true` on success |
| `value` | string \| null | Node value; `null` when `defined: false` |
| `defined` | bool | `true` if `$Data > 0` |

### `action=get` with `subtree: true`

```json
{
  "success": true,
  "nodes": [
    {"path": "^MyApp(\"a\",\"b\")", "value": "v"}
  ],
  "truncated": false,
  "node_count": 1
}
```

| Field | Type | Notes |
|---|---|---|
| `success` | bool | Always `true` on success |
| `nodes` | array | Array of `{path, value}` objects |
| `nodes[].path` | string | Full global reference as returned by `$Query` |
| `nodes[].value` | string | Node value |
| `truncated` | bool | `true` when `max_nodes` or 5s timeout was hit |
| `node_count` | integer | Number of nodes returned |

### `action=set`

```json
{
  "success": true
}
```

### `action=kill`

```json
{
  "success": true
}
```

### `action=list`

```json
{
  "success": true,
  "subscripts": ["a", "b", "c"],
  "truncated": false
}
```

| Field | Type | Notes |
|---|---|---|
| `success` | bool | Always `true` on success |
| `subscripts` | string[] | First-level subscript values at the specified node |
| `truncated` | bool | `true` when `max_subscripts` cap was hit |

### Error response (all actions)

```json
{
  "success": false,
  "error_code": "INVALID_SUBSCRIPT",
  "message": "subscript 'bad\"char' contains disallowed characters",
  "subscript": "bad\"char",
  "pattern": "^[a-zA-Z0-9 _.:\\-]+$"
}
```

All error responses include at minimum `success: false`, `error_code`, and `message`.
Additional fields depend on the error code (see registry above).

---

## Tool Parameters

| Parameter | Type | Required | Default | Notes |
|---|---|---|---|---|
| `action` | enum | ✅ | — | `get`, `set`, `kill`, `list` |
| `global_name` | string | ✅ | — | With or without leading `^` |
| `subscripts` | string[] | — | `[]` | Each must match allowlist |
| `value` | string | set only | — | Required for `set`; ignored for others |
| `namespace` | string | — | connection default | IRIS namespace |
| `subtree` | bool | — | `false` | `get` only: return full subtree |
| `max_nodes` | integer | — | `100` | `get+subtree` only; clamped to 1000 |
| `max_subscripts` | integer | — | `50` | `list` only; clamped to 500 |
| `acknowledgePhi` | bool | — | `false` | Bypass PHI name gate (per spec 051) |
