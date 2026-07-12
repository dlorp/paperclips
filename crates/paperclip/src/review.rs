use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;
use std::path::PathBuf;

use paper_core::error::{AppError, AppResult};
use paper_core::output;
use paper_core::store;
use paper_core::{ClipListItem, ClipStatus, ItemStatus, ListItem};
use serde::{Deserialize, Serialize};

use crate::OutputFormat;

const TAG_CAP: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Strength {
    Exact,
    Prefix,
    Tag,
}

impl Strength {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Prefix => "prefix",
            Self::Tag => "tag-level",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Overlap {
    pub anchor: String,
    pub strength: Strength,
    pub cuts: Vec<CutSummary>,
    pub clips: Vec<ClipSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CutSummary {
    pub id: String,
    pub severity: String,
    pub text: String,
    pub agent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipSummary {
    pub id: String,
    pub impact: String,
    pub text: String,
    pub agent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reports: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewData {
    pub overlaps: Vec<Overlap>,
    pub fix: Vec<CutSummary>,
    pub keep: Vec<ClipSummary>,
}

// ReviewMeta and ReviewFiles are used by the JSON envelope structure
// (the output module's Meta handles contract/file metadata)
pub fn run(
    format: OutputFormat,
    cuts_file: Option<PathBuf>,
    clips_file: Option<PathBuf>,
    pretty: bool,
) -> AppResult<i32> {
    let resolved_cuts = store::discover(cuts_file)?;
    let resolved_clips = store::discover_clips(clips_file)?;

    let mut warnings = Vec::new();

    // Fold cuts
    let cuts_folded = match store::with_shared(&resolved_cuts.path, |log| {
        let bytes = store::read_bytes(log, &resolved_cuts.path)?;
        Ok(store::fold_bytes(&bytes))
    }) {
        Ok(folded) => folded,
        Err(error) if error.code == "not_found" && error.exit_code == 66 => {
            if resolved_cuts.explicit {
                return Err(AppError::not_found(
                    format!("papercuts file not found: {}", resolved_cuts.path.display()),
                    "Pass an existing --file PATH or run `papercuts add` first.",
                ));
            }
            warnings.push("no papercuts file yet".into());
            store::FoldResult::default()
        }
        Err(error) => return Err(error),
    };
    warnings.extend(cuts_folded.warnings);

    // Fold clips
    let clips_folded = match store::with_shared(&resolved_clips.path, |log| {
        let bytes = store::read_bytes(log, &resolved_clips.path)?;
        Ok(store::fold_clip_bytes(&bytes))
    }) {
        Ok(folded) => folded,
        Err(error) if error.code == "not_found" && error.exit_code == 66 => {
            if resolved_clips.explicit {
                return Err(AppError::not_found(
                    format!(
                        "paperclips file not found: {}",
                        resolved_clips.path.display()
                    ),
                    "Pass an existing --file PATH or run `paperclip add` first.",
                ));
            }
            warnings.push("no paperclips file yet".into());
            store::ClipFoldResult::default()
        }
        Err(error) => return Err(error),
    };
    warnings.extend(clips_folded.warnings);

    // Filter to open only
    let open_cuts: Vec<&ListItem> = cuts_folded
        .items
        .iter()
        .filter(|item| item.status == ItemStatus::Open)
        .collect();

    let open_clips: Vec<&ClipListItem> = clips_folded
        .items
        .iter()
        .filter(|item| item.status == ClipStatus::Open)
        .collect();

    // Build join
    let mut overlaps = Vec::new();
    let mut matched_cut_ids = HashSet::new();
    let mut matched_clip_ids = HashSet::new();

    // 1. Exact join — identical normalized where
    let mut exact_anchors: BTreeMap<String, (Vec<&ListItem>, Vec<&ClipListItem>)> = BTreeMap::new();
    for cut in &open_cuts {
        if let Some(ref w) = cut.cut.where_loc {
            exact_anchors.entry(w.clone()).or_default().0.push(cut);
        }
    }
    for clip in &open_clips {
        if let Some(ref w) = clip.clip.where_loc {
            if let Some(entry) = exact_anchors.get_mut(w) {
                entry.1.push(clip);
            }
        }
    }
    for (anchor, (cuts, clips)) in &exact_anchors {
        if !clips.is_empty() {
            let cut_ids: Vec<String> = cuts.iter().map(|c| c.cut.id.clone()).collect();
            let clip_ids: Vec<String> = clips.iter().map(|c| c.clip.id.clone()).collect();
            matched_cut_ids.extend(cut_ids);
            matched_clip_ids.extend(clip_ids);
            overlaps.push(Overlap {
                anchor: anchor.clone(),
                strength: Strength::Exact,
                cuts: cuts.iter().map(|c| cut_summary(c)).collect(),
                clips: clips.iter().map(|c| clip_summary(c)).collect(),
            });
        }
    }

    // 2. Prefix join — one where is a path-prefix of the other
    // Collect unmatched cuts/clips with where
    let unmatched_cuts_with_where: Vec<&&ListItem> = open_cuts
        .iter()
        .filter(|c| c.cut.where_loc.is_some() && !matched_cut_ids.contains(&c.cut.id))
        .collect();

    let unmatched_clips_with_where: Vec<&&ClipListItem> = open_clips
        .iter()
        .filter(|c| c.clip.where_loc.is_some() && !matched_clip_ids.contains(&c.clip.id))
        .collect();

    // Build prefix map: for each unmatched cut/clip, generate all path prefixes
    // and collect which records share a prefix relationship
    let mut prefix_anchors: BTreeMap<String, (Vec<&ListItem>, Vec<&ClipListItem>)> =
        BTreeMap::new();

    for cut in &unmatched_cuts_with_where {
        let w = cut.cut.where_loc.as_ref().unwrap();
        for prefix in path_prefixes(w) {
            prefix_anchors.entry(prefix).or_default().0.push(cut);
        }
        // Also register the full path so that shorter paths can match
        prefix_anchors.entry(w.clone()).or_default().0.push(cut);
    }

    for clip in &unmatched_clips_with_where {
        let w = clip.clip.where_loc.as_ref().unwrap();
        for prefix in path_prefixes(w) {
            if let Some(entry) = prefix_anchors.get_mut(&prefix) {
                entry.1.push(clip);
            }
        }
        if let Some(entry) = prefix_anchors.get_mut(w.as_str()) {
            entry.1.push(clip);
        }
    }

    // For prefix join: only emit if BOTH sides have entries, and they don't all
    // share the exact same where (those were already caught by exact join)
    for (anchor, (cuts, clips)) in &prefix_anchors {
        if clips.is_empty() || cuts.is_empty() {
            continue;
        }
        // Filter out pairs that would be exact matches (same where already handled)
        let has_mixed = cuts
            .iter()
            .any(|c| c.cut.where_loc.as_deref() != Some(anchor.as_str()))
            || clips
                .iter()
                .any(|c| c.clip.where_loc.as_deref() != Some(anchor.as_str()));
        if !has_mixed {
            continue;
        }
        // Deduplicate: only include records not yet matched
        let new_cuts: Vec<&ListItem> = cuts
            .iter()
            .filter(|c| !matched_cut_ids.contains(&c.cut.id))
            .copied()
            .collect();
        let new_clips: Vec<&ClipListItem> = clips
            .iter()
            .filter(|c| !matched_clip_ids.contains(&c.clip.id))
            .copied()
            .collect();
        if new_cuts.is_empty() || new_clips.is_empty() {
            continue;
        }
        let cut_ids: Vec<String> = new_cuts.iter().map(|c| c.cut.id.clone()).collect();
        let clip_ids: Vec<String> = new_clips.iter().map(|c| c.clip.id.clone()).collect();
        matched_cut_ids.extend(cut_ids);
        matched_clip_ids.extend(clip_ids);
        overlaps.push(Overlap {
            anchor: anchor.clone(),
            strength: Strength::Prefix,
            cuts: new_cuts.iter().map(|c| cut_summary(c)).collect(),
            clips: new_clips.iter().map(|c| clip_summary(c)).collect(),
        });
    }

    // 3. Tag join — shared tag, no where match, capped at TAG_CAP
    let remaining_cuts: Vec<&&ListItem> = open_cuts
        .iter()
        .filter(|c| !matched_cut_ids.contains(&c.cut.id))
        .collect();

    let remaining_clips: Vec<&&ClipListItem> = open_clips
        .iter()
        .filter(|c| !matched_clip_ids.contains(&c.clip.id))
        .collect();

    // Build tag -> indices map
    let mut tag_to_cuts: HashMap<&str, Vec<&ListItem>> = HashMap::new();
    for cut in &remaining_cuts {
        for tag in &cut.cut.tags {
            tag_to_cuts.entry(tag.as_str()).or_default().push(cut);
        }
    }
    let mut tag_to_clips: HashMap<&str, Vec<&ClipListItem>> = HashMap::new();
    for clip in &remaining_clips {
        for tag in &clip.clip.tags {
            tag_to_clips.entry(tag.as_str()).or_default().push(clip);
        }
    }

    // Collect tag overlaps
    let mut tag_overlaps: Vec<(String, Vec<&ListItem>, Vec<&ClipListItem>)> = Vec::new();
    let mut seen_tags = HashSet::new();
    for tag in tag_to_cuts.keys() {
        if tag_to_clips.contains_key(tag) && seen_tags.insert(*tag) {
            let cuts = tag_to_cuts.get(tag).unwrap().clone();
            let clips = tag_to_clips.get(tag).unwrap().clone();
            tag_overlaps.push((tag.to_string(), cuts, clips));
        }
    }

    // Sort by combined count desc, then max severity/impact desc, then anchor alpha
    tag_overlaps.sort_by(|a, b| {
        let count_a = a.1.len() + a.2.len();
        let count_b = b.1.len() + b.2.len();
        count_b
            .cmp(&count_a)
            .then_with(|| {
                let max_sev_a = a.1.iter().map(|c| c.cut.severity.rank()).max().unwrap_or(0);
                let max_imp_a = a.2.iter().map(|c| c.clip.impact.rank()).max().unwrap_or(0);
                let max_a = max_sev_a.max(max_imp_a);
                let max_sev_b = b.1.iter().map(|c| c.cut.severity.rank()).max().unwrap_or(0);
                let max_imp_b = b.2.iter().map(|c| c.clip.impact.rank()).max().unwrap_or(0);
                let max_b = max_sev_b.max(max_imp_b);
                max_b.cmp(&max_a)
            })
            .then_with(|| a.0.cmp(&b.0))
    });

    // Cap at TAG_CAP
    let capped_total = tag_overlaps.len();
    tag_overlaps.truncate(TAG_CAP);

    for (tag, cuts, clips) in &tag_overlaps {
        let cut_ids: Vec<String> = cuts.iter().map(|c| c.cut.id.clone()).collect();
        let clip_ids: Vec<String> = clips.iter().map(|c| c.clip.id.clone()).collect();
        matched_cut_ids.extend(cut_ids);
        matched_clip_ids.extend(clip_ids);
        overlaps.push(Overlap {
            anchor: tag.clone(),
            strength: Strength::Tag,
            cuts: cuts.iter().map(|c| cut_summary(c)).collect(),
            clips: clips.iter().map(|c| clip_summary(c)).collect(),
        });
    }

    if capped_total > TAG_CAP {
        warnings.push(format!(
            "tag-level overlaps capped: showing top {TAG_CAP} of {capped_total} tags"
        ));
    }

    // Sort all overlaps: exact > prefix > tag; within strength: combined count desc,
    // max severity/impact desc, anchor alpha
    overlaps.sort_by(|a, b| {
        a.strength
            .cmp(&b.strength)
            .then_with(|| {
                let count_a = a.cuts.len() + a.clips.len();
                let count_b = b.cuts.len() + b.clips.len();
                count_b.cmp(&count_a)
            })
            .then_with(|| {
                let max_sev_a = a
                    .cuts
                    .iter()
                    .map(|c| severity_rank(&c.severity))
                    .max()
                    .unwrap_or(0);
                let max_imp_a = a
                    .clips
                    .iter()
                    .map(|c| impact_rank(&c.impact))
                    .max()
                    .unwrap_or(0);
                let max_a = max_sev_a.max(max_imp_a);
                let max_sev_b = b
                    .cuts
                    .iter()
                    .map(|c| severity_rank(&c.severity))
                    .max()
                    .unwrap_or(0);
                let max_imp_b = b
                    .clips
                    .iter()
                    .map(|c| impact_rank(&c.impact))
                    .max()
                    .unwrap_or(0);
                let max_b = max_sev_b.max(max_imp_b);
                max_b.cmp(&max_a)
            })
            .then_with(|| a.anchor.cmp(&b.anchor))
    });

    // Build fix/keep lists (unmatched open records)
    let fix: Vec<CutSummary> = open_cuts
        .iter()
        .filter(|c| !matched_cut_ids.contains(&c.cut.id))
        .map(|c| cut_summary(c))
        .collect();

    let keep: Vec<ClipSummary> = open_clips
        .iter()
        .filter(|c| !matched_clip_ids.contains(&c.clip.id))
        .map(|c| clip_summary(c))
        .collect();

    // Count strengths for header
    let exact_count = overlaps
        .iter()
        .filter(|o| o.strength == Strength::Exact)
        .count();
    let prefix_count = overlaps
        .iter()
        .filter(|o| o.strength == Strength::Prefix)
        .count();
    let tag_count = overlaps
        .iter()
        .filter(|o| o.strength == Strength::Tag)
        .count();

    let data = ReviewData {
        overlaps,
        fix,
        keep,
    };

    match format {
        OutputFormat::Md => {
            write_markdown(&data, exact_count, prefix_count, tag_count, &warnings)?;
        }
        OutputFormat::Json => {
            let mut meta = output::Meta::new();
            meta.file = Some(format!(
                "{}, {}",
                resolved_cuts.path.display(),
                resolved_clips.path.display()
            ));
            meta.warnings = warnings;
            output::write_success(data, pretty, meta)
                .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
        }
    }
    Ok(0)
}

fn path_prefixes(path: &str) -> Vec<String> {
    let parts: Vec<&str> = path.split('/').collect();
    let mut prefixes = Vec::new();
    for i in 1..parts.len() {
        prefixes.push(parts[..i].join("/"));
    }
    prefixes
}

fn cut_summary(item: &ListItem) -> CutSummary {
    CutSummary {
        id: item.cut.id.clone(),
        severity: item.cut.severity.as_str().to_string(),
        text: item.cut.text.clone(),
        agent: item.cut.agent.clone(),
    }
}

fn clip_summary(item: &ClipListItem) -> ClipSummary {
    ClipSummary {
        id: item.clip.id.clone(),
        impact: item.clip.impact.as_str().to_string(),
        text: item.clip.text.clone(),
        agent: item.clip.agent.clone(),
        reports: if item.notes.is_empty() {
            None
        } else {
            Some(item.notes.len() + 1)
        },
    }
}

fn severity_rank(s: &str) -> u8 {
    match s {
        "minor" => 0,
        "major" => 1,
        "blocker" => 2,
        _ => 0,
    }
}

fn impact_rank(s: &str) -> u8 {
    match s {
        "nice" => 0,
        "solid" => 1,
        "huge" => 2,
        _ => 0,
    }
}

fn write_markdown(
    data: &ReviewData,
    exact_count: usize,
    prefix_count: usize,
    tag_count: usize,
    warnings: &[String],
) -> AppResult<()> {
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());

    // Legend
    writeln!(out, "Overlap shapes — read each pair and assign one:")
        .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
    writeln!(
        out,
        "  A. Same component, different aspect — clip praises the part, cut names a sharp"
    )
    .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
    writeln!(
        out,
        "     edge on it. → Fix the edge, keep the part. The clip is a guardrail on the"
    )
    .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
    writeln!(out, "     remediation: it tells you what NOT to remove.")
        .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
    writeln!(
        out,
        "  B. Value confirmed, access broken — clip proves the asset's worth, cut says"
    )
    .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
    writeln!(
        out,
        "     finding/using it is the problem. → Amplify: index, rename, document."
    )
    .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
    writeln!(
        out,
        "     Promotion target is discoverability, not CLAUDE.md prose."
    )
    .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
    writeln!(
        out,
        "  C. Context-dependence — the same *behavior* is clipped by one agent/workflow"
    )
    .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
    writeln!(
        out,
        "     and cut by another. → Neither fix nor remove: add a mode or config flag."
    )
    .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
    writeln!(
        out,
        "     Rare; highest-information record in the file when it appears."
    )
    .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
    writeln!(out).map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;

    // Header
    let mut parts = Vec::new();
    if exact_count > 0 {
        parts.push(format!("{exact_count} exact"));
    }
    if prefix_count > 0 {
        parts.push(format!("{prefix_count} prefix"));
    }
    if tag_count > 0 {
        parts.push(format!("{tag_count} tag-level"));
    }
    let summary = if parts.is_empty() {
        "No overlaps found".to_string()
    } else {
        format!("Overlaps ({})", parts.join(", "))
    };
    writeln!(out, "## {summary}")
        .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
    writeln!(out).map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;

    // Each overlap
    for overlap in &data.overlaps {
        writeln!(
            out,
            "### {}  [{} · {} cut{} / {} clip{}]",
            overlap.anchor,
            overlap.strength.as_str(),
            overlap.cuts.len(),
            if overlap.cuts.len() == 1 { "" } else { "s" },
            overlap.clips.len(),
            if overlap.clips.len() == 1 { "" } else { "s" },
        )
        .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;

        for cut in &overlap.cuts {
            let text_preview = truncate_text(&cut.text, 72);
            writeln!(
                out,
                "  CUT  [{:<7}] {} — {}",
                cut.severity, cut.id, text_preview
            )
            .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
        }
        for clip in &overlap.clips {
            let text_preview = truncate_text(&clip.text, 72);
            let reports_str = clip
                .reports
                .map(|r| format!(" (×{r} reports)"))
                .unwrap_or_default();
            writeln!(
                out,
                "  CLIP [{:<7}] {} — {}{}",
                clip.impact, clip.id, text_preview, reports_str
            )
            .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
        }
        writeln!(out, "  Disposition: ____________________")
            .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
        writeln!(out).map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
    }

    // Fix section (unmatched cuts)
    if !data.fix.is_empty() {
        writeln!(
            out,
            "## Fix ({} unpaired cut{})",
            data.fix.len(),
            if data.fix.len() == 1 { "" } else { "s" }
        )
        .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
        for cut in &data.fix {
            let text_preview = truncate_text(&cut.text, 72);
            writeln!(
                out,
                "  CUT  [{:<7}] {} — {}",
                cut.severity, cut.id, text_preview
            )
            .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
        }
        writeln!(out).map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
    }

    // Keep section (unmatched clips)
    if !data.keep.is_empty() {
        writeln!(
            out,
            "## Keep ({} unpaired clip{})",
            data.keep.len(),
            if data.keep.len() == 1 { "" } else { "s" }
        )
        .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
        for clip in &data.keep {
            let text_preview = truncate_text(&clip.text, 72);
            writeln!(
                out,
                "  CLIP [{:<7}] {} — {}",
                clip.impact, clip.id, text_preview
            )
            .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
        }
        writeln!(out).map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
    }

    for warning in warnings {
        writeln!(out, "> note: {warning}")
            .map_err(|e| AppError::from_io(e, std::path::Path::new("stdout")))?;
    }

    Ok(())
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}…", &text[..max_len.saturating_sub(1)])
    }
}
