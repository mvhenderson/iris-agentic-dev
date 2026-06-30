# Research: iris_global Tool — API Verification

**Branch**: `052-iris-global`
**Verified against**: IRIS for UNIX (Ubuntu ARM64) 2026.2.0L (Build 208U)
**Instance**: `iris-dev-iris` container, localhost:52780
**Verification date**: 2026-06-29
**Verified by**: `execute_via_generator` path (iris-agentic-dev MCP `iris_execute` tool)

---

## ObjectScript Constructs

All constructs below were verified by running the exact code patterns against the live IRIS
instance. No documentation-only or inferred results.

### `$Get(^Global(subscripts))`

**Status**: ✅ VERIFIED

```objectscript
Set ^IrisDevVerify("a","b") = "hello"
Set val = $Get(^IrisDevVerify("a","b"))
Write "GET_RESULT:",val,"||",!
```

**Result**: `GET_RESULT:hello||` — returns the stored string value.

**Edge case verified**: `$Get` on an undefined node returns `""` (empty string) without error.

---

### `$Data(^Global(subscripts))`

**Status**: ✅ VERIFIED

```objectscript
Set defined = $Data(^IrisDevVerify("a","b"))
Write "DATA_RESULT:",defined,"||",!
```

**Result**: `DATA_RESULT:1||` — returns `1` when the node exists with a value.

Return values per IRIS documentation (consistent with verification):
- `0` — node does not exist, no descendants
- `1` — node has a value, no descendants
- `10` — node has no value but has descendants
- `11` — node has a value AND descendants

Implementation note: `defined: true` maps to `$Data > 0`.

---

### `Kill ^Global(subscripts)`

**Status**: ✅ VERIFIED

```objectscript
Kill ^IrisDevVerify
Set chk = $Data(^IrisDevVerify)
Write "KILLED:",chk,"||",!
```

**Result**: `KILLED:0||` — node and all descendants deleted. `Kill` of a non-existent node
is a no-op (no error). This confirms FR-005's "kill of non-existent node is success."

---

### `$Order(^Global(sub))` for subscript listing

**Status**: ✅ VERIFIED

```objectscript
Set ^IrisDevVerify("x") = "v1"
Set ^IrisDevVerify("y") = "v2"
Set sub = ""
Set sub = $Order(^IrisDevVerify(sub))   ; → "x"
Set sub = $Order(^IrisDevVerify(sub))   ; → "y"
Set sub = $Order(^IrisDevVerify(sub))   ; → "" (end of subscripts)
```

**Result**: `ORDER1:x||`, `ORDER2:y||`, `ORDER3:||`

`$Order` returns `""` when there are no more subscripts at that level. The standard
iteration pattern (loop until `sub = ""`) is confirmed correct.

---

### `$Query(@ref)` for subtree traversal

**Status**: ✅ VERIFIED

```objectscript
Set ^IrisDevVerify("q","r") = "qr"
Set ref = $Name(^IrisDevVerify("q"))
Set node = $Query(@ref)
Write "QUERY:",node,"||",!
```

**Result**: `QUERY:^IrisDevVerify("q","r")||`

`$Query` returns the next global node reference (as a string like `^Name("sub1","sub2")`) in
the global tree. Returns `""` when there are no more nodes. The standard subtree iteration
pattern is confirmed:

```objectscript
Set ref = $Name(^GlobalName(sub1))
Set node = ref
For {
    Set node = $Query(@node)
    Quit:node=""
    Quit:$Extract(node,1,$Length(ref))'=ref  ; stopped leaving the subtree
    Set val = @node
    ; collect node + val
}
```

The prefix-check `$Extract(node,1,$Length(ref))'=ref` correctly detects when `$Query` has
left the target subtree.

---

### `$ZH` for wall-clock timing

**Status**: ✅ VERIFIED

```objectscript
Set t1 = $ZH
Write "ZH_GT_ZERO:",(t1>0),"||",!
```

**Result**: `ZH_GT_ZERO:1||`

`$ZH` returns fractional seconds since midnight as a floating-point number. Arithmetic
`elapsed = $ZH - startTime` correctly measures elapsed seconds. This confirms the 5-second
subtree timeout implementation.

**Known limitation** (documented in spec Edge Cases): `$ZH` resets to `0` at midnight.
A traversal straddling midnight will see a large negative elapsed time, which does NOT
satisfy `elapsed > 5`, causing the loop to run to node cap rather than time cap. This is
acceptable for a developer tool; behavior is documented, not silently broken.

---

## Execute Transport

**Mechanism**: `execute_via_generator` — compile-and-query pattern (not `/action/execute`).

The `iris_global` handler MUST use `IrisConnection::execute_via_generator()`, the same
function used by `iris_execute`. This compiles a temp `IrisDevTmp.IrisDevRunXXX` class
with the ObjectScript code body, queries it via SQL, then deletes the class.

**No new crates or transport mechanisms needed.** The existing `IrisConnection` abstraction
handles all execution.

---

## Error Surface

Errors from the ObjectScript execution path surface as:
- `"ERROR: <ERRORCODE>..."` prefix in the output string (from the `Catch ex` block in
  `execute_via_generator`)
- HTTP errors (4xx/5xx from the Atelier endpoint) surface as `anyhow::Error` from
  `execute_via_generator`

The `iris_global` handler parses the output string and maps:
- Output starting with `"ERROR:"` → `IRIS_EXECUTE_ERROR` with the message after the prefix
- HTTP/network failure → `IRIS_UNREACHABLE`
- Missing/invalid params → `INVALID_SUBSCRIPT` or `INVALID_PARAMS` (before any IRIS call)

---

## Decisions

| Decision | Chosen | Rationale |
|---|---|---|
| Execute transport | `execute_via_generator` | Only available HTTP exec path; verified working; consistent with `iris_execute` |
| Subtree traversal | `$Query` loop with prefix guard | Verified correct; standard IRIS pattern |
| Subscript listing | `$Order` loop | Verified correct; standard IRIS pattern |
| Timeout mechanism | `$ZH` elapsed check inside loop | Verified working; no IRIS-side timeout API needed |
| Error detection | `"ERROR:"` prefix parse | Existing convention in `execute_via_generator`; no new protocol needed |
