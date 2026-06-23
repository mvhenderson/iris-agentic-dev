//! UUID-keyed in-memory log store for progressive disclosure.
//!
//! When a tool (iris_compile, iris_search, iris_info, debug_get_error_logs) produces
//! output above its per-tool inline threshold, the full result is stored here under a
//! UUID and a compact summary is returned to the agent instead. The agent can retrieve
//! the full result via the `iris_get_log` tool.

use serde_json::Value;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use uuid::Uuid;

// ── LogEntry ─────────────────────────────────────────────────────────────────

/// One stored result entry.
pub struct LogEntry {
    pub id: String,
    pub tool: String,
    pub created_at: Instant,
    /// The inline preview — first `inline_count` items.
    pub preview: Vec<Value>,
    /// The complete result payload.
    pub full_result: Value,
    pub total_count: usize,
}

// ── LogSummary ───────────────────────────────────────────────────────────────

/// Compact listing returned by `iris_get_log` with no id parameter.
#[derive(serde::Serialize)]
pub struct LogSummary {
    pub id: String,
    pub tool: String,
    pub created_at: String,
    pub total_count: usize,
}

// ── GetResult ────────────────────────────────────────────────────────────────

pub enum GetResult {
    Found(Value),
    NotFound,
    Expired,
}

// ── LogStore ─────────────────────────────────────────────────────────────────

/// Process-global ring buffer of LogEntry values.
/// Owned as `Arc<Mutex<LogStore>>` on `IrisTools`.
pub struct LogStore {
    pub entries: VecDeque<LogEntry>,
    pub max_entries: usize,
    pub ttl_minutes: u64,
    /// Server start time — used to compute ISO timestamps from Instant offsets.
    start_time: std::time::SystemTime,
}

impl LogStore {
    pub fn new(max_entries: usize, ttl_minutes: u64) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_entries),
            max_entries,
            ttl_minutes,
            start_time: std::time::SystemTime::now(),
        }
    }

    /// Store a new entry.  Evicts oldest if at capacity.  Returns the entry id.
    pub fn store(&mut self, entry: LogEntry) -> String {
        let id = entry.id.clone();
        if self.entries.len() == self.max_entries {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
        id
    }

    /// Retrieve by id.  Does NOT evict — preserves LOG_EXPIRED vs LOG_NOT_FOUND distinction.
    pub fn get(&self, id: &str) -> GetResult {
        let ttl = Duration::from_secs(self.ttl_minutes * 60);
        match self.entries.iter().find(|e| e.id == id) {
            None => GetResult::NotFound,
            Some(e) => {
                if e.created_at.elapsed() > ttl {
                    GetResult::Expired
                } else {
                    GetResult::Found(e.full_result.clone())
                }
            }
        }
    }

    /// List all non-expired entries.  Calls evict_expired first.
    pub fn list(&mut self) -> Vec<LogSummary> {
        self.evict_expired();
        self.entries
            .iter()
            .map(|e| LogSummary {
                id: e.id.clone(),
                tool: e.tool.clone(),
                created_at: self.instant_to_iso(e.created_at),
                total_count: e.total_count,
            })
            .collect()
    }

    /// Remove entries past TTL.
    pub fn evict_expired(&mut self) {
        let ttl = Duration::from_secs(self.ttl_minutes * 60);
        self.entries.retain(|e| e.created_at.elapsed() <= ttl);
    }

    /// Retrieve a paginated slice from a stored entry's full_result array.
    /// Returns (items, has_more).  If full_result is not an array, returns it whole.
    pub fn get_paginated(
        &self,
        id: &str,
        limit: Option<usize>,
        offset: usize,
    ) -> Option<(Value, bool, usize)> {
        let ttl = Duration::from_secs(self.ttl_minutes * 60);
        let entry = self.entries.iter().find(|e| e.id == id)?;
        if entry.created_at.elapsed() > ttl {
            return None; // expired — caller checks GetResult separately
        }
        match limit {
            None => Some((entry.full_result.clone(), false, entry.total_count)),
            Some(lim) => {
                if let Some(arr) = entry.full_result.as_array() {
                    let slice: Vec<Value> = arr.iter().skip(offset).take(lim).cloned().collect();
                    let has_more = offset + lim < arr.len();
                    Some((Value::Array(slice), has_more, arr.len()))
                } else {
                    Some((entry.full_result.clone(), false, entry.total_count))
                }
            }
        }
    }

    fn instant_to_iso(&self, instant: Instant) -> String {
        // Compute how long ago this instant was relative to now, then subtract
        // from the current wall-clock time to get an approximate creation timestamp.
        let now_instant = Instant::now();
        let elapsed = if now_instant > instant {
            now_instant.duration_since(instant)
        } else {
            Duration::ZERO
        };
        let approx = std::time::SystemTime::now()
            .checked_sub(elapsed)
            .unwrap_or(self.start_time);
        let secs = approx
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        use chrono::{DateTime, Utc};
        let dt = DateTime::<Utc>::from_timestamp(secs as i64, 0)
            .unwrap_or_else(|| DateTime::from_timestamp(0, 0).unwrap());
        dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    }
}

// ── Helper: generate a log entry id ─────────────────────────────────────────

pub fn new_log_id() -> String {
    let ts_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let uid = Uuid::new_v4().to_string();
    // Take first 8 chars of UUID for brevity while keeping uniqueness
    let short = &uid[..8];
    format!("iris-{}-{}", ts_ms, short)
}

// ── Helper: read per-tool inline threshold ────────────────────────────────────

/// Read per-tool inline threshold from an env var at call time.
/// Falls back to `default` when the var is unset or unparseable.
/// Zero or negative → also returns default.
pub fn read_inline_threshold(env_var: &str, default: usize) -> usize {
    std::env::var(env_var)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(default)
}

// ── Helper: apply truncation to a JSON result ────────────────────────────────

/// Apply progressive disclosure to a JSON result value.
///
/// `items_key` — the key within `result` whose array length is counted and truncated.
/// `threshold` — number of items above which truncation activates.
/// `inline`    — if true, bypass the store and return everything inline.
/// `store`     — the LogStore to write to when truncation applies.
/// `tool`      — tool name stored in the LogEntry.
///
/// Mutates `result` in-place: truncates the array at `items_key` to `threshold` items,
/// then adds `truncated`, `log_id`, `inline_count`, `total_count` fields.
///
/// If `inline==true` or item count ≤ threshold, does nothing (additive fields not added).
pub fn apply_truncation(
    result: &mut Value,
    items_key: &str,
    threshold: usize,
    inline: bool,
    store: &std::sync::Arc<std::sync::Mutex<LogStore>>,
    tool: &str,
) {
    if inline {
        result["truncated"] = Value::Bool(false);
        return;
    }

    let items = match result.get(items_key).and_then(|v| v.as_array()) {
        Some(arr) => arr.clone(),
        None => return,
    };

    let total = items.len();
    if total <= threshold {
        result["truncated"] = Value::Bool(false);
        return;
    }

    // Truncate inline
    let preview: Vec<Value> = items[..threshold].to_vec();
    result[items_key] = Value::Array(preview.clone());

    // Store full result
    let id = new_log_id();
    let entry = LogEntry {
        id: id.clone(),
        tool: tool.to_string(),
        created_at: Instant::now(),
        preview: preview.clone(),
        full_result: Value::Array(items),
        total_count: total,
    };
    if let Ok(mut s) = store.lock() {
        s.store(entry);
    }

    result["truncated"] = Value::Bool(true);
    result["log_id"] = Value::String(id);
    result["inline_count"] = Value::Number(threshold.into());
    result["total_count"] = Value::Number(total.into());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn make_entry(id: &str, tool: &str, data: Value) -> LogEntry {
        LogEntry {
            id: id.to_string(),
            tool: tool.to_string(),
            created_at: Instant::now(),
            preview: vec![],
            full_result: data,
            total_count: 1,
        }
    }

    // ── LogStore::new ──────────────────────────────────────────────────────────

    #[test]
    fn new_creates_empty_store() {
        let store = LogStore::new(100, 60);
        assert!(store.entries.is_empty());
        assert_eq!(store.max_entries, 100);
        assert_eq!(store.ttl_minutes, 60);
    }

    // ── LogStore::store ───────────────────────────────────────────────────────

    #[test]
    fn store_adds_entry_and_returns_id() {
        let mut store = LogStore::new(100, 60);
        let entry = make_entry("test-id-1", "iris_compile", serde_json::json!({}));
        let id = store.store(entry);
        assert_eq!(id, "test-id-1");
        assert_eq!(store.entries.len(), 1);
    }

    #[test]
    fn store_evicts_oldest_at_capacity() {
        let mut store = LogStore::new(3, 60);
        store.store(make_entry("id-1", "tool", serde_json::json!(1)));
        store.store(make_entry("id-2", "tool", serde_json::json!(2)));
        store.store(make_entry("id-3", "tool", serde_json::json!(3)));
        assert_eq!(store.entries.len(), 3);
        // Adding a 4th evicts the first
        store.store(make_entry("id-4", "tool", serde_json::json!(4)));
        assert_eq!(store.entries.len(), 3);
        assert!(store.entries.iter().all(|e| e.id != "id-1"), "oldest evicted");
        assert!(store.entries.iter().any(|e| e.id == "id-4"));
    }

    // ── LogStore::get ─────────────────────────────────────────────────────────

    #[test]
    fn get_returns_found_for_existing_entry() {
        let mut store = LogStore::new(100, 60);
        store.store(make_entry("abc", "tool", serde_json::json!({"key": "val"})));
        match store.get("abc") {
            GetResult::Found(v) => assert_eq!(v["key"], "val"),
            _ => panic!("expected Found"),
        }
    }

    #[test]
    fn get_returns_not_found_for_missing_id() {
        let store = LogStore::new(100, 60);
        assert!(matches!(store.get("nonexistent"), GetResult::NotFound));
    }

    #[test]
    fn get_returns_expired_for_ttl_exceeded() {
        let mut store = LogStore::new(100, 60);
        // Use TTL of 0 minutes so everything is immediately expired
        let ttl_store = LogStore::new(100, 0);
        // Insert manually with a past Instant
        let id = "expire-me";
        let entry = LogEntry {
            id: id.to_string(),
            tool: "tool".to_string(),
            created_at: Instant::now() - Duration::from_secs(1),
            preview: vec![],
            full_result: serde_json::json!({}),
            total_count: 0,
        };
        let _ = &mut store;
        let mut zero_ttl = LogStore::new(100, 0);
        zero_ttl.entries.push_back(entry);
        assert!(matches!(zero_ttl.get(id), GetResult::Expired));
    }

    // ── LogStore::list ────────────────────────────────────────────────────────

    #[test]
    fn list_returns_non_expired_entries() {
        let mut store = LogStore::new(100, 60);
        store.store(make_entry("a", "iris_compile", serde_json::json!({})));
        store.store(make_entry("b", "iris_search", serde_json::json!({})));
        let summaries = store.list();
        assert_eq!(summaries.len(), 2);
        assert!(summaries.iter().any(|s| s.id == "a"));
        assert!(summaries.iter().any(|s| s.id == "b"));
    }

    #[test]
    fn list_returns_empty_for_empty_store() {
        let mut store = LogStore::new(100, 60);
        assert!(store.list().is_empty());
    }

    #[test]
    fn list_removes_expired_entries() {
        let mut store = LogStore::new(100, 0); // TTL=0 min → all expired immediately
        let entry = LogEntry {
            id: "exp-list".to_string(),
            tool: "tool".to_string(),
            created_at: Instant::now() - Duration::from_secs(1),
            preview: vec![],
            full_result: serde_json::json!({}),
            total_count: 0,
        };
        store.entries.push_back(entry);
        let summaries = store.list();
        assert!(summaries.is_empty(), "expired entries evicted from list");
        assert!(store.entries.is_empty(), "evict_expired called by list");
    }

    // ── LogStore::evict_expired ───────────────────────────────────────────────

    #[test]
    fn evict_expired_removes_old_keeps_fresh() {
        let mut store = LogStore::new(100, 0); // 0-minute TTL
        let fresh = LogEntry {
            id: "fresh".to_string(),
            tool: "t".to_string(),
            created_at: Instant::now() + Duration::from_secs(10), // future
            preview: vec![],
            full_result: serde_json::json!({}),
            total_count: 0,
        };
        let expired = LogEntry {
            id: "old".to_string(),
            tool: "t".to_string(),
            created_at: Instant::now() - Duration::from_secs(1),
            preview: vec![],
            full_result: serde_json::json!({}),
            total_count: 0,
        };
        store.entries.push_back(expired);
        store.entries.push_back(fresh);
        store.evict_expired();
        assert_eq!(store.entries.len(), 1);
        assert_eq!(store.entries[0].id, "fresh");
    }

    // ── LogStore::get_paginated ───────────────────────────────────────────────

    #[test]
    fn get_paginated_no_limit_returns_all() {
        let mut store = LogStore::new(100, 60);
        let data = serde_json::json!([1, 2, 3, 4, 5]);
        let entry = LogEntry {
            id: "pag-1".to_string(),
            tool: "t".to_string(),
            created_at: Instant::now(),
            preview: vec![],
            full_result: data,
            total_count: 5,
        };
        store.entries.push_back(entry);
        let (val, has_more, total) = store.get_paginated("pag-1", None, 0).unwrap();
        assert!(!has_more);
        assert_eq!(total, 5);
        assert_eq!(val.as_array().unwrap().len(), 5);
    }

    #[test]
    fn get_paginated_with_limit_and_offset() {
        let mut store = LogStore::new(100, 60);
        let data = serde_json::json!([10, 20, 30, 40, 50]);
        let entry = LogEntry {
            id: "pag-2".to_string(),
            tool: "t".to_string(),
            created_at: Instant::now(),
            preview: vec![],
            full_result: data,
            total_count: 5,
        };
        store.entries.push_back(entry);
        let (val, has_more, total) = store.get_paginated("pag-2", Some(2), 1).unwrap();
        let arr = val.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0], 20);
        assert_eq!(arr[1], 30);
        assert!(has_more, "items 3,4 remain");
        assert_eq!(total, 5);
    }

    #[test]
    fn get_paginated_returns_none_for_missing_id() {
        let store = LogStore::new(100, 60);
        assert!(store.get_paginated("no-such-id", None, 0).is_none());
    }

    #[test]
    fn get_paginated_returns_none_for_expired() {
        let mut store = LogStore::new(100, 0);
        let entry = LogEntry {
            id: "exp-pag".to_string(),
            tool: "t".to_string(),
            created_at: Instant::now() - Duration::from_secs(1),
            preview: vec![],
            full_result: serde_json::json!([1, 2, 3]),
            total_count: 3,
        };
        store.entries.push_back(entry);
        assert!(store.get_paginated("exp-pag", None, 0).is_none());
    }

    #[test]
    fn get_paginated_non_array_full_result_returns_whole() {
        let mut store = LogStore::new(100, 60);
        let data = serde_json::json!({"not": "an array"});
        let entry = LogEntry {
            id: "pag-obj".to_string(),
            tool: "t".to_string(),
            created_at: Instant::now(),
            preview: vec![],
            full_result: data.clone(),
            total_count: 1,
        };
        store.entries.push_back(entry);
        let (val, has_more, _) = store.get_paginated("pag-obj", Some(5), 0).unwrap();
        assert!(!has_more);
        assert_eq!(val, data);
    }

    // ── new_log_id ─────────────────────────────────────────────────────────────

    #[test]
    fn new_log_id_has_iris_prefix() {
        let id = new_log_id();
        assert!(id.starts_with("iris-"), "id: {id}");
    }

    #[test]
    fn new_log_id_is_unique() {
        let id1 = new_log_id();
        let id2 = new_log_id();
        assert_ne!(id1, id2);
    }

    // ── read_inline_threshold ─────────────────────────────────────────────────

    #[test]
    fn read_inline_threshold_returns_env_value() {
        std::env::set_var("TEST_INLINE_THRESH_XYZ", "42");
        let val = read_inline_threshold("TEST_INLINE_THRESH_XYZ", 10);
        std::env::remove_var("TEST_INLINE_THRESH_XYZ");
        assert_eq!(val, 42);
    }

    #[test]
    fn read_inline_threshold_returns_default_when_unset() {
        std::env::remove_var("TEST_INLINE_THRESH_UNSET_XYZ");
        assert_eq!(read_inline_threshold("TEST_INLINE_THRESH_UNSET_XYZ", 99), 99);
    }

    #[test]
    fn read_inline_threshold_returns_default_for_zero() {
        std::env::set_var("TEST_INLINE_THRESH_ZERO", "0");
        let val = read_inline_threshold("TEST_INLINE_THRESH_ZERO", 55);
        std::env::remove_var("TEST_INLINE_THRESH_ZERO");
        assert_eq!(val, 55, "zero should fall back to default");
    }

    #[test]
    fn read_inline_threshold_returns_default_for_invalid() {
        std::env::set_var("TEST_INLINE_THRESH_INVALID", "not-a-number");
        let val = read_inline_threshold("TEST_INLINE_THRESH_INVALID", 7);
        std::env::remove_var("TEST_INLINE_THRESH_INVALID");
        assert_eq!(val, 7);
    }

    // ── apply_truncation ──────────────────────────────────────────────────────

    #[test]
    fn apply_truncation_inline_mode_adds_truncated_false() {
        let store = Arc::new(Mutex::new(LogStore::new(100, 60)));
        let mut result = serde_json::json!({"items": [1, 2, 3, 4, 5]});
        apply_truncation(&mut result, "items", 3, true, &store, "tool");
        assert_eq!(result["truncated"], false);
        // inline=true: no log_id added
        assert!(result.get("log_id").is_none());
    }

    #[test]
    fn apply_truncation_no_key_does_nothing() {
        let store = Arc::new(Mutex::new(LogStore::new(100, 60)));
        let mut result = serde_json::json!({"other_key": [1, 2, 3]});
        apply_truncation(&mut result, "items", 2, false, &store, "tool");
        // No items key → nothing added
        assert!(result.get("truncated").is_none());
    }

    #[test]
    fn apply_truncation_below_threshold_adds_truncated_false() {
        let store = Arc::new(Mutex::new(LogStore::new(100, 60)));
        let mut result = serde_json::json!({"items": [1, 2]});
        apply_truncation(&mut result, "items", 5, false, &store, "tool");
        assert_eq!(result["truncated"], false);
        assert!(result.get("log_id").is_none());
    }

    #[test]
    fn apply_truncation_above_threshold_stores_and_truncates() {
        let store = Arc::new(Mutex::new(LogStore::new(100, 60)));
        let mut result = serde_json::json!({"items": [1, 2, 3, 4, 5]});
        apply_truncation(&mut result, "items", 2, false, &store, "iris_search");
        assert_eq!(result["truncated"], true);
        assert_eq!(result["inline_count"], 2);
        assert_eq!(result["total_count"], 5);
        let log_id = result["log_id"].as_str().expect("log_id set");
        assert!(!log_id.is_empty());
        // Items array truncated to 2
        assert_eq!(result["items"].as_array().unwrap().len(), 2);
        // Full result stored in log store
        let s = store.lock().unwrap();
        match s.get(log_id) {
            GetResult::Found(v) => {
                let arr = v.as_array().unwrap();
                assert_eq!(arr.len(), 5);
            }
            _ => panic!("expected full result in store"),
        }
    }

    // ── instant_to_iso (via list summaries) ──────────────────────────────────

    #[test]
    fn list_summary_created_at_is_iso_format() {
        let mut store = LogStore::new(100, 60);
        store.store(make_entry("ts-test", "tool", serde_json::json!({})));
        let summaries = store.list();
        assert_eq!(summaries.len(), 1);
        let ts = &summaries[0].created_at;
        // Should be ISO 8601: "2026-06-23T20:00:00Z" format
        assert!(ts.contains('T'), "timestamp: {ts}");
        assert!(ts.ends_with('Z'), "timestamp: {ts}");
    }
}
