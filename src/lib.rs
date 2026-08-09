//! Language-independent source-code information-density analysis.
//!
//! Codensity scans recognized source files, orders them by normalized relative
//! path, preserves the frozen [`PROTOCOL_ID`] compression ledger, and adds a
//! multi-compressor, entropy, duplication, noise, structure, and baseline
//! profile. Database generation intentionally retains the schema-v1 ledger.

mod analyzer;
mod baseline;
mod comparison;
mod database;
mod error;
mod function;
mod init;
mod language;
mod model;
mod profile;
mod release;
mod repository;

pub use analyzer::{
    analyze_granular_path, analyze_ledger_path, analyze_path, relate_paths, safe_input_label,
};
pub use comparison::{analyze_github_repository, compare_github_repositories};
pub use database::{build_database, load_manifest};
pub use error::{CodensityError, Result};
pub use init::{
    CacheStatus, InitializationResult, clean_project, initialize_project,
    initialize_project_with_status,
};
pub use language::{LANGUAGES, LanguageSpec, language_for_path};
pub use model::{
    ANALYSIS_SCHEMA_VERSION, AnalysisResult, CompressionCurvePoint, CompressionMeasurement,
    CompressionProfile, DATABASE_SCHEMA_VERSION, Database, DatabaseProject, DuplicationProfile,
    EntropyProfile, FileResult, FunctionResult, FunctionSimilarityResult,
    GRANULAR_ANALYSIS_PROTOCOL_ID, InformationProfile, LEDGER_SCHEMA_VERSION, LanguageBaseline,
    LanguageResult, Manifest, ManifestProject, MetricResult, NoiseProfile, PROFILE_PROTOCOL_ID,
    PROTOCOL_ID, RELATION_PROTOCOL_ID, RELATION_SCHEMA_VERSION, REPOSITORY_ANALYSIS_PROTOCOL_ID,
    REPOSITORY_ANALYSIS_SCHEMA_VERSION, REPOSITORY_COMPARISON_PROTOCOL_ID,
    REPOSITORY_COMPARISON_SCHEMA_VERSION, RUST_FUNCTION_PROTOCOL_ID, RelationFileResult,
    RelationResult, RepositoryAnalysisResult, RepositoryComparisonResult, RepositoryProvenance,
    ScoreProfile, ScoreWeights, StructureProfile, render_granular_analysis, render_relation,
    render_repository_analysis, render_repository_comparison, render_text,
};
pub use release::update_database;

/// The package version embedded in every result.
pub const CODENSITY_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the linked zstd runtime version embedded in every result.
#[must_use]
pub fn zstd_version() -> &'static str {
    zstd::zstd_safe::version_string()
}
