use crate::cli::ResolveArgs;
use crate::error::{AppError, AppResult};
use crate::feedback::{self, FeedbackRecord};
use crate::output::{self, Meta};
use crate::store;
use crate::{ItemStatus, ListItem, Resolution, ResolveRecord, format_timestamp, resolve_agent};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct ResolveData {
    pub changed: bool,
    pub record: ListItem,
    /// The disposition ledger record written for this resolution.
    pub feedback: FeedbackRecord,
}

/// The CAPA effectiveness gate: a resolution is only a resolution when it
/// carries verification evidence. A linked task with no verification
/// reproduces the exact failure the record is meant to prevent.
fn capa_errors(args: &ResolveArgs) -> Vec<String> {
    let mut errors = Vec::new();
    if args
        .verification_method
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        errors.push("--verification-method is required (how was effectiveness verified?)".into());
    }
    if args
        .verification_result
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        errors.push("--verification-result is required (what did the verification find?)".into());
    }
    let has_resolution = args
        .resolution
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        || args
            .change
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
    if !has_resolution {
        errors.push("--resolution is required (or legacy --change naming the fix)".into());
    }
    let has_resolver = args
        .resolved_by
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        || args
            .owner
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
    if !has_resolver {
        errors.push("--resolved-by is required (or legacy --owner naming the resolver)".into());
    }
    errors
}

pub fn run(
    args: ResolveArgs,
    file: Option<PathBuf>,
    pretty: bool,
    now: Timestamp,
) -> AppResult<i32> {
    let prefix = normalize_prefix(&args.id)?;
    let resolved = store::discover(file)?;
    let feedback_path = feedback::discover_feedback(&resolved)?;
    if args
        .agent
        .as_deref()
        .is_some_and(|agent| agent.trim().is_empty())
    {
        return Err(AppError::invalid_input(
            "agent name cannot be empty or whitespace-only",
            "Pass a non-empty --agent NAME or omit the flag.",
        ));
    }
    // Locate the record before validating CAPA fields so a bad ID still
    // reports not_found/ambiguous rather than a verification complaint.
    let _ = locate(&resolved, &prefix)?;
    let errors = capa_errors(&args);
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

    // Prefer the explicit actor; fall back to the CAPA resolver so the resolve
    // event never records "unknown" when --resolved-by was supplied.
    let actor = args
        .agent
        .clone()
        .or_else(|| args.resolved_by.clone().or_else(|| args.owner.clone()));
    let (agent, source) = resolve_agent(actor);
    let ts = format_timestamp(now);
    let note = args.note;
    let action = |log: &mut std::fs::File| -> AppResult<(bool, bool, ListItem)> {
        let bytes = store::read_bytes(log, &resolved.path)?;
        let folded = store::fold_bytes(&bytes);
        let id = match_id(&prefix, &folded.items)?;
        let mut item = folded
            .items
            .into_iter()
            .find(|item| item.cut.id == id)
            .ok_or_else(|| AppError::internal("matched papercut disappeared during resolution"))?;
        if item.status == ItemStatus::Resolved {
            return Ok((false, true, item));
        }
        item.status = ItemStatus::Resolved;
        item.resolution = Some(Resolution {
            ts: ts.clone(),
            agent: agent.clone(),
            note: note.clone(),
        });
        if !args.dry_run {
            let event = ResolveRecord {
                kind: "resolve".into(),
                id,
                ts: ts.clone(),
                agent: agent.clone(),
                note: note.clone(),
            };
            store::append_json(log, &resolved.path, &bytes, &event)?;
        }
        Ok((!args.dry_run, false, item))
    };
    let (changed, already_resolved, record) = match if args.dry_run {
        store::with_shared(&resolved.path, action)
    } else {
        store::with_exclusive(&resolved.path, false, action)
    } {
        Ok(result) => result,
        Err(error) if error.code == "not_found" && error.exit_code == 66 => {
            return Err(AppError::not_found(
                format!("papercuts file not found: {}", resolved.path.display()),
                "Run `papercuts list --status all` to find an ID after adding a papercut.",
            ));
        }
        Err(error) => return Err(error),
    };

    let existing = feedback::load(&feedback_path);
    let mut disposition = FeedbackRecord::new(
        &record.cut.id,
        "cut",
        "resolved",
        &ts,
        existing.get(&record.cut.id),
    );
    disposition.resolution = args
        .resolution
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            args.change
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .map(|value| format!("fixed via {value}"))
        });
    disposition.linked_task = normalize_task(args.linked_task.as_deref().or(args.task.as_deref()));
    disposition.resolved_by = args
        .resolved_by
        .filter(|value| !value.trim().is_empty())
        .or_else(|| args.owner.clone().filter(|value| !value.trim().is_empty()));
    disposition.verification_method = args.verification_method;
    disposition.verification_result = args.verification_result;
    disposition.change_class = args.change_class.map(|value| value.as_str().to_string());
    disposition.changed_artifact = args.change;
    disposition.channel = args.channel.map(|value| value.as_str().to_string());
    disposition.task_id = args.task;
    disposition.owner = args.owner;

    if changed {
        feedback::append(&feedback_path, &disposition)?;
    }

    let mut meta = Meta::new();
    meta.file = Some(resolved.path.to_string_lossy().into_owned());
    meta.agent_source = Some(source.into());
    if already_resolved {
        meta.warnings.push("already resolved".into());
    } else if args.dry_run {
        meta.warnings
            .push("dry run; no resolve event or disposition appended".into());
    }
    output::write_success(
        ResolveData {
            changed,
            record,
            feedback: disposition,
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

fn locate(resolved: &store::ResolvedFile, prefix: &str) -> AppResult<String> {
    match store::with_shared(&resolved.path, |log| {
        let bytes = store::read_bytes(log, &resolved.path)?;
        let folded = store::fold_bytes(&bytes);
        match_id(prefix, &folded.items)
    }) {
        Ok(id) => Ok(id),
        Err(error) if error.code == "not_found" && error.exit_code == 66 => {
            Err(AppError::not_found(
                format!("papercuts file not found: {}", resolved.path.display()),
                "Run `papercuts list --status all` to find an ID after adding a papercut.",
            ))
        }
        Err(error) => Err(error),
    }
}

fn normalize_prefix(input: &str) -> AppResult<String> {
    let prefix = input
        .get(..3)
        .filter(|prefix| prefix.eq_ignore_ascii_case("pc_"))
        .map_or(input, |_| &input[3..]);
    if prefix.len() < 4 || !prefix.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(AppError::invalid_argument(
            format!("invalid papercut ID prefix '{input}'"),
            "Use `papercuts list --status all` and pass at least 4 hexadecimal digits, with optional pc_ prefix.",
        ));
    }
    Ok(prefix.to_ascii_lowercase())
}

fn match_id(prefix: &str, items: &[ListItem]) -> AppResult<String> {
    let mut candidates: Vec<_> = items
        .iter()
        .map(|item| item.cut.id.clone())
        .filter(|id| {
            id.get(..3)
                .filter(|id_prefix| id_prefix.eq_ignore_ascii_case("pc_"))
                .and_then(|_| id.get(3..))
                .is_some_and(|hex| hex.to_ascii_lowercase().starts_with(prefix))
        })
        .collect();
    candidates.sort();
    match candidates.as_slice() {
        [] => Err(AppError::not_found(
            format!("no papercut matches ID prefix '{prefix}'"),
            "Run `papercuts list --status all` and retry with a listed ID.",
        )),
        [id] => Ok(id.clone()),
        _ => Err(AppError::ambiguous_id(prefix, candidates)),
    }
}
