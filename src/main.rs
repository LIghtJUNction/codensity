use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use codensity::{
    analyze_github_repository, analyze_granular_path, analyze_ledger_path, analyze_path,
    build_database, compare_github_repositories, initialize_project, relate_paths,
    render_granular_analysis, render_relation, render_repository_analysis,
    render_repository_comparison, render_text, safe_input_label, update_database,
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
        /// Local source path or a public GitHub repository URL.
        #[arg(default_value = "src")]
        input: String,
        /// Output representation.
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        /// Run only the frozen single-zstd schema-v1 ledger.
        #[arg(long)]
        ledger_only: bool,
        /// Result detail level for local paths and GitHub repository URLs.
        #[arg(long, value_enum, default_value_t = Granularity::Repository)]
        granularity: Granularity,
    },
    /// Measure shared byte-level patterns between two source files.
    Relation {
        /// Root directory whose canonical source scan selects both files.
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// First root-relative source path.
        first: PathBuf,
        /// Second root-relative source path.
        second: PathBuf,
        /// Output representation.
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    /// Compare two public GitHub repositories by byte-level source regularity.
    Compare {
        /// First public GitHub repository URL.
        first: String,
        /// Second public GitHub repository URL.
        second: String,
        /// Include parser-backed Rust function-pair candidates.
        #[arg(long, value_enum, default_value_t = Granularity::Repository)]
        granularity: Granularity,
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

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Granularity {
    Repository,
    File,
    Function,
}

fn main() -> Result<()> {
    run(Cli::parse().command)
}

fn run(command: Command) -> Result<()> {
    match command {
        Command::Init { path, force } => {
            initialize_project(&path, force)?;
            println!(
                "initialized: {}",
                path.join(".codensity/analysis.json").display()
            );
        }
        Command::Analyze {
            input,
            format,
            ledger_only,
            granularity,
        } => {
            if input.starts_with("https://github.com/") {
                if ledger_only {
                    anyhow::bail!("--ledger-only is only available for local source paths");
                }
                let result = analyze_github_repository(
                    &input,
                    !matches!(granularity, Granularity::Repository),
                    matches!(granularity, Granularity::Function),
                )?;
                match format {
                    OutputFormat::Text => print!("{}", render_repository_analysis(&result)),
                    OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&result)?),
                }
                return Ok(());
            }
            let path = PathBuf::from(input);
            let label = safe_input_label(&path)?;
            if matches!(granularity, Granularity::Repository) {
                let result = if ledger_only {
                    analyze_ledger_path(&path, &label)?
                } else {
                    analyze_path(&path, &label)?
                };
                match format {
                    OutputFormat::Text => print!("{}", render_text(&result)),
                    OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&result)?),
                }
            } else {
                if ledger_only {
                    anyhow::bail!(
                        "--ledger-only cannot be combined with --granularity file or function"
                    );
                }
                let result = analyze_granular_path(
                    &path,
                    &label,
                    true,
                    matches!(granularity, Granularity::Function),
                )?;
                match format {
                    OutputFormat::Text => print!("{}", render_granular_analysis(&result)),
                    OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&result)?),
                }
            }
        }
        Command::Relation {
            root,
            first,
            second,
            format,
        } => {
            let result = relate_paths(&root, &first, &second)?;
            match format {
                OutputFormat::Text => print!("{}", render_relation(&result)),
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&result)?),
            }
        }
        Command::Compare {
            first,
            second,
            granularity,
            format,
        } => {
            if matches!(granularity, Granularity::File) {
                anyhow::bail!(
                    "compare --granularity file is not available; use analyze <repository-url> --granularity file"
                );
            }
            let result = compare_github_repositories(
                &first,
                &second,
                matches!(granularity, Granularity::Function),
            )?;
            match format {
                OutputFormat::Text => print!("{}", render_repository_comparison(&result)),
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

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Command, Granularity, OutputFormat};

    #[test]
    fn relation_command_parses_root_paths_and_json_format() {
        let cli = Cli::try_parse_from([
            "codensity",
            "relation",
            "--root",
            "project",
            "src/b.rs",
            "src/a.rs",
            "--format",
            "json",
        ])
        .expect("parse relation command");
        match cli.command {
            Command::Relation {
                root,
                first,
                second,
                format,
            } => {
                assert_eq!(root, std::path::PathBuf::from("project"));
                assert_eq!(first, std::path::PathBuf::from("src/b.rs"));
                assert_eq!(second, std::path::PathBuf::from("src/a.rs"));
                assert!(matches!(format, OutputFormat::Json));
            }
            _ => panic!("expected relation command"),
        }
    }

    #[test]
    fn github_commands_parse_granularity_without_local_path_coercion() {
        let analyze = Cli::try_parse_from([
            "codensity",
            "analyze",
            "https://github.com/BurntSushi/ripgrep",
            "--granularity",
            "function",
        ])
        .expect("parse GitHub analyze command");
        let compare = Cli::try_parse_from([
            "codensity",
            "compare",
            "https://github.com/BurntSushi/ripgrep",
            "https://github.com/serde-rs/serde",
            "--granularity",
            "function",
        ])
        .expect("parse GitHub compare command");

        assert!(matches!(
            analyze.command,
            Command::Analyze {
                input,
                granularity: Granularity::Function,
                ..
            } if input == "https://github.com/BurntSushi/ripgrep"
        ));
        assert!(matches!(
            compare.command,
            Command::Compare {
                granularity: Granularity::Function,
                ..
            }
        ));
    }
}
