use jiff::Timestamp;
use paper_core::error::{AppError, AppResult};
use paper_core::feedback::{self, FeedbackRecord};
use paper_core::output::{self, Meta};
use paper_core::store;
use paper_core::{
    ClipRecord, ClipStatus, Impact, compute_clip_id, format_timestamp, normalize_where,
    resolve_agent,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{IsTerminal, Read};
use std::path::PathBuf;

use crate::{OutputFormat, ResolveArgs, StatusFilter, WontfixArgs};

#[derive(Debug, Serialize, Deserialize)]
pub struct ClipAddData {
    pub changed: bool,
    pub record: ClipRecord,
}

#[allow(clippy::too_many_arguments)]
pub fn add(
    text: Option<String>,
    agent: Option<String>,
    tags: Vec<String>,
    impact: Impact,
    dry_run: bool,
    force: bool,
    where_loc: Option<String>,
    file: Option<PathBuf>,
    pretty: bool,
    now: Timestamp,
) -> AppResult<i32> {
    let resolved = store::discover_clips(file)?;
    let text = if let Some(t) = text {
        t
    } else if std::io::stdin().is_terminal() {
        return Err(AppError::invalid_argument(
            "TEXT is required when stdin is not piped",
            "Pass TEXT as a positional argument or pipe it on stdin.",
        ));
    } else {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| AppError::from_io(e, std::path::Path::new("stdin")))?;
        buf
    };
    if text.trim().is_empty() {
        return Err(AppError::invalid_input(
            "clip text cannot be empty",
            "Pass non-empty TEXT.",
        ));
    }
    if !force && let Some(pattern) = paper_core::secrets::scan(&text) {
        return Err(AppError::secret_detected(pattern));
    }
    let (agent, _source) = resolve_agent(agent);
    let mut tags = tags;
    tags.sort();
    let ts = format_timestamp(now);
    let where_normalized = normalize_where(where_loc);
    let record = ClipRecord {
        kind: "clip".into(),
        id: compute_clip_id(&ts, &agent, &text, impact, &tags),
        ts,
        agent,
        text,
        tags,
        impact,
        where_loc: where_normalized,
        cwd: store::repo_relative(
            resolved.repo.as_deref(),
            &std::env::current_dir()
                .map_err(|e| AppError::from_io(e, std::path::Path::new(".")))?,
        ),
        repo: resolved.repo.as_ref().map(|_| ".".to_string()),
    };
    let mut warnings = Vec::new();
    let (changed, record) = if dry_run {
        warnings.push("dry run; no record appended".into());
        (false, record)
    } else {
        store::with_exclusive(&resolved.path, true, |log| {
            let bytes = store::read_bytes(log, &resolved.path)?;
            let folded = store::fold_clip_bytes(&bytes);
            let duplicate = folded.items.iter().any(|item| item.clip.id == record.id);
            if !duplicate {
                store::append_json(log, &resolved.path, &bytes, &record)?;
            }
            Ok((!duplicate, record))
        })?
    };
    let mut meta = Meta::new();
    meta.file = Some(resolved.path.to_string_lossy().into_owned());
    meta.warnings = warnings;
    output::write_success(ClipAddData { changed, record }, pretty, meta)
        .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
    Ok(0)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClipListData {
    pub items: Vec<paper_core::ClipListItem>,
    pub count: usize,
    pub total: usize,
}

#[allow(clippy::too_many_arguments)]
pub fn list(
    tag: Option<String>,
    impact: Option<Impact>,
    status_filter: StatusFilter,
    limit: usize,
    _format: OutputFormat,
    where_loc: Option<String>,
    unresolved: bool,
    file: Option<PathBuf>,
    pretty: bool,
) -> AppResult<i32> {
    let resolved = store::discover_clips(file)?;
    let feedback_path = feedback::discover_feedback(&resolved)?;
    let folded = match store::with_shared(&resolved.path, |log| {
        let bytes = store::read_bytes(log, &resolved.path)?;
        Ok(store::fold_clip_bytes(&bytes))
    }) {
        Ok(f) => f,
        Err(e) if e.code == "not_found" => {
            let meta = Meta::new();
            output::write_success(
                ClipListData {
                    items: vec![],
                    count: 0,
                    total: 0,
                },
                pretty,
                meta,
            )
            .map_err(|e2| AppError::from_io(e2, std::path::Path::new("stdout")))?;
            return Ok(0);
        }
        Err(e) => return Err(e),
    };
    let dispositions = feedback::load(&feedback_path);
    let mut items: Vec<_> = folded
        .items
        .into_iter()
        .map(|mut item| {
            if let Some(status) =
                feedback::clip_status(disposition_of(&dispositions, &item.clip.id))
            {
                item.status = status;
            }
            item
        })
        .filter(|item| match status_filter {
            StatusFilter::Open => item.status == ClipStatus::Open,
            StatusFilter::Promoted => item.status == ClipStatus::Promoted,
            StatusFilter::Noted => item.status == ClipStatus::Noted,
            StatusFilter::Resolved => item.status == ClipStatus::Resolved,
            StatusFilter::Wontfix => item.status == ClipStatus::Wontfix,
            StatusFilter::All => true,
        })
        .filter(|item| {
            !unresolved
                || (item.status == ClipStatus::Open
                    && !feedback::is_terminal(
                        disposition_of(&dispositions, &item.clip.id).unwrap_or("open"),
                    ))
        })
        .filter(|item| tag.as_ref().is_none_or(|t| item.clip.tags.contains(t)))
        .filter(|item| impact.as_ref().is_none_or(|i| item.clip.impact == *i))
        .filter(|item| {
            where_loc
                .as_ref()
                .is_none_or(|w| item.clip.where_loc.as_deref() == Some(w.as_str()))
        })
        .collect();
    let total = items.len();
    items.truncate(limit);
    let count = items.len();
    let meta = Meta::new();
    output::write_success(
        ClipListData {
            items,
            count,
            total,
        },
        pretty,
        meta,
    )
    .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
    Ok(0)
}

pub fn promote(
    id: String,
    agent: Option<String>,
    file: Option<PathBuf>,
    pretty: bool,
    now: Timestamp,
) -> AppResult<i32> {
    let resolved = store::discover_clips(file)?;
    let (agent, _source) = resolve_agent(agent);
    let ts = format_timestamp(now);
    let (changed, full_id) = store::with_exclusive(&resolved.path, false, |log| {
        let bytes = store::read_bytes(log, &resolved.path)?;
        let folded = store::fold_clip_bytes(&bytes);
        let item = folded
            .items
            .iter()
            .find(|i| i.clip.id.starts_with(&id))
            .ok_or_else(|| {
                AppError::not_found(
                    format!("clip not found: {}", id),
                    "Run `paperclip list` to find valid IDs.",
                )
            })?;
        let full_id = item.clip.id.clone();
        if item.status == ClipStatus::Promoted {
            return Ok((false, full_id));
        }
        let event = serde_json::json!({
            "kind": "promote",
            "id": full_id,
            "ts": ts,
            "agent": agent,
        });
        store::append_json(log, &resolved.path, &bytes, &event)?;
        Ok((true, full_id))
    })?;
    let mut meta = Meta::new();
    if !changed {
        meta.warnings.push("already promoted".into());
    }
    output::write_success(
        serde_json::json!({"changed": changed, "id": full_id, "status": "promoted"}),
        pretty,
        meta,
    )
    .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
    Ok(0)
}

pub fn note(
    id: String,
    text: String,
    agent: Option<String>,
    file: Option<PathBuf>,
    pretty: bool,
    now: Timestamp,
) -> AppResult<i32> {
    let resolved = store::discover_clips(file)?;
    let (agent, _source) = resolve_agent(agent);
    let ts = format_timestamp(now);
    let (full_id, notes) = store::with_exclusive(&resolved.path, false, |log| {
        let bytes = store::read_bytes(log, &resolved.path)?;
        let folded = store::fold_clip_bytes(&bytes);
        let item = folded
            .items
            .iter()
            .find(|i| i.clip.id.starts_with(&id))
            .ok_or_else(|| {
                AppError::not_found(
                    format!("clip not found: {}", id),
                    "Run `paperclip list` to find valid IDs.",
                )
            })?;
        let full_id = item.clip.id.clone();
        let notes = item.notes.len() + 1;
        let event = serde_json::json!({
            "kind": "note",
            "id": full_id,
            "ts": ts,
            "agent": agent,
            "text": text,
        });
        store::append_json(log, &resolved.path, &bytes, &event)?;
        Ok((full_id, notes))
    })?;
    let meta = Meta::new();
    output::write_success(
        serde_json::json!({"changed": true, "id": full_id, "notes": notes}),
        pretty,
        meta,
    )
    .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
    Ok(0)
}

pub fn top(limit: usize, file: Option<PathBuf>, pretty: bool) -> AppResult<i32> {
    let resolved = store::discover_clips(file)?;
    let folded = match store::with_shared(&resolved.path, |log| {
        let bytes = store::read_bytes(log, &resolved.path)?;
        Ok(store::fold_clip_bytes(&bytes))
    }) {
        Ok(f) => f,
        Err(e) if e.code == "not_found" => {
            let meta = Meta::new();
            output::write_success(serde_json::json!({"items": [], "count": 0}), pretty, meta)
                .map_err(|e2| AppError::from_io(e2, std::path::Path::new("stdout")))?;
            return Ok(0);
        }
        Err(e) => return Err(e),
    };
    use std::collections::BTreeMap;
    let mut by_tag: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_where: BTreeMap<String, usize> = BTreeMap::new();
    for item in &folded.items {
        if item.status == ClipStatus::Open {
            for tag in &item.clip.tags {
                *by_tag.entry(tag.clone()).or_insert(0) += 1;
            }
            if let Some(ref w) = item.clip.where_loc {
                *by_where.entry(w.clone()).or_insert(0) += 1;
            }
        }
    }
    let top_tags: Vec<_> = by_tag.into_iter().rev().take(limit).collect();
    let top_wheres: Vec<_> = by_where.into_iter().rev().take(limit).collect();
    let meta = Meta::new();
    output::write_success(
        serde_json::json!({
            "by_tag": top_tags,
            "by_where": top_wheres,
            "total_open": folded.items.iter().filter(|i| i.status == ClipStatus::Open).count(),
        }),
        pretty,
        meta,
    )
    .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
    Ok(0)
}

pub fn doctor(file: Option<PathBuf>, pretty: bool) -> AppResult<i32> {
    let resolved = store::discover_clips(file)?;
    let folded = match store::with_shared(&resolved.path, |log| {
        let bytes = store::read_bytes(log, &resolved.path)?;
        Ok(store::fold_clip_bytes(&bytes))
    }) {
        Ok(f) => f,
        Err(e) if e.code == "not_found" => {
            let meta = Meta::new();
            output::write_success(
                serde_json::json!({"healthy": true, "findings": [], "checked_lines": 0}),
                pretty,
                meta,
            )
            .map_err(|e2| AppError::from_io(e2, std::path::Path::new("stdout")))?;
            return Ok(0);
        }
        Err(e) => return Err(e),
    };
    let mut findings = Vec::new();
    for w in &folded.warnings {
        findings.push(serde_json::json!({"kind": "warning", "message": w}));
    }
    let healthy = findings.is_empty();
    let meta = Meta::new();
    output::write_success(
        serde_json::json!({
            "healthy": healthy,
            "findings": findings,
            "total_clips": folded.items.len(),
        }),
        pretty,
        meta,
    )
    .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
    Ok(i32::from(!healthy))
}

fn disposition_of<'a>(
    dispositions: &'a HashMap<String, FeedbackRecord>,
    id: &str,
) -> Option<&'a str> {
    dispositions
        .get(id)
        .map(|record| record.disposition.as_str())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClipResolveData {
    pub changed: bool,
    pub id: String,
    pub feedback: FeedbackRecord,
}

fn capa_errors(
    resolution: Option<&str>,
    change: Option<&str>,
    resolved_by: Option<&str>,
    owner: Option<&str>,
    verification_method: Option<&str>,
    verification_result: Option<&str>,
) -> Vec<String> {
    let mut errors = Vec::new();
    if verification_method.is_none_or(|value| value.trim().is_empty()) {
        errors.push("--verification-method is required (how was effectiveness verified?)".into());
    }
    if verification_result.is_none_or(|value| value.trim().is_empty()) {
        errors.push("--verification-result is required (what did the verification find?)".into());
    }
    let has_resolution = resolution.is_some_and(|value| !value.trim().is_empty())
        || change.is_some_and(|value| !value.trim().is_empty());
    if !has_resolution {
        errors.push("--resolution is required (or legacy --change naming the fix)".into());
    }
    let has_resolver = resolved_by.is_some_and(|value| !value.trim().is_empty())
        || owner.is_some_and(|value| !value.trim().is_empty());
    if !has_resolver {
        errors.push("--resolved-by is required (or legacy --owner naming the resolver)".into());
    }
    errors
}

pub fn resolve(
    args: ResolveArgs,
    file: Option<PathBuf>,
    pretty: bool,
    now: Timestamp,
) -> AppResult<i32> {
    let prefix = normalize_clip_prefix(&args.id)?;
    let resolved = store::discover_clips(file)?;
    let feedback_path = feedback::discover_feedback(&resolved)?;
    let errors = capa_errors(
        args.resolution.as_deref(),
        args.change.as_deref(),
        args.resolved_by.as_deref(),
        args.owner.as_deref(),
        args.verification_method.as_deref(),
        args.verification_result.as_deref(),
    );
    if !errors.is_empty() {
        let mut message = format!(
            "resolve rejected by the CAPA effectiveness gate for {}:\n  - {}",
            args.id,
            errors.join("\n  - ")
        );
        message.push_str(
            "\n  A link without verification reproduces the exact failure it is meant to catch.\n  Use wontfix --reason if this is a deliberate no-action disposition.",
        );
        return Err(AppError::invalid_input(
            message,
            "Pass --verification-method, --verification-result, --resolution (or --change), and --resolved-by (or --owner).",
        ));
    }
    let (agent, _source) = resolve_agent(args.agent);
    let ts = format_timestamp(now);
    let id = match store::with_shared(&resolved.path, |log| {
        let bytes = store::read_bytes(log, &resolved.path)?;
        let folded = store::fold_clip_bytes(&bytes);
        match_clip(&prefix, &folded.items)
    }) {
        Ok(id) => id,
        Err(error) if error.code == "not_found" && error.exit_code == 66 => {
            return Err(AppError::not_found(
                format!("paperclips file not found: {}", resolved.path.display()),
                "Run `paperclip list --status all` to find a clip ID first.",
            ));
        }
        Err(error) => return Err(error),
    };

    let existing = feedback::load(&feedback_path);
    let mut record = FeedbackRecord::new(&id, "clip", "resolved", &ts, existing.get(&id));
    record.resolution = args
        .resolution
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            args.change
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .map(|value| format!("fixed via {value}"))
        });
    record.linked_task = normalize_task(args.linked_task.as_deref().or(args.task.as_deref()));
    record.resolved_by = args
        .resolved_by
        .filter(|value| !value.trim().is_empty())
        .or_else(|| args.owner.clone().filter(|value| !value.trim().is_empty()));
    record.verification_method = args.verification_method;
    record.verification_result = args.verification_result;
    record.change_class = args.change_class.map(|value| value.as_str().to_string());
    record.changed_artifact = args.change;
    record.channel = args.channel.map(|value| value.as_str().to_string());
    record.task_id = args.task;
    record.owner = args.owner;
    let _ = agent;

    let changed = !args.dry_run;
    if changed {
        feedback::append(&feedback_path, &record)?;
    }
    let mut meta = Meta::new();
    meta.file = Some(feedback_path.to_string_lossy().into_owned());
    if args.dry_run {
        meta.warnings
            .push("dry run; no disposition appended".into());
    }
    output::write_success(
        ClipResolveData {
            changed,
            id,
            feedback: record,
        },
        pretty,
        meta,
    )
    .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    Ok(0)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClipWontfixData {
    pub changed: bool,
    pub feedback: FeedbackRecord,
}

pub fn wontfix(
    args: WontfixArgs,
    file: Option<PathBuf>,
    pretty: bool,
    now: Timestamp,
) -> AppResult<i32> {
    let prefix = normalize_clip_prefix(&args.id)?;
    if args.reason.trim().is_empty() {
        return Err(AppError::invalid_input(
            "wontfix reason cannot be empty",
            "Pass a non-empty --reason explaining why this will not be fixed.",
        ));
    }
    let resolved = store::discover_clips(file)?;
    let feedback_path = feedback::discover_feedback(&resolved)?;
    let ts = format_timestamp(now);
    let id = match store::with_shared(&resolved.path, |log| {
        let bytes = store::read_bytes(log, &resolved.path)?;
        let folded = store::fold_clip_bytes(&bytes);
        match_clip(&prefix, &folded.items)
    }) {
        Ok(id) => id,
        Err(error) if error.code == "not_found" && error.exit_code == 66 => {
            return Err(AppError::not_found(
                format!("paperclips file not found: {}", resolved.path.display()),
                "Run `paperclip list --status all` to find a clip ID first.",
            ));
        }
        Err(error) => return Err(error),
    };
    let existing = feedback::load(&feedback_path);
    let mut record = FeedbackRecord::new(&id, "clip", "wontfix", &ts, existing.get(&id));
    record.resolution = Some(format!("wontfix: {}", args.reason));
    record.reason = Some(args.reason);
    record.resolved_by = Some(
        args.owner
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "unknown".to_string()),
    );
    record.owner = args.owner;
    record.linked_task = args.task.clone();
    record.task_id = args.task;
    feedback::append(&feedback_path, &record)?;
    let mut meta = Meta::new();
    meta.file = Some(feedback_path.to_string_lossy().into_owned());
    output::write_success(
        ClipWontfixData {
            changed: true,
            feedback: record,
        },
        pretty,
        meta,
    )
    .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    Ok(0)
}

fn normalize_task(value: Option<&str>) -> Option<String> {
    match value {
        None | Some("") | Some("null") | Some("None") => None,
        Some(value) => Some(value.to_string()),
    }
}

fn normalize_clip_prefix(input: &str) -> AppResult<String> {
    let prefix = input
        .get(..3)
        .filter(|prefix| prefix.eq_ignore_ascii_case("cl_"))
        .map_or(input, |_| &input[3..]);
    if prefix.len() < 4 || !prefix.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(AppError::invalid_argument(
            format!("invalid clip ID prefix '{input}'"),
            "Pass at least 4 hexadecimal digits, with optional cl_ prefix.",
        ));
    }
    Ok(prefix.to_ascii_lowercase())
}

fn match_clip(prefix: &str, items: &[paper_core::ClipListItem]) -> AppResult<String> {
    let mut candidates: Vec<_> = items
        .iter()
        .map(|item| item.clip.id.clone())
        .filter(|id| {
            id.get(..3)
                .filter(|id_prefix| id_prefix.eq_ignore_ascii_case("cl_"))
                .and_then(|_| id.get(3..))
                .is_some_and(|hex| hex.to_ascii_lowercase().starts_with(prefix))
        })
        .collect();
    candidates.sort();
    match candidates.as_slice() {
        [] => Err(AppError::not_found(
            format!("no clip matches ID prefix '{prefix}'"),
            "Run `paperclip list --status all` and retry with a listed ID.",
        )),
        [id] => Ok(id.clone()),
        _ => Err(AppError::ambiguous_id(prefix, candidates)),
    }
}
