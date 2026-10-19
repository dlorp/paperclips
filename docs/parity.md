# Feature parity with the reference logger

`crates/paper-core` implements the full verb surface of the reference Python
logger (`papercuts.py`) that the original tools were built against, with the
three structural differences listed at the end. This page is the verification
checklist: what Rust implements, and for each row the test or live run that
backs it.

**Reference provenance.** The reference logger is not vendored in this
repository. The "Reference behaviour" column records what the port was written
against, so it is a description, not something a reader can check from this
checkout. The "Rust implementation" and "Evidence" columns are checkable: they
name paths, functions and test names that exist here. A row whose evidence cell
says "not covered" is implemented but untested, and is listed as such rather
than pointed at an unrelated test.

## Vocabulary

| Reference logger | Rust | Notes |
|---|---|---|
| `add` (cut) | `papercuts add` | Same fields: text, tags, agent, severity, where. |
| `clip` | `paperclip add` | Same fields: text, tags, agent, impact, where. |
| `list` | `papercuts list` / `paperclip list` | The reference has one binary with `--kind`; the Rust tools ship one binary per record kind. `paperclip review` remains the combined view. |
| `resolve` | `papercuts resolve` / `paperclip resolve` | CAPA-gated, writes the disposition ledger. |
| `wontfix` | `papercuts wontfix` / `paperclip wontfix` | Terminal no-action disposition. |

## Feature matrix

| Feature | Reference behaviour | Rust implementation | Evidence |
|---|---|---|---|
| Cut capture | `kind=cut`, severity minor/major/blocker, tags, where | `papercuts add` | `crates/papercuts/tests/cli.rs` (`every_command_success_envelope_deserializes`) |
| Clip capture | `kind=clip`, impact trivial/solid/huge, tags, where | `paperclip add`; `trivial` is accepted as an alias for `nice` | `crates/paperclip/tests/cli.rs` (`clip_accepts_python_impact_vocabulary_and_writes_agent_events`) |
| Record shape from either writer | Python records omit `cwd`/`repo`, add `status`, and may use `trivial` | Fold defaults unknown fields instead of reporting malformed lines | `python_written_cut_and_feedback_ledger_fold_in_rust`, `python_written_clip_folds_without_warnings` |
| Identical-text recurrence | `_norm_text` comparison against prior cuts | `store::normalize_text` + `store::find_recurrence_anchor`, writes `recurrence_of` | `recurrence_links_identical_text_and_timeout_signature` |
| Failure-class recurrence | `_signature_match`: timeout word + absence marker in one clause | `store::signature_match` with the same markers and word boundary | `recurrence_links_identical_text_and_timeout_signature` |
| Recurrence is read-only | Prior records are never rewritten | Anchor scan reads raw lines, the new record carries the link | `recurrence_links_identical_text_and_timeout_signature` asserts the prior line is unchanged |
| CAPA effectiveness gate | `--verification-method`, `--verification-result`, resolution, resolver are required | `resolve` rejects with `invalid_input` and lists every missing field | `resolve_requires_capa_verification_fields` |
| Legacy resolve flags | `--change`, `--class`, `--channel`, `--task`, `--owner` map into CAPA fields | Implemented in `commands::resolve::run`: `--change` feeds `resolution` and `changed_artifact`, `--class` feeds `change_class`, `--channel` feeds `channel`, `--task` feeds `linked_task`/`task_id`, `--owner` feeds `resolved_by` | Not covered. No test passes `--change`, `--class`, `--channel` or `--task`; `resolve_prefix_errors_and_idempotence_are_structured` covers only ID-prefix resolution and idempotence |
| Disposition ledger | Append-only `feedback-resolutions.jsonl`, latest record per id wins | `paper_core::feedback` with `FEEDBACK_RESOLUTIONS_FILE` override | `resolve_and_wontfix_gate_and_land_in_the_feedback_ledger` |
| Wontfix | `disposition=wontfix`, `reason` required, terminal | `wontfix` on both binaries; source log untouched | `wontfix_is_terminal_and_excluded_from_unresolved_and_digest` |
| Feedback join | `load_feedback_resolutions`; terminal set resolved/wontfix/duplicate/already_fixed | `feedback::load`, `feedback::is_terminal`, status overlay in both `list` commands | `python_written_cut_and_feedback_ledger_fold_in_rust` (the `resolved` case only) |
| `list --unresolved` | Non-terminal dispositions only | `--unresolved` on both `list` commands | `wontfix_is_terminal_and_excluded_from_unresolved_and_digest` |
| `list --digest` | Open-cut counts by severity | `papercuts list --digest` (JSON envelope or md line) | `wontfix_is_terminal_and_excluded_from_unresolved_and_digest` |
| `list --severity` | Filter cuts by severity | `papercuts list --severity` | `list_filters_sorts_limits_since_and_markdown` |
| Dispositions `duplicate` / `already_fixed` / `deferred` | Read as dispositions; no reference command writes them | Read and mapped in `feedback::is_terminal` (`resolved`, `wontfix`, `duplicate`, `already_fixed`) and `feedback::cut_status` / `clip_status` (`duplicate` and `already_fixed` resolve the item, `deferred` leaves it open) | Not covered. No test writes these three dispositions |
| Clip lifecycle | Not in the reference logger | `paperclip promote`, `note`, `top`, `review` extend the model | `promote` and `note`: `clip_accepts_python_impact_vocabulary_and_writes_agent_events`. `review`: `review_joins_cuts_and_clips_on_where`. `top` is not covered by any test |

## Structural mapping, not missing behaviour

- **Two binaries.** The reference exposes `--kind cut|clip|all` on one `list`.
  Here each record kind owns its binary, and `paperclip review` is the combined
  view. Every verb still exists for both record kinds.
- **Feedback ledger location.** The reference defaults the ledger into a
  workspace-specific vault path. A standalone CLI cannot assume that layout, so
  the ledger is discovered in four steps, first match wins:
  1. `FEEDBACK_RESOLUTIONS_FILE`, resolved against the current directory.
  2. With an explicit `--file PATH`, `PATH`'s own directory plus
     `.feedback-resolutions.jsonl`. This branch is checked before the repository
     root, so a log passed by path keeps its ledger beside it.
  3. The repository root plus `.feedback-resolutions.jsonl`.
  4. `$HOME/.papercuts/feedback-resolutions.jsonl` outside a repository, or when
     `HOME` is unset the command fails with a config error naming all three
     alternatives.
- **`--kind` on resolve.** `papercuts resolve` resolves cuts and
  `paperclip resolve` resolves clips, because each binary only discovers its own
  log file. The verb itself is complete; only the single-argument-parser
  convenience of addressing both kinds at once is not reproduced.

## Running the checks

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
