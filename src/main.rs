use clap::Parser;
use papercuts::cli::Cli;
use papercuts::error::AppError;

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
                std::process::exit(papercuts::output::write_error(&app_error));
            }
        },
    };
    let code = papercuts::effective_now()
        .and_then(|now| papercuts::commands::run(cli, now))
        .unwrap_or_else(|error| papercuts::output::write_error(&error));
    std::process::exit(code);
}
