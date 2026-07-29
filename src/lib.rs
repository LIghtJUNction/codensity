//! Language-independent source-code information-density analysis.
//!
//! Codensity scans recognized source files, orders them by normalized relative
//! path, preserves the frozen [`PROTOCOL_ID`] compression ledger, and adds a
//! multi-compressor, entropy, duplication, noise, structure, and baseline
//! profile. Database generation intentionally retains the schema-v1 ledger.

mod analyzer;
mod baseline;
mod database;
mod error;
mod init;
mod language;
mod model;
mod profile;
mod release;

pub use analyzer::{analyze_ledger_path, analyze_path, safe_input_label};
pub use database::{build_database, load_manifest};
pub use error::{CodensityError, Result};
pub use init::initialize_project;
pub use language::{LANGUAGES, LanguageSpec, language_for_path};
pub use model::{
    ANALYSIS_SCHEMA_VERSION, AnalysisResult, CompressionCurvePoint, CompressionMeasurement,
    CompressionProfile, DATABASE_SCHEMA_VERSION, Database, DatabaseProject, DuplicationProfile,
    EntropyProfile, InformationProfile, LEDGER_SCHEMA_VERSION, LanguageBaseline, LanguageResult,
    Manifest, ManifestProject, MetricResult, NoiseProfile, PROFILE_PROTOCOL_ID, PROTOCOL_ID,
    ScoreProfile, ScoreWeights, StructureProfile, render_text,
};
pub use release::update_database;

/// The package version embedded in every result.
pub const CODENSITY_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the linked zstd runtime version embedded in every result.
#[must_use]
pub fn zstd_version() -> &'static str {
    zstd::zstd_safe::version_string()
}
