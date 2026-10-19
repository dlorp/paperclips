// paperclips — the clips side of the agent friction log.
// Companion to papercuts: papercuts logs friction, paperclip logs wins.

use clap::Parser;
use paper_core::Impact;
use paper_core::error::AppError;

mod commands;
mod review;

#[derive(Debug, Parser)]
#[command(
    name = "paperclip",
    version,
    about = "Log wins and clips — the positive side of the agent friction log.",
    long_about = None,
    arg_required_else_help = true,
    subcommand_required = true,
    color = clap::ColorChoice::Never,
    rename_all = "kebab-case"
)]
pub struct Cli {
    #[arg(long, global = true, value_name = "PATH")]
    pub file: Option<std::path::PathBuf>,

    #[arg(long, global = true)]
    pub pretty: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    Add(AddArgs),
    List(ListArgs),
    Promote(PromoteArgs),
    Note(NoteArgs),
    Resolve(ResolveArgs),
    Wontfix(WontfixArgs),
    Top(TopArgs),
    Review(ReviewArgs),
    Schema,
    Doctor,
}

#[derive(Debug, clap::Args)]
pub struct AddArgs {
    #[arg(value_name = "TEXT")]
    pub what: Option<String>,
    #[arg(long)]
    pub agent: Option<String>,
    #[arg(long = "tag")]
    pub tags: Vec<String>,
    #[arg(long, value_enum, default_value_t = Impact::Nice)]
    pub impact: Impact,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub force: bool,
    #[arg(long = "where", value_name = "COMPONENT")]
    pub where_loc: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct ListArgs {
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long, value_enum)]
    pub impact: Option<Impact>,
    #[arg(long, value_enum, default_value_t = StatusFilter::Open)]
    pub status: StatusFilter,
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    pub format: OutputFormat,
    #[arg(long = "where", value_name = "COMPONENT")]
    pub where_loc: Option<String>,
    /// Only clips with no terminal disposition in the feedback ledger.
    #[arg(long)]
    pub unresolved: bool,
}

#[derive(Debug, clap::Args)]
pub struct PromoteArgs {
    #[arg(value_name = "ID")]
    pub id: String,
    #[arg(long)]
    pub agent: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct NoteArgs {
    #[arg(value_name = "ID")]
    pub id: String,
    #[arg(value_name = "TEXT")]
    pub text: String,
    #[arg(long)]
    pub agent: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct ResolveArgs {
    #[arg(value_name = "ID")]
    pub id: String,
    #[arg(long)]
    pub agent: Option<String>,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub resolution: Option<String>,
    #[arg(long)]
    pub linked_task: Option<String>,
    #[arg(long)]
    pub resolved_by: Option<String>,
    #[arg(long)]
    pub verification_method: Option<String>,
    #[arg(long)]
    pub verification_result: Option<String>,
    #[arg(long)]
    pub change: Option<String>,
    #[arg(long = "class", value_enum)]
    pub change_class: Option<paper_core::cli::ChangeClass>,
    #[arg(long, value_enum)]
    pub channel: Option<paper_core::cli::Channel>,
    #[arg(long)]
    pub task: Option<String>,
    #[arg(long)]
    pub owner: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct WontfixArgs {
    #[arg(value_name = "ID")]
    pub id: String,
    #[arg(long)]
    pub reason: String,
    #[arg(long)]
    pub owner: Option<String>,
    #[arg(long)]
    pub task: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct TopArgs {
    #[arg(long, default_value_t = 10)]
    pub limit: usize,
}

#[derive(Debug, clap::Args)]
pub struct ReviewArgs {
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    pub format: OutputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum StatusFilter {
    Open,
    Promoted,
    Noted,
    Resolved,
    Wontfix,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    Json,
    Md,
}

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => match error.kind() {
            clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                let _ = error.print();
                std::process::exit(0);
            }
            _ => {
                let app_error = AppError::invalid_argument(
                    error.to_string(),
                    "Run `paperclip --help` or `paperclip schema` for accepted commands and values.",
                );
                std::process::exit(paper_core::output::write_error(&app_error));
            }
        },
    };
    let code = match cli.command {
        Command::Review(args) => review::run(args.format, None, cli.file, cli.pretty)
            .unwrap_or_else(|error| paper_core::output::write_error(&error)),
        Command::Add(args) => commands::add(
            args.what,
            args.agent,
            args.tags,
            args.impact,
            args.dry_run,
            args.force,
            args.where_loc,
            cli.file,
            cli.pretty,
            paper_core::effective_now().unwrap(),
        )
        .unwrap_or_else(|error| paper_core::output::write_error(&error)),
        Command::List(args) => commands::list(
            args.tag,
            args.impact,
            args.status,
            args.limit,
            args.format,
            args.where_loc,
            args.unresolved,
            cli.file,
            cli.pretty,
        )
        .unwrap_or_else(|error| paper_core::output::write_error(&error)),
        Command::Promote(args) => commands::promote(
            args.id,
            args.agent,
            cli.file,
            cli.pretty,
            paper_core::effective_now().unwrap(),
        )
        .unwrap_or_else(|error| paper_core::output::write_error(&error)),
        Command::Note(args) => commands::note(
            args.id,
            args.text,
            args.agent,
            cli.file,
            cli.pretty,
            paper_core::effective_now().unwrap(),
        )
        .unwrap_or_else(|error| paper_core::output::write_error(&error)),
        Command::Resolve(args) => commands::resolve(
            args,
            cli.file,
            cli.pretty,
            paper_core::effective_now().unwrap(),
        )
        .unwrap_or_else(|error| paper_core::output::write_error(&error)),
        Command::Wontfix(args) => commands::wontfix(
            args,
            cli.file,
            cli.pretty,
            paper_core::effective_now().unwrap(),
        )
        .unwrap_or_else(|error| paper_core::output::write_error(&error)),
        Command::Top(args) => commands::top(args.limit, cli.file, cli.pretty)
            .unwrap_or_else(|error| paper_core::output::write_error(&error)),
        Command::Doctor => commands::doctor(cli.file, cli.pretty)
            .unwrap_or_else(|error| paper_core::output::write_error(&error)),
        Command::Schema => {
            let schema = serde_json::json!({
                "contract": 1,
                "name": "paperclip",
                "description": "Log wins and clips — the positive side of the agent friction log.",
                "commands": {
                    "add": {"flags": {"--impact": "nice|solid|huge; default nice", "--tag": "TAG; repeatable", "--agent": "NAME", "--dry-run": "boolean", "--where": "COMPONENT"}, "positional": "TEXT or -"},
                    "list": {"flags": {"--tag": "TAG", "--impact": "nice|solid|huge (trivial accepted as nice)", "--status": "open|promoted|noted|resolved|wontfix|all; default open", "--limit": "N; default 50", "--format": "json|md; default json", "--where": "COMPONENT", "--unresolved": "boolean"}},
                    "promote": {"positional": "ID", "flags": {"--agent": "NAME"}},
                    "note": {"positional": "ID TEXT", "flags": {"--agent": "NAME"}},
                    "resolve": {"positional": "ID", "flags": {"--resolution": "TEXT; CAPA required unless --change", "--linked-task": "TASK|null", "--resolved-by": "NAME; CAPA required unless --owner", "--verification-method": "TEXT; CAPA required", "--verification-result": "TEXT; CAPA required", "--change": "TEXT; legacy", "--class": "code|standard|cron|skill|doc|config; legacy", "--channel": "kanban|review|discord; legacy", "--task": "TASK; legacy", "--owner": "NAME; legacy"}},
                    "wontfix": {"positional": "ID", "flags": {"--reason": "TEXT; required", "--owner": "NAME", "--task": "TASK"}},
                    "top": {"flags": {"--limit": "N; default 10"}},
                    "review": {"flags": {"--format": "json|md; default json"}, "description": "Reads both .papercuts.jsonl and .paperclips.jsonl. Emits Fix/Keep/Overlaps digest."},
                    "schema": {"description": "Print this contract."},
                    "doctor": {"description": "Self-check."}
                },
                "record": {
                    "kind": "clip",
                    "id": "cl_<12 lowercase hex>",
                    "what": "string",
                    "tag": "string",
                    "impact": "nice|solid|huge",
                    "status": "open|promoted|noted|resolved|wontfix",
                    "ts": "RFC3339 UTC milliseconds",
                    "agent": "string",
                    "notes": ["string"]
                }
            });
            let meta = paper_core::output::Meta::new();
            if cli.pretty {
                let _ = paper_core::output::write_success(schema, true, meta);
            } else {
                let _ = paper_core::output::write_success(schema, false, meta);
            }
            0
        }
    };
    std::process::exit(code);
}
