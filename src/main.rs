use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use codensity::{
    analyze_path, build_database, initialize_project, render_text, safe_input_label,
    update_database,
};

#[derive(Debug, Parser)]
#[command(name = "codensity", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Initialize Codensity state and record an initial project snapshot.
    Init {
        /// Project directory to initialize.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Allow filesystem-root or home-directory initialization.
        #[arg(long)]
        force: bool,
    },
    /// Analyze a source tree.
    Analyze {
        /// Source path to analyze.
        #[arg(default_value = "src")]
        path: PathBuf,
        /// Output representation.
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
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
    /// Download the official database from a GitHub Release.
    Update {
        /// Release tag to download instead of the latest release.
        #[arg(long)]
        tag: Option<String>,
        /// Local database file to atomically replace.
        #[arg(long, default_value = "database-v1.json")]
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
        Command::Init { path, force } => {
            initialize_project(&path, force)?;
            println!(
                "initialized: {}",
                path.join(".codensity/analysis.json").display()
            );
        }
        Command::Analyze { path, format } => {
            let label = safe_input_label(&path)?;
            let result = analyze_path(&path, &label)?;
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
        Command::Database {
            command: DatabaseCommand::Update { tag, output },
        } => {
            update_database(tag.as_deref(), &output)?;
            println!("updated: {}", output.display());
        }
    }
    Ok(())
}
