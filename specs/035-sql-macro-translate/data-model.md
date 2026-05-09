# Data Model: iris_execute &sql Macro Translation

## New Types

### TranslationResult
Output of `translate_sql_macros()`.

| Field | Type | Description |
|-------|------|-------------|
| `translated_code` | `String` | Full rewritten code (equals input if `found=false`) |
| `found` | `bool` | Whether any `&sql(...)` macros were present |
| `warnings` | `Vec<String>` | Descriptions of untranslatable constructs left in place |

### Modified Types

### ExecuteParams (modified)
| Field | Before | After |
|-------|--------|-------|
| `translate_sql` | *(new)* | `bool`, default `true` — if true, translate `&sql(...)` before executing |

All other fields unchanged.

## Error Codes (no new codes)
Translation fallback does not produce new error codes — untranslatable constructs produce `translation_warning` in the response, not an error code.

## Response Fields (additive)

When translation fires (`translate_sql: true` and `found: true`):
- `sql_translated: true` — signals translation occurred
- `translated_code: String` — the rewritten ObjectScript sent to IRIS
- `translation_warning: String[]` (optional) — descriptions of skipped constructs

When no translation needed:
- None of the above fields appear — response identical to current behavior
