use clap::{CommandFactory, Parser};

#[derive(Parser)]
#[command(
    name = "notes",
    version,
    about = "Foglio: local-first Markdown notes (Phase 0 bootstrap)",
    after_help = "Note lifecycle commands will arrive in Phase 1. No library is opened or modified."
)]
struct Cli {}

fn main() -> std::io::Result<()> {
    Cli::parse();
    Cli::command().print_help()
}
