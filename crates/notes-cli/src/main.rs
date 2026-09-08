use clap::{CommandFactory, Parser, Subcommand};
use notes_core::ErrorCode;
use notes_core::{Error, Library, Result};
use serde_json::{Value, json};
use std::{
    io::{self, IsTerminal, Write},
    path::PathBuf,
    process::ExitCode,
};
#[derive(Parser)]
#[command(
    name = "notes",
    version,
    about = "Foglio: local-first Markdown notes",
    after_help = "Linux local filesystems only. Delete is permanent: --yes or interactive confirmation required. Paths are library-relative .md paths; no IDs or adoption required. Exit codes: 0 success; 1 operational/partial; 2 usage; 3 not found; 4 conflict/busy. JSON emits result, diagnostics, incomplete, error. Search is literal by default; --phrase/--prefix are explicit. rescan/reindex read all notes. No sync/watcher commands."
)]
struct Cli {
    #[arg(long, global = true)]
    library: Option<PathBuf>,
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Option<Command>,
}
#[derive(Subcommand)]
enum Command {
    /// Create/select a library without modifying existing Markdown files.
    Init {
        directory: PathBuf,
    },
    /// Create an ordinary Markdown note. Filename defaults to title with spaces replaced by hyphens.
    New {
        title: String,
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        body: Option<String>,
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    List,
    /// Search title, body, path and tags through the disposable index.
    Search {
        query: String,
        #[arg(long, conflicts_with = "prefix")]
        phrase: bool,
        #[arg(long)]
        prefix: bool,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        folder: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// Content-check every note and reconcile the index (no adoption).
    Rescan,
    /// Reconstruct all derived tables from Markdown (no note writes).
    Reindex,
    /// Reconcile metadata candidates and report actual index/process state.
    Status,
    /// Print the complete Markdown document.
    Show {
        path: String,
    },
    /// Move to a complete relative .md destination without overwriting.
    Move {
        path: String,
        destination: String,
    },
    /// Permanently delete one note.
    Delete {
        path: String,
        #[arg(long)]
        yes: bool,
    },
    Tags,
    Tag {
        #[command(subcommand)]
        command: TagCommand,
    },
}
#[derive(Subcommand)]
enum TagCommand {
    Add { path: String, tag: String },
    Remove { path: String, tag: String },
}
struct Output {
    result: Value,
    diagnostics: Value,
    incomplete: bool,
    human: String,
}
fn execute(cli: &Cli) -> Result<Output> {
    let command = cli.command.as_ref().unwrap();
    if let Command::Init { directory } = command {
        if cli.library.is_some() {
            return Err(Error::new(
                ErrorCode::Usage,
                "init directory and --library are mutually exclusive",
            ));
        }
        let (lib, r) = Library::init(directory)?;
        return Ok(Output {
            result: json!({"root":lib.root(),"notes":r.summaries()}),
            diagnostics: json!(r.diagnostics),
            incomplete: r.incomplete,
            human: format!(
                "Selected {}\n{} notes\n",
                lib.root().display(),
                r.notes.len()
            ),
        });
    }
    let lib = Library::resolve(cli.library.as_deref())?;
    let mut diagnostics = json!([]);
    let mut incomplete = false;
    let (result, human) = match command {
        Command::Init { .. } => unreachable!(),
        Command::Search {
            query,
            phrase,
            prefix,
            tag,
            folder,
            limit,
        } => {
            use notes_core::search::{SearchMode, SearchQuery};
            let q = SearchQuery {
                text: query.clone(),
                mode: if *phrase {
                    SearchMode::Phrase
                } else if *prefix {
                    SearchMode::Prefix
                } else {
                    SearchMode::Literal
                },
                tag: tag.clone(),
                folder: folder.clone(),
                limit: *limit,
            };
            let report = lib.search(&q)?;
            diagnostics = json!(report.status.diagnostics);
            incomplete = report.status.incomplete;
            let human = report
                .hits
                .iter()
                .map(|h| format!("{}\t{}\n{}\n", h.path, h.title, h.snippet))
                .collect();
            (json!(report), human)
        }
        Command::Rescan | Command::Reindex | Command::Status => {
            let status = match command {
                Command::Rescan => lib.rescan()?,
                Command::Reindex => lib.reindex()?,
                _ => lib.status()?,
            };
            diagnostics = json!(status.diagnostics);
            incomplete = status.incomplete;
            let human = format!(
                "{} discovered; {} indexed; {} stale\n{} parsed; {} reused\nWatcher: inactive (this process)\n",
                status.discovered_notes,
                status.indexed_notes,
                status.stale_notes,
                status.parsed_notes,
                status.reused_notes
            );
            (json!(status), human)
        }
        Command::New {
            title,
            path,
            body,
            tags,
        } => {
            notes_core::validate_note_title(title)?;
            let path = path
                .clone()
                .map_or_else(|| notes_core::default_note_path(title), Ok)?;
            let e = lib.create(
                &path,
                &body.clone().unwrap_or_else(|| format!("# {title}\n")),
                tags,
            )?;
            (json!(e), format!("{}\n", e.path))
        }
        Command::List | Command::Tags => {
            let r = lib.scan()?;
            diagnostics = json!(r.diagnostics);
            incomplete = r.incomplete;
            if matches!(command, Command::Tags) {
                let mut tags: Vec<_> = r
                    .notes
                    .iter()
                    .flat_map(|e| e.document.tags.clone())
                    .collect();
                tags.sort();
                tags.dedup();
                (json!(tags), tags.join("\n") + "\n")
            } else {
                let human = r
                    .notes
                    .iter()
                    .map(|e| format!("{}\t{}\n", e.path, e.document.title))
                    .collect();
                (json!(r.summaries()), human)
            }
        }
        Command::Show { path } => {
            let e = lib.get(path)?;
            (json!(e), e.document.source)
        }
        Command::Move { path, destination } => {
            let e = lib.get(path)?;
            let c = lib.move_note(path, &e.document.revision, destination)?;
            incomplete = !c.durability_confirmed;
            (json!(c), format!("Moved to {destination}\n"))
        }
        Command::Delete { path, yes } => {
            let e = lib.get(path)?;
            if !yes {
                if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
                    return Err(Error::new(
                        ErrorCode::Usage,
                        "permanent deletion requires --yes in noninteractive use",
                    ));
                }
                eprint!("Permanently delete {}? Type yes: ", e.path);
                io::stderr().flush()?;
                let mut answer = String::new();
                io::stdin().read_line(&mut answer)?;
                if answer.trim() != "yes" {
                    return Err(Error::new(ErrorCode::Cancelled, "deletion cancelled"));
                }
            }
            let c = lib.delete(path, &e.document.revision)?;
            incomplete = !c.durability_confirmed;
            (json!(c), format!("Deleted {}\n", e.path))
        }
        Command::Tag { command } => {
            let (path, tag, add) = match command {
                TagCommand::Add { path, tag } => (path, tag, true),
                TagCommand::Remove { path, tag } => (path, tag, false),
            };
            let e = lib.get(path)?;
            let c = lib.tag(path, &e.document.revision, tag, add)?;
            incomplete = !c.durability_confirmed;
            (json!(c), format!("Updated tags for {}\n", e.path))
        }
    };
    Ok(Output {
        result,
        diagnostics,
        incomplete,
        human,
    })
}
fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().collect();
    let wants_json = args
        .iter()
        .take_while(|arg| *arg != "--")
        .any(|arg| arg == "--json");
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) if !error.use_stderr() => {
            let _ = error.print();
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            if wants_json {
                println!(
                    "{}",
                    json!({"result":null,"diagnostics":[],"incomplete":true,
                    "error": Error::new(ErrorCode::Usage, error.to_string())})
                );
            } else {
                let _ = error.print();
            }
            return ExitCode::from(2);
        }
    };
    if cli.command.is_none() {
        let _ = Cli::command().print_help();
        return ExitCode::SUCCESS;
    }
    match execute(&cli) {
        Ok(o) => {
            if cli.json {
                println!(
                    "{}",
                    json!({"result":o.result,"diagnostics":o.diagnostics,"incomplete":o.incomplete,"error":null})
                );
            } else {
                print!("{}", o.human);
                if let Some(ds) = o.diagnostics.as_array() {
                    for d in ds {
                        eprintln!(
                            "{}: {}: {}",
                            d["path"].as_str().unwrap_or(""),
                            d["code"].as_str().unwrap_or(""),
                            d["message"].as_str().unwrap_or("")
                        );
                    }
                }
                if o.incomplete {
                    eprintln!("incomplete: inspect diagnostics or commit durability");
                }
            }
            ExitCode::from(u8::from(o.incomplete))
        }
        Err(e) => {
            let code = match e.code.as_str() {
                "usage" | "path" | "metadata" => 2,
                "not_found" => 3,
                "conflict" | "ambiguous" | "busy" | "exists" => 4,
                _ => 1,
            };
            if cli.json {
                println!(
                    "{}",
                    json!({"result":null,"diagnostics":[],"incomplete":true,"error":e})
                );
            } else {
                eprintln!("{e}");
            }
            ExitCode::from(code)
        }
    }
}
