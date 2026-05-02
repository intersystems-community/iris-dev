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
