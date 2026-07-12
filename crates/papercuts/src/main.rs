// paperclips — fork of treygoff24/papercuts (MIT)
// Original: https://github.com/treygoff24/papercuts
// This is the papercuts binary: a thin wrapper on paper-core.

use clap::Parser;
use paper_core::cli::Cli;
use paper_core::error::AppError;

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
                    "Run `papercuts --help` or `papercuts schema` for accepted commands and values.",
                );
                std::process::exit(paper_core::output::write_error(&app_error));
            }
        },
    };
    let code = paper_core::effective_now()
        .and_then(|now| paper_core::commands::run(cli, now))
        .unwrap_or_else(|error| paper_core::output::write_error(&error));
    std::process::exit(code);
}
