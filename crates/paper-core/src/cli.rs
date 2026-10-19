use crate::Severity;
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = env!("CARGO_PKG_NAME"),
    version,
    about,
    long_about = None,
    arg_required_else_help = true,
    subcommand_required = true,
    color = clap::ColorChoice::Never,
    rename_all = "kebab-case"
)]
pub struct Cli {
    #[arg(long, global = true, value_name = "PATH")]
    pub file: Option<PathBuf>,

    #[arg(long, global = true)]
    pub pretty: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(alias = "log")]
    Add(AddArgs),
    List(ListArgs),
    Resolve(ResolveArgs),
    Wontfix(WontfixArgs),
    Schema {
        #[arg(value_enum, default_value_t = SchemaTarget::All)]
        target: SchemaTarget,
    },
    Doctor(DoctorArgs),
}

#[derive(Debug, Args)]
pub struct DoctorArgs {
    #[arg(long)]
    pub scan: bool,
}

#[derive(Debug, Args)]
pub struct AddArgs {
    #[arg(value_name = "TEXT")]
    pub text: Option<String>,
    #[arg(long)]
    pub agent: Option<String>,
    #[arg(long = "tag")]
    pub tags: Vec<String>,
    #[arg(long, value_enum, default_value_t = Severity::Minor)]
    pub severity: Severity,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub force: bool,
    #[arg(long = "where", value_name = "COMPONENT")]
    pub where_loc: Option<String>,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long, value_enum, default_value_t = StatusFilter::Open)]
    pub status: StatusFilter,
    #[arg(long)]
    pub agent: Option<String>,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long, value_enum)]
    pub severity: Option<Severity>,
    #[arg(long)]
    pub since: Option<String>,
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    pub format: OutputFormat,
    #[arg(long = "where", value_name = "COMPONENT")]
    pub where_loc: Option<String>,
    /// Only items with no terminal disposition in the feedback ledger.
    #[arg(long)]
    pub unresolved: bool,
    /// Print open-cut severity counts and stop (unless a filter was supplied).
    #[arg(long)]
    pub digest: bool,
}

#[derive(Debug, Args)]
pub struct ResolveArgs {
    #[arg(value_name = "ID")]
    pub id: String,
    /// Freeform note stored on the resolve event in the source log.
    #[arg(long)]
    pub note: Option<String>,
    #[arg(long)]
    pub agent: Option<String>,
    #[arg(long)]
    pub dry_run: bool,
    /// What fixed the friction (CAPA: required, or legacy --change).
    #[arg(long)]
    pub resolution: Option<String>,
    /// Related task id, or the literal `null` for a no-action resolution.
    #[arg(long)]
    pub linked_task: Option<String>,
    /// Who resolved it (CAPA: required, or legacy --owner).
    #[arg(long)]
    pub resolved_by: Option<String>,
    /// How effectiveness was verified (CAPA: required).
    #[arg(long)]
    pub verification_method: Option<String>,
    /// What the verification found (CAPA: required).
    #[arg(long)]
    pub verification_result: Option<String>,
    /// Legacy: sha, path, or version identifying the fix.
    #[arg(long)]
    pub change: Option<String>,
    /// Legacy: what kind of artifact changed.
    #[arg(long = "class", value_enum)]
    pub change_class: Option<ChangeClass>,
    /// Legacy: how the fix was delivered.
    #[arg(long, value_enum)]
    pub channel: Option<Channel>,
    /// Legacy: task id (maps to linked_task).
    #[arg(long)]
    pub task: Option<String>,
    /// Legacy: resolving owner (maps to resolved_by).
    #[arg(long)]
    pub owner: Option<String>,
}

#[derive(Debug, Args)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ChangeClass {
    Code,
    Standard,
    Cron,
    Skill,
    Doc,
    Config,
}

impl ChangeClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Standard => "standard",
            Self::Cron => "cron",
            Self::Skill => "skill",
            Self::Doc => "doc",
            Self::Config => "config",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Channel {
    Kanban,
    Review,
    Discord,
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Kanban => "kanban",
            Self::Review => "review",
            Self::Discord => "discord",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum StatusFilter {
    Open,
    Resolved,
    Wontfix,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Json,
    Md,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SchemaTarget {
    All,
    Record,
    Error,
    ExitCodes,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_covers_defaults_aliases_and_globals() {
        let cli =
            Cli::try_parse_from(["papercuts", "--file", "x", "log", "ouch", "--pretty"]).unwrap();
        assert!(cli.pretty);
        assert_eq!(cli.file, Some(PathBuf::from("x")));
        let Command::Add(args) = cli.command else {
            panic!("expected add")
        };
        assert_eq!(args.text.as_deref(), Some("ouch"));
        assert_eq!(args.severity, Severity::Minor);

        let cli = Cli::try_parse_from(["papercuts", "list"]).unwrap();
        let Command::List(args) = cli.command else {
            panic!("expected list")
        };
        assert_eq!(args.status, StatusFilter::Open);
        assert_eq!(args.limit, 50);
        assert_eq!(args.format, OutputFormat::Json);
    }

    #[test]
    fn parser_rejects_bad_values_and_missing_required_id() {
        assert!(Cli::try_parse_from(["papercuts", "list", "--format", "jsonl"]).is_err());
        assert!(Cli::try_parse_from(["papercuts", "add", "x", "--severity", "critical"]).is_err());
        assert!(Cli::try_parse_from(["papercuts", "resolve"]).is_err());
        assert!(Cli::try_parse_from(["papercuts"]).is_err());
    }

    #[test]
    fn parser_accepts_every_command_and_stdin_marker() {
        for args in [
            vec!["papercuts", "add", "-"],
            vec!["papercuts", "list", "--status", "all"],
            vec!["papercuts", "resolve", "abcd"],
            vec!["papercuts", "schema", "record"],
            vec!["papercuts", "doctor"],
        ] {
            assert!(Cli::try_parse_from(args).is_ok());
        }
    }

    #[test]
    fn parser_accepts_where_flag() {
        let cli =
            Cli::try_parse_from(["papercuts", "add", "ouch", "--where", "sync-worker"]).unwrap();
        let Command::Add(args) = cli.command else {
            panic!("expected add")
        };
        assert_eq!(args.where_loc.as_deref(), Some("sync-worker"));

        let cli =
            Cli::try_parse_from(["papercuts", "list", "--where", "skills/chronicle"]).unwrap();
        let Command::List(args) = cli.command else {
            panic!("expected list")
        };
        assert_eq!(args.where_loc.as_deref(), Some("skills/chronicle"));
    }
}
