//! End-to-end tests for the clips binary: lifecycle, feedback ledger, and
//! record compatibility with logs written by the Python logger.

use assert_cmd::Command;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::path::Path;
use tempfile::TempDir;

const NOW: &str = "2026-07-09T18:30:00.123456Z";

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    ok: bool,
    data: T,
    meta: Meta,
}

#[derive(Debug, Deserialize)]
struct Meta {
    #[serde(default)]
    warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ClipAddData {
    changed: bool,
    record: ClipRecord,
}

#[derive(Debug, Deserialize)]
struct ClipRecord {
    id: String,
    impact: String,
    tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ClipListData {
    items: Vec<ClipListItem>,
    count: usize,
}

#[derive(Debug, Deserialize)]
struct ClipListItem {
    status: String,
    #[serde(default)]
    notes: Vec<Value>,
}

fn command() -> Command {
    let mut command = assert_cmd::cargo::cargo_bin_cmd!("paperclip");
    command
        .env("PAPERCUTS_NOW", NOW)
        .env_remove("PAPERCUTS_FILE")
        .env_remove("PAPERCLIP_FILE")
        .env_remove("PAPERCUTS_AGENT")
        .env_remove("FEEDBACK_RESOLUTIONS_FILE")
        .env_remove("CLAUDECODE");
    command
}

fn run_file(file: &Path, args: &[&str]) -> std::process::Output {
    command()
        .arg("--file")
        .arg(file)
        .args(args)
        .output()
        .unwrap()
}

fn success<T: DeserializeOwned>(output: &std::process::Output) -> Envelope<T> {
    assert!(
        output.status.success(),
        "status={:?}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Envelope<T> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(envelope.ok);
    envelope
}

fn error(output: &std::process::Output, exit: i32, code: &str) -> Value {
    assert_eq!(output.status.code(), Some(exit));
    let envelope: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(envelope["error"]["code"], code);
    envelope
}

#[test]
fn clip_accepts_python_impact_vocabulary_and_writes_agent_events() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("clips.jsonl");
    let added: Envelope<ClipAddData> = success(&run_file(
        &file,
        &[
            "add",
            "bulk insert cut sync time to seconds",
            "--agent",
            "tester",
            "--tag",
            "storage",
            "--impact",
            "trivial",
            "--where",
            "kv-store",
        ],
    ));
    assert!(added.data.changed);
    assert_eq!(added.data.record.impact, "nice");
    assert_eq!(added.data.record.tags, ["storage"]);

    let id = added.data.record.id;
    success::<Value>(&run_file(&file, &["promote", &id, "--agent", "tester"]));
    success::<Value>(&run_file(
        &file,
        &["note", &id, "confirmed in staging", "--agent", "reviewer"],
    ));

    let raw = std::fs::read_to_string(&file).unwrap();
    let events: Vec<Value> = raw
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let promote = events
        .iter()
        .find(|event| event["kind"] == "promote")
        .unwrap();
    assert_eq!(promote["agent"], "tester");
    let note = events.iter().find(|event| event["kind"] == "note").unwrap();
    assert_eq!(note["agent"], "reviewer");

    let listed: Envelope<ClipListData> =
        success(&run_file(&file, &["list", "--status", "promoted"]));
    assert_eq!(listed.data.count, 1);
    assert_eq!(listed.data.items[0].status, "promoted");
    assert_eq!(listed.data.items[0].notes.len(), 1);
    assert!(listed.meta.warnings.is_empty());
}

#[test]
fn python_written_clip_folds_without_warnings() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("clips.jsonl");
    let python_clip = json!({
        "kind": "clip",
        "id": "cl_0123456789ab",
        "ts": "2026-07-09T18:30:00.000+00:00",
        "agent": "logger",
        "text": "python wrote this win",
        "tags": ["tooling"],
        "impact": "trivial",
        "status": "active",
        "where": "kv-store"
    })
    .to_string();
    std::fs::write(&file, format!("{python_clip}\n")).unwrap();

    let listed: Envelope<ClipListData> = success(&run_file(&file, &["list", "--status", "all"]));
    assert_eq!(listed.data.count, 1);
    assert_eq!(listed.data.items[0].status, "open");
    assert!(listed.meta.warnings.is_empty());
}

#[test]
fn resolve_and_wontfix_gate_and_land_in_the_feedback_ledger() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("clips.jsonl");
    let first: Envelope<ClipAddData> = success(&run_file(
        &file,
        &["add", "helper paid for itself", "--agent", "tester"],
    ));
    let first_id = first.data.record.id;

    let rejected = error(
        &run_file(&file, &["resolve", &first_id, "--resolution", "codified"]),
        65,
        "invalid_input",
    );
    assert!(
        rejected["error"]["message"]
            .as_str()
            .unwrap()
            .contains("CAPA")
    );

    success::<Value>(&run_file(
        &file,
        &[
            "resolve",
            &first_id,
            "--resolution",
            "codified in the contributor guide",
            "--resolved-by",
            "tester",
            "--verification-method",
            "fresh clone followed the guide",
            "--verification-result",
            "setup completed without prompts",
        ],
    ));
    let resolved: Envelope<ClipListData> =
        success(&run_file(&file, &["list", "--status", "resolved"]));
    assert_eq!(resolved.data.count, 1);
    assert_eq!(resolved.data.items[0].status, "resolved");

    let second: Envelope<ClipAddData> = success(&run_file(
        &file,
        &["add", "one-off measurement", "--agent", "tester"],
    ));
    let second_id = second.data.record.id;
    success::<Value>(&run_file(
        &file,
        &[
            "wontfix",
            &second_id,
            "--reason",
            "measurement was environment specific",
        ],
    ));
    let wontfix: Envelope<ClipListData> =
        success(&run_file(&file, &["list", "--status", "wontfix"]));
    assert_eq!(wontfix.data.count, 1);
    assert_eq!(wontfix.data.items[0].status, "wontfix");

    let unresolved: Envelope<ClipListData> = success(&run_file(
        &file,
        &["list", "--status", "all", "--unresolved"],
    ));
    assert_eq!(unresolved.data.count, 0);

    // Both dispositions are in one append-only ledger next to the explicit file.
    let ledger = std::fs::read_to_string(temp.path().join(".feedback-resolutions.jsonl")).unwrap();
    assert_eq!(ledger.lines().count(), 2);
}

#[test]
fn review_joins_cuts_and_clips_on_where() {
    let temp = TempDir::new().unwrap();
    let clips = temp.path().join("clips.jsonl");
    let cuts = temp.path().join("cuts.jsonl");
    let cut = json!({
        "kind": "cut",
        "id": "pc_000000000001",
        "ts": "2026-07-09T18:30:00.000Z",
        "agent": "tester",
        "text": "cache invalidation races on concurrent writes",
        "tags": ["storage"],
        "severity": "major",
        "cwd": ".",
        "repo": ".",
        "where": "kv-store"
    })
    .to_string();
    std::fs::write(&cuts, format!("{cut}\n")).unwrap();

    success::<Value>(&run_file(
        &clips,
        &[
            "add",
            "bulk insert cut sync time to seconds",
            "--agent",
            "tester",
            "--tag",
            "storage",
            "--impact",
            "solid",
            "--where",
            "kv-store",
        ],
    ));

    let output = command()
        .env("PAPERCUTS_FILE", &cuts)
        .arg("--file")
        .arg(&clips)
        .args(["review", "--format", "md"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let markdown = String::from_utf8(output.stdout).unwrap();
    assert!(markdown.contains("kv-store"));
    assert!(markdown.contains("exact"));
    assert!(markdown.contains("cache invalidation"));
    assert!(markdown.contains("bulk insert"));
}
