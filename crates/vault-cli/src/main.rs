use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use vault_core::{GraphIndex, Link, LinkStatus};
use vault_index::{build_index, concise_diagnostics, has_errors};

#[derive(Debug, Parser)]
#[command(name = "vault")]
#[command(about = "Deterministic Markdown vault graph tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Graph(GraphCommand),
}

#[derive(Debug, Parser)]
struct GraphCommand {
    #[command(subcommand)]
    command: GraphSubcommand,
}

#[derive(Debug, Subcommand)]
enum GraphSubcommand {
    Documents(GraphArgs),
    Links(GraphArgs),
    Unresolved(GraphArgs),
}

#[derive(Debug, Parser)]
struct GraphArgs {
    #[arg(long, default_value = ".")]
    root: Utf8PathBuf,
    #[arg(long, value_enum, default_value_t = OutputFormat::Jsonl)]
    format: OutputFormat,
    #[arg(long)]
    verbose: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Json,
    Jsonl,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let exit_code = run(cli)?;
    std::process::exit(exit_code);
}

fn run(cli: Cli) -> Result<i32> {
    match cli.command {
        Command::Graph(graph) => match graph.command {
            GraphSubcommand::Documents(args) => {
                let mut index = build_index(&args.root)?;
                trim_diagnostics(&mut index, args.verbose);
                write_output(&index.documents, args.format)?;
                Ok(exit_code_for(&index))
            }
            GraphSubcommand::Links(args) => {
                let mut index = build_index(&args.root)?;
                trim_diagnostics(&mut index, args.verbose);
                let links = all_links(&index);
                write_output(&links, args.format)?;
                Ok(exit_code_for(&index))
            }
            GraphSubcommand::Unresolved(args) => {
                let mut index = build_index(&args.root)?;
                trim_diagnostics(&mut index, args.verbose);
                let links = unresolved_links(&index);
                write_output(&links, args.format)?;
                Ok(exit_code_for(&index))
            }
        },
    }
}

fn trim_diagnostics(index: &mut GraphIndex, verbose: bool) {
    if verbose {
        return;
    }

    for document in &mut index.documents {
        document.diagnostics = concise_diagnostics(document);
    }
}

fn all_links(index: &GraphIndex) -> Vec<&Link> {
    index
        .documents
        .iter()
        .flat_map(|document| document.links.iter())
        .collect()
}

fn unresolved_links(index: &GraphIndex) -> Vec<&Link> {
    index
        .documents
        .iter()
        .flat_map(|document| document.links.iter())
        .filter(|link| link.status != LinkStatus::Resolved)
        .collect()
}

fn write_output<T: Serialize>(items: &[T], format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(items)?);
        }
        OutputFormat::Jsonl => {
            for item in items {
                println!("{}", serde_json::to_string(item)?);
            }
        }
    }
    Ok(())
}

fn exit_code_for(index: &GraphIndex) -> i32 {
    if has_errors(index) {
        1
    } else {
        0
    }
}
