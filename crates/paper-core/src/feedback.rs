//! Disposition ledger for cuts and clips.
//!
//! Resolve and wontfix never rewrite the source JSONL logs. They append a
//! record to a separate feedback-resolutions ledger, and the latest record for
//! a given `feedback_id` is that item's current disposition. This mirrors the
//! canonical Python logger so both CLIs can operate on the same files.

use crate::error::{AppError, AppResult};
use crate::store::{self, ResolvedFile};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const FEEDBACK_FILE_ENV: &str = "FEEDBACK_RESOLUTIONS_FILE";

/// Dispositions that close an item. `deferred` is deliberately not terminal:
/// it records a decision to revisit, not an outcome.
pub const TERMINAL_DISPOSITIONS: [&str; 4] = ["resolved", "wontfix", "duplicate", "already_fixed"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackRecord {
    pub feedback_id: String,
    pub kind: String,
    pub disposition: String,
    #[serde(default)]
    pub resolution: Option<String>,
    #[serde(default)]
    pub linked_task: Option<String>,
    #[serde(default)]
    pub resolved_at: Option<String>,
    #[serde(default)]
    pub resolved_by: Option<String>,
    #[serde(default)]
    pub verification_method: Option<String>,
    #[serde(default)]
    pub verification_result: Option<String>,
    #[serde(default)]
    pub verified_at: Option<String>,
    #[serde(default)]
    pub change_class: Option<String>,
    #[serde(default)]
    pub changed_artifact: Option<String>,
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub next_review: Option<String>,
    #[serde(default)]
    pub recorded_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

impl FeedbackRecord {
    /// Build a record, preserving the original `recorded_at` when this item
    /// already has a disposition history (latest-wins semantics).
    pub fn new(
        feedback_id: &str,
        kind: &str,
        disposition: &str,
        now: &str,
        existing: Option<&FeedbackRecord>,
    ) -> Self {
        Self {
            feedback_id: feedback_id.to_string(),
            kind: kind.to_string(),
            disposition: disposition.to_string(),
            resolution: None,
            linked_task: None,
            resolved_at: Some(now.to_string()),
            resolved_by: None,
            verification_method: None,
            verification_result: None,
            verified_at: Some(now.to_string()),
            change_class: None,
            changed_artifact: None,
            channel: None,
            task_id: None,
            owner: None,
            reason: None,
            next_review: None,
            recorded_at: Some(
                existing
                    .and_then(|record| record.recorded_at.clone())
                    .unwrap_or_else(|| now.to_string()),
            ),
            updated_at: existing.map(|_| now.to_string()),
        }
    }
}

pub fn discover_feedback(source: &ResolvedFile) -> AppResult<PathBuf> {
    let cwd = std::env::current_dir().map_err(|error| AppError::from_io(error, Path::new(".")))?;
    if let Some(path) = std::env::var_os(FEEDBACK_FILE_ENV)
        && !path.is_empty()
    {
        return Ok(store::absolute(&cwd, PathBuf::from(path)));
    }
    if source.explicit
        && let Some(parent) = source.path.parent()
    {
        return Ok(parent.join(".feedback-resolutions.jsonl"));
    }
    if let Some(root) = source.repo.as_ref() {
        return Ok(root.join(".feedback-resolutions.jsonl"));
    }
    let home = std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| {
            AppError::config(
                "cannot resolve the home directory for the feedback ledger",
                "Set HOME, or set FEEDBACK_RESOLUTIONS_FILE, or pass --file PATH.",
            )
        })?;
    Ok(store::absolute(&cwd, home).join(".papercuts/feedback-resolutions.jsonl"))
}

/// Latest record per feedback id. Malformed lines are skipped, matching the
/// Python reader: a partial line must never take the whole ledger down.
pub fn load(path: &Path) -> HashMap<String, FeedbackRecord> {
    let mut latest = HashMap::new();
    let Ok(bytes) = std::fs::read(path) else {
        return latest;
    };
    for raw in bytes.split(|byte| *byte == b'\n') {
        if raw.is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_slice::<FeedbackRecord>(raw) else {
            continue;
        };
        latest.insert(record.feedback_id.clone(), record);
    }
    latest
}

pub fn append(path: &Path, record: &FeedbackRecord) -> AppResult<()> {
    store::with_exclusive(path, true, |file| {
        let bytes = store::read_bytes(file, path)?;
        store::append_json(file, path, &bytes, record)
    })
}

pub fn is_terminal(disposition: &str) -> bool {
    TERMINAL_DISPOSITIONS.contains(&disposition)
}

/// Map a terminal disposition onto a cut status. Non-terminal or absent
/// dispositions leave the status established by the source log alone.
pub fn cut_status(disposition: Option<&str>) -> Option<crate::ItemStatus> {
    match disposition? {
        "wontfix" => Some(crate::ItemStatus::Wontfix),
        "resolved" | "duplicate" | "already_fixed" => Some(crate::ItemStatus::Resolved),
        _ => None,
    }
}

pub fn clip_status(disposition: Option<&str>) -> Option<crate::ClipStatus> {
    match disposition? {
        "wontfix" => Some(crate::ClipStatus::Wontfix),
        "resolved" | "duplicate" | "already_fixed" => Some(crate::ClipStatus::Resolved),
        _ => None,
    }
}
