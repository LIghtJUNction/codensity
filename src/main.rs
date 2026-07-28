use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use codensity::{analyze_ledger_path, analyze_path, build_database, render_text, safe_input_label};

#[derive(Debug, Parser)]
#[command(name = "codensity", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Analyze a source tree.
    Analyze {
        /// Source path to analyze.
        #[arg(default_value = "src")]
        path: PathBuf,
        /// Output representation.
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        /// Run only the frozen single-zstd schema-v1 ledger.
        #[arg(long)]
        ledger_only: bool,
    },
    /// Work with reproducible analysis databases.
    Database {
        #[command(subcommand)]
        command: DatabaseCommand,
    },
}

#[derive(Debug, Subcommand)]
enum DatabaseCommand {
    /// Build a database from a schema-v1 manifest.
    Build {
        /// Manifest JSON path.
        manifest: PathBuf,
        /// Atomic output JSON path.
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Analyze {
            path,
            format,
            ledger_only,
        } => {
            let label = safe_input_label(&path)?;
            let result = if ledger_only {
                analyze_ledger_path(&path, &label)?
            } else {
                analyze_path(&path, &label)?
            };
            match format {
                OutputFormat::Text => print!("{}", render_text(&result)),
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&result)?),
            }
        }
        Command::Database {
            command: DatabaseCommand::Build { manifest, output },
        } => {
            build_database(&manifest, &output)?;
        }
    }
    Ok(())
}
