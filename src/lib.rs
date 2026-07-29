//! Deterministic source-code compression-density analysis.
//!
//! Codensity scans recognized source files, orders them by normalized relative
//! path, and compresses their concatenated raw bytes using the
//! [`PROTOCOL_ID`] protocol. [`analyze_path`] is the single analyzer used by
//! both the CLI and database generation.

mod analyzer;
mod database;
mod error;
mod init;
mod language;
mod model;
mod release;

pub use analyzer::{analyze_path, safe_input_label};
pub use database::{build_database, load_manifest};
pub use error::{CodensityError, Result};
pub use init::initialize_project;
pub use language::{LANGUAGES, LanguageSpec, language_for_path};
pub use model::{
    ANALYSIS_SCHEMA_VERSION, AnalysisResult, DATABASE_SCHEMA_VERSION, Database, DatabaseProject,
    LanguageResult, Manifest, ManifestProject, MetricResult, PROTOCOL_ID, render_text,
};
pub use release::update_database;

/// The package version embedded in every result.
pub const CODENSITY_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the linked zstd runtime version embedded in every result.
#[must_use]
pub fn zstd_version() -> &'static str {
    zstd::zstd_safe::version_string()
}
