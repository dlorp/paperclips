use crate::cli::{ListArgs, OutputFormat, StatusFilter};
use crate::error::{AppError, AppResult};
use crate::feedback::{self, FeedbackRecord};
use crate::output::{self, Meta};
use crate::store;
use crate::{ItemStatus, ListItem, Severity, parse_since};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SeverityCounts {
    pub minor: usize,
    pub major: usize,
    pub blocker: usize,
    pub unset: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListData {
    pub items: Vec<ListItem>,
    pub count: usize,
    pub total: usize,
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<SeverityCounts>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DigestData {
    pub open_by_severity: SeverityCounts,
    pub total_open: usize,
}

pub fn run(args: ListArgs, file: Option<PathBuf>, pretty: bool, now: Timestamp) -> AppResult<i32> {
    let resolved = store::discover(file)?;
    let feedback_path = feedback::discover_feedback(&resolved)?;
    let mut warnings = Vec::new();
    let folded = match store::with_shared(&resolved.path, |log| {
        let bytes = store::read_bytes(log, &resolved.path)?;
        Ok(store::fold_bytes(&bytes))
    }) {
        Ok(folded) => folded,
        Err(error) if error.code == "not_found" && error.exit_code == 66 => {
            if resolved.explicit {
                return Err(AppError::not_found(
                    format!("papercuts file not found: {}", resolved.path.display()),
                    "Pass an existing --file PATH or run `papercuts add` to create a discovered default file.",
                ));
            }
            warnings.push("no papercuts file yet; papercuts add creates it".into());
            store::FoldResult::default()
        }
        Err(error) => return Err(error),
    };
    warnings.extend(folded.warnings);
    let dispositions = feedback::load(&feedback_path);
    let since = args
        .since
        .as_deref()
        .map(|value| parse_since(value, now))
        .transpose()?;
    let mut items: Vec<_> = folded
        .items
        .into_iter()
        .map(|mut item| {
            if let Some(status) = feedback::cut_status(disposition_of(&dispositions, &item.cut.id))
            {
                item.status = status;
            }
            item
        })
        .filter(|item| match args.status {
            StatusFilter::Open => item.status == ItemStatus::Open,
            StatusFilter::Resolved => item.status == ItemStatus::Resolved,
            StatusFilter::Wontfix => item.status == ItemStatus::Wontfix,
            StatusFilter::All => true,
        })
        .filter(|item| {
            !args.unresolved
                || (item.status == ItemStatus::Open
                    && !feedback::is_terminal(
                        disposition_of(&dispositions, &item.cut.id).unwrap_or("open"),
                    ))
        })
        .filter(|item| {
            args.agent
                .as_ref()
                .is_none_or(|agent| &item.cut.agent == agent)
        })
        .filter(|item| {
            args.tag
                .as_ref()
                .is_none_or(|tag| item.cut.tags.contains(tag))
        })
        .filter(|item| {
            args.severity
                .is_none_or(|severity| item.cut.severity == severity)
        })
        .filter(|item| {
            since.is_none_or(|threshold| {
                item.cut
                    .ts
                    .parse::<Timestamp>()
                    .is_ok_and(|timestamp| timestamp >= threshold)
            })
        })
        .filter(|item| {
            args.where_loc
                .as_ref()
                .is_none_or(|filter| item.cut.where_loc.as_ref().is_some_and(|loc| loc == filter))
        })
        .collect();

    if args.digest {
        let counts = severity_counts(&items);
        let total_open = items
            .iter()
            .filter(|item| item.status == ItemStatus::Open)
            .count();
        let digest_only = !args.unresolved && args.severity.is_none();
        if digest_only {
            if args.format == OutputFormat::Md {
                let mut out = std::io::stdout().lock();
                writeln!(
                    out,
                    "open cuts by severity: minor={} major={} blocker={} unset={} (total open={})",
                    counts.minor, counts.major, counts.blocker, counts.unset, total_open
                )
                .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
                return Ok(0);
            }
            let mut meta = Meta::new();
            meta.file = Some(resolved.path.to_string_lossy().into_owned());
            meta.warnings = warnings;
            output::write_success(
                DigestData {
                    open_by_severity: counts,
                    total_open,
                },
                pretty,
                meta,
            )
            .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
            return Ok(0);
        }
        let total = items.len();
        let full = ListData {
            count: items.len(),
            total,
            truncated: false,
            items,
            digest: Some(counts),
        };
        if args.format == OutputFormat::Md {
            write_markdown(&full.items, &warnings)?;
        } else {
            let mut meta = Meta::new();
            meta.file = Some(resolved.path.to_string_lossy().into_owned());
            meta.warnings = warnings;
            output::write_success(full, pretty, meta)
                .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
        }
        return Ok(0);
    }

    let total = items.len();
    items.truncate(args.limit);
    let data = ListData {
        count: items.len(),
        total,
        truncated: total > items.len(),
        items,
        digest: None,
    };
    if data.items.is_empty() {
        warnings.push("no papercuts matched; try --status all or broader filters".into());
    }
    if args.format == OutputFormat::Md {
        write_markdown(&data.items, &warnings)?;
    } else {
        let mut meta = Meta::new();
        meta.file = Some(resolved.path.to_string_lossy().into_owned());
        meta.warnings = warnings;
        output::write_success(data, pretty, meta)
            .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    }
    Ok(0)
}

fn disposition_of<'a>(
    dispositions: &'a HashMap<String, FeedbackRecord>,
    id: &str,
) -> Option<&'a str> {
    dispositions
        .get(id)
        .map(|record| record.disposition.as_str())
}

fn severity_counts(items: &[ListItem]) -> SeverityCounts {
    let mut counts = SeverityCounts::default();
    for item in items {
        if item.status != ItemStatus::Open {
            continue;
        }
        match item.cut.severity {
            Severity::Minor => counts.minor += 1,
            Severity::Major => counts.major += 1,
            Severity::Blocker => counts.blocker += 1,
        }
    }
    counts.unset = items
        .iter()
        .filter(|item| item.status == ItemStatus::Open)
        .count()
        - counts.minor
        - counts.major
        - counts.blocker;
    counts
}

fn write_markdown(items: &[ListItem], warnings: &[String]) -> AppResult<()> {
    let mut output = std::io::BufWriter::new(std::io::stdout().lock());
    for severity in [Severity::Blocker, Severity::Major, Severity::Minor] {
        let matching: Vec<_> = items
            .iter()
            .filter(|item| item.cut.severity == severity)
            .collect();
        if matching.is_empty() {
            continue;
        }
        writeln!(
            output,
            "## {}",
            match severity {
                Severity::Blocker => "Blocker",
                Severity::Major => "Major",
                Severity::Minor => "Minor",
            }
        )
        .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
        for item in matching {
            let id = match item.status {
                ItemStatus::Resolved => format!("~~{}~~", item.cut.id),
                ItemStatus::Wontfix => format!("{} (wontfix)", item.cut.id),
                ItemStatus::Open => item.cut.id.clone(),
            };
            let tags = if item.cut.tags.is_empty() {
                String::new()
            } else {
                format!(" ({})", item.cut.tags.join(","))
            };
            writeln!(
                output,
                "- [{id}] {} — {}, {}{tags}",
                item.cut.text, item.cut.agent, item.cut.ts
            )
            .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
        }
    }
    for warning in warnings {
        writeln!(output, "> note: {warning}")
            .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    }
    Ok(())
}
