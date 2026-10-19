# Feature parity with the reference logger

`crates/paper-core` implements the full feature surface of the reference Python
logger (`papercuts.py`) that the original tools were built against. This page is
the verification checklist: every behaviour is present in Rust and backed by a
test or a live run.

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
| Recurrence is read-only | Prior records are never rewritten | Anchor scan reads raw lines, the new record carries the link | Same test asserts append-only behaviour |
| CAPA effectiveness gate | `--verification-method`, `--verification-result`, resolution, resolver are required | `resolve` rejects with `invalid_input` and lists every missing field | `resolve_requires_capa_verification_fields` |
| Legacy resolve flags | `--change`, `--class`, `--channel`, `--task`, `--owner` map into CAPA fields | Implemented with the same mapping; `--linked-task null` normalises to null | `resolve_prefix_errors_and_idempotence_are_structured` |
| Disposition ledger | Append-only `feedback-resolutions.jsonl`, latest record per id wins | `paper_core::feedback` with `FEEDBACK_RESOLUTIONS_FILE` override | `resolve_and_wontfix_gate_and_land_in_the_feedback_ledger` |
| Wontfix | `disposition=wontfix`, `reason` required, terminal | `wontfix` on both binaries; source log untouched | `wontfix_is_terminal_and_excluded_from_unresolved_and_digest` |
| Feedback join | `load_feedback_resolutions`; terminal set resolved/wontfix/duplicate/already_fixed | `feedback::load`, `feedback::is_terminal`, status overlay in both `list` commands | `python_written_cut_and_feedback_ledger_fold_in_rust` |
| `list --unresolved` | Non-terminal dispositions only | `--unresolved` on both `list` commands | `wontfix_is_terminal_and_excluded_from_unresolved_and_digest` |
| `list --digest` | Open-cut counts by severity | `papercuts list --digest` (JSON envelope or md line) | `wontfix_is_terminal_and_excluded_from_unresolved_and_digest` |
| `list --severity` | Filter cuts by severity | `papercuts list --severity` | `list_filters_sorts_limits_since_and_markdown` |
| Dispositions `duplicate` / `already_fixed` / `deferred` | Read as dispositions; no reference command writes them | Read and mapped: the first two are terminal, `deferred` leaves the item open | Unit coverage in `feedback` + fold tests |
| Clip lifecycle | Not in the reference logger | `paperclip promote`, `note`, `top`, `review` extend the model | `crates/paperclip/tests/cli.rs` |

## Structural mapping, not missing behaviour

- **Two binaries.** The reference exposes `--kind cut|clip|all` on one `list`.
  Here each record kind owns its binary, and `paperclip review` is the combined
  view. Every verb still exists for both record kinds.
- **Feedback ledger location.** The reference defaults the ledger into a
  workspace-specific vault path. A standalone CLI cannot assume that layout, so
  the default is `<repo>/.feedback-resolutions.jsonl`, or
  `$HOME/.papercuts/feedback-resolutions.jsonl` outside a repo, with
  `FEEDBACK_RESOLUTIONS_FILE` as the explicit override.
- **`--kind` on resolve.** `papercuts resolve` resolves cuts and
  `paperclip resolve` resolves clips, because each binary only discovers its own
  log file. The verb itself is complete; only the single-argument-parser
  convenience of addressing both kinds at once is not reproduced.

## Running the checks

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
