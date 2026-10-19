use crate::cli::WontfixArgs;
use crate::error::{AppError, AppResult};
use crate::feedback::{self, FeedbackRecord};
use crate::output::{self, Meta};
use crate::store;
use crate::{ListItem, format_timestamp};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct WontfixData {
    pub changed: bool,
    pub feedback: FeedbackRecord,
}

/// Record a terminal wontfix disposition. The source log is never rewritten;
/// the decision lands in the feedback ledger and wins as the latest record.
pub fn run(
    args: WontfixArgs,
    file: Option<PathBuf>,
    pretty: bool,
    now: Timestamp,
) -> AppResult<i32> {
    let prefix = normalize_prefix(&args.id)?;
    if args.reason.trim().is_empty() {
        return Err(AppError::invalid_input(
            "wontfix reason cannot be empty",
            "Pass a non-empty --reason explaining why this will not be fixed.",
        ));
    }
    let resolved = store::discover(file)?;
    let feedback_path = feedback::discover_feedback(&resolved)?;
    let ts = format_timestamp(now);

    let id = match store::with_shared(&resolved.path, |log| {
        let bytes = store::read_bytes(log, &resolved.path)?;
        let folded = store::fold_bytes(&bytes);
        match_id(&prefix, &folded.items)
    }) {
        Ok(id) => id,
        Err(error) if error.code == "not_found" && error.exit_code == 66 => {
            return Err(AppError::not_found(
                format!("papercuts file not found: {}", resolved.path.display()),
                "Run `papercuts list --status all` to find an ID after adding a papercut.",
            ));
        }
        Err(error) => return Err(error),
    };

    let existing = feedback::load(&feedback_path);
    let mut record = FeedbackRecord::new(&id, "cut", "wontfix", &ts, existing.get(&id));
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
        WontfixData {
            changed: true,
            feedback: record,
        },
        pretty,
        meta,
    )
    .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    Ok(0)
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
