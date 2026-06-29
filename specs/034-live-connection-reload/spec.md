# Feature Specification: Live Connection Hot-Reload and check_config Tool

**Feature Branch**: `034-live-connection-reload`  
**Created**: 2026-05-08  
**Status**: Implemented — merged to master  
**Closes**: #11 (iris_select_container discards new connection)

## Clarifications

### Session 2026-05-08

- Q: After `iris_select_container` switches the connection, what happens when `.iris-dev.toml` subsequently changes on disk? → A: File always wins — any `.iris-dev.toml` mtime change overrides a prior `iris_select_container` switch. Developer intent expressed via file edit takes precedence.
- Q: What should `write_tools_enabled` return in `check_config` immediately after a connection swap before a SystemMode probe completes? → A: Re-probe SystemMode on every connection swap — `write_tools_enabled` is always fresh after a hot-reload or `iris_select_container` switch. Stale values are unsafe for a production-safety gate.
- Q: What happens to an in-flight tool call when a hot-reload fires mid-execution? → A: In-flight calls complete on the old connection; the swap takes effect only for the next tool call. Interrupting a running operation would corrupt it.
- Q: Should config polling use a background task or lazy check on tool call entry? → A: Lazy check — `stat()` the config file at the start of each tool call handler; reload if mtime changed. Zero background tasks, no cancellation complexity, negligible syscall cost.

---

## Overview

Three related improvements that eliminate the need to restart the iris-dev MCP session when the target IRIS instance changes:

1. **Config hot-reload** — iris-dev detects when `.iris-dev.toml` changes on disk and silently reconnects to the new target without any session restart.
2. **Working `iris_select_container`** — fixes the long-standing bug where selecting a container probes it correctly but throws the result away; subsequent tool calls now use the new connection.
3. **`check_config` tool** — a new read-only tool agents can call at any time to see exactly which IRIS instance they're connected to, when the config was last loaded, and whether write tools are enabled.

---

## User Scenarios & Testing

### User Story 1 — Config file changes, session adapts silently (Priority: P1)

A developer updates `.iris-dev.toml` to point at a different IRIS container (e.g., switching from a downed enterprise container to a local community container). Without restarting their AI coding session, the next tool call automatically targets the new instance.

**Why this priority**: This is the most common friction point — restarting an AI session loses conversation context, loaded skills, and in-progress work. Silent reconnection removes that cost entirely.

**Independent Test**: Update `.iris-dev.toml` container name while a session is running. Call any iris-dev tool. Verify it targets the new container on that very call, not the old one.

**Acceptance Scenarios**:

1. **Given** a running iris-dev MCP session with a valid `.iris-dev.toml`, **When** the file is saved with a different `container` value, **Then** the very next tool call uses the new connection without any error or session restart.
2. **Given** a running session, **When** `.iris-dev.toml` is updated to point to an unreachable host, **Then** tool calls return `IRIS_UNREACHABLE` (graceful degradation, not a crash).
3. **Given** a session where hot-reload has switched connections twice, **When** `check_config` is called, **Then** it shows the most recently loaded configuration.
4. **Given** a session with no `.iris-dev.toml` (env-var-only config), **When** no config file exists to watch, **Then** the session behaves identically to today — no file watching attempted.

---

### User Story 2 — Agent explicitly switches containers mid-session (Priority: P1)

An agent working in a workspace calls `iris_select_container` to switch to a different running IRIS container. All subsequent tool calls in the session use the new container — no restart required.

**Why this priority**: Fixes issue #11 directly. Agents must be able to switch connections programmatically, especially when working across multiple namespaces or container environments in a single session.

**Independent Test**: Call `iris_select_container(name="gqs-ivg-test")`, then call `iris_execute(code="write $ZVersion,!")`. Verify the output is from `gqs-ivg-test`, not the previously active container.

**Acceptance Scenarios**:

1. **Given** a running session connected to container A, **When** `iris_select_container(name="container-B")` is called and container B is reachable, **Then** the response confirms the switch and all subsequent tool calls target container B.
2. **Given** a running session, **When** `iris_select_container` is called with a container name that doesn't exist or isn't reachable, **Then** the response includes an error and the existing connection is preserved unchanged.
3. **Given** a session that has been switched via `iris_select_container`, **When** `check_config` is called, **Then** it shows `"source": "iris_select_container"` and the new container's details.
4. **Given** a session switched via `iris_select_container`, **When** `.iris-dev.toml` is subsequently updated on disk, **Then** the config file change takes precedence and reconnects to the file-specified target.

---

### User Story 3 — Agent inspects active connection state (Priority: P2)

An agent calls `check_config` to understand exactly which IRIS instance it's currently connected to, diagnose why a tool returned unexpected results, or verify that a hot-reload completed successfully.

**Why this priority**: Agents have no way today to introspect connection state — they must infer it from tool outputs or error messages. `check_config` makes this explicit and auditable.

**Independent Test**: Call `check_config` immediately after session start. Verify it returns host, port, namespace, container name (if set), config file path, and when the connection was established.

**Acceptance Scenarios**:

1. **Given** a connected session, **When** `check_config` is called, **Then** it returns: `host`, `port`, `namespace`, `container` (or null), `config_file` (path or null), `config_loaded_at` (timestamp), `iris_version` (from last probe), `write_tools_enabled` (bool), and `connection_source` (`"config_file"` | `"env_vars"` | `"iris_select_container"` | `"auto_discovered"`).
2. **Given** a session with no IRIS connection (unreachable), **When** `check_config` is called, **Then** it returns the attempted connection details plus `"connected": false` — it never throws `IRIS_UNREACHABLE`.
3. **Given** a session where hot-reload just completed, **When** `check_config` is called, **Then** `config_loaded_at` reflects the reload time, not the original session start.

---

### Edge Cases

- `.iris-dev.toml` is deleted while a session is running — session keeps the last known connection, does not crash.
- `.iris-dev.toml` is written with invalid TOML syntax — session keeps the last known connection; `check_config` reports `"config_parse_error": "..."`.
- `iris_select_container` is called while a hot-reload is in progress — last writer wins; no deadlock.
- A tool call is in progress when a hot-reload fires — the in-flight call completes on the old connection; the new connection takes effect for the next call only.
- Session starts with `IRIS_CONTAINER` env var set AND a `.iris-dev.toml` — existing precedence rules preserved; `check_config` shows which source won.
- `check_config` is called when write tools are disabled (production instance) — still works; returns full state including `write_tools_enabled: false`.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: iris-dev MCP MUST check the active `.iris-dev.toml` file's modification time at the start of each tool call handler. If the mtime has changed since the last load, the config MUST be reloaded before the tool executes. No file-watching library or background task may be added as a dependency.
- **FR-002**: When a mtime change is detected, iris-dev MUST reload the config file, construct a new connection, probe it (including SystemMode for `write_tools_enabled`), and atomically replace the active connection. If the probe fails, the previous connection MUST be preserved.
- **FR-003**: Config hot-reload MUST be completely silent to the agent — no error, no notification, no session interruption. The agent learns of the change only by calling `check_config` or noticing changed tool behavior.
- **FR-004**: `iris_select_container` MUST update the active connection for the remainder of the session after a successful probe, including re-probing SystemMode to set `write_tools_enabled`. The previous behavior of probing but discarding the result MUST be eliminated.
- **FR-005**: If `iris_select_container` is called with an unreachable or unknown container, the active connection MUST remain unchanged and the tool MUST return an error identifying the problem.
- **FR-006**: A new `check_config` tool MUST be added that returns the active connection state without making any IRIS calls. It MUST always succeed (never return `IRIS_UNREACHABLE`).
- **FR-007**: `check_config` MUST return: `connected` (bool), `host`, `port`, `namespace`, `container` (string or null), `config_file` (path or null), `config_loaded_at` (ISO 8601 timestamp), `iris_version` (string or null), `write_tools_enabled` (bool), `connection_source` (one of: `"config_file"`, `"env_vars"`, `"iris_select_container"`, `"auto_discovered"`).
- **FR-008**: When no `.iris-dev.toml` exists (env-var-only or auto-discovery), no polling MUST be attempted and `check_config` MUST reflect `"config_file": null`.
- **FR-009**: A connection switch via `iris_select_container` MUST take priority over any pending config file hot-reload until the config file changes again after the switch.

### Key Entities

- **ConnectionState**: Snapshot of the active connection — host, port, namespace, container, source, timestamps.
- **ConfigWatcher**: Background state tracking the `.iris-dev.toml` path and last-seen mtime; triggers reload on change.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: After updating `.iris-dev.toml`, the new connection is active on the very next tool call — the reload is synchronous at tool call entry, so no polling delay exists.
- **SC-002**: `iris_select_container` followed immediately by any other tool call uses the new container in 100% of cases — verified by comparing `$ZVersion` output before and after switch.
- **SC-003**: `check_config` returns a complete, accurate connection snapshot in every session state (connected, disconnected, hot-reloaded, switched) with zero `IRIS_UNREACHABLE` errors.
- **SC-004**: Zero regressions in existing tool behavior — all existing iris-dev unit and E2E tests continue to pass.
- **SC-E2E**: End-to-end test confirms: start session → verify connection A → update config → make next tool call → verify connection B active via `check_config` and a live tool call (reload is synchronous at tool call entry — no wait required).

---

## Assumptions

- The MCP session runs as a single process; atomic connection swap uses a mutex rather than inter-process coordination.
- Polling at 2-second intervals is sufficient UX (file saves are human-speed); sub-second latency is not required.
- `iris_select_container` switch persists only for the session lifetime — it does not write back to `.iris-dev.toml`.
- When both `IRIS_CONTAINER` env var and `.iris-dev.toml` are present, existing precedence rules are preserved unchanged.
- `check_config` does not re-probe IRIS — it returns cached state from the last successful probe. If the last probe was at session start, `iris_version` may be stale; this is acceptable.
- The `connection_source` field reflects how the **currently active** connection was established, not how the session originally started.
