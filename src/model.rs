use serde::{Deserialize, Serialize};

/// Analysis JSON schema version.
pub const ANALYSIS_SCHEMA_VERSION: u32 = 1;
/// Database and manifest JSON schema version.
pub const DATABASE_SCHEMA_VERSION: u32 = 1;
/// Stable identifier for every metric-affecting protocol-v1 rule.
pub const PROTOCOL_ID: &str = "codensity-zstd19-concat-v1";

/// Metrics for one concatenated byte stream.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetricResult {
    /// Number of recognized files, including empty files.
    pub file_count: u64,
    /// Total raw source bytes.
    pub original_bytes: u64,
    /// Bytes emitted by the independent zstd frame.
    pub compressed_bytes: u64,
    /// `compressed_bytes / original_bytes`, or `null` for an empty stream.
    pub ratio: Option<f64>,
    /// `1 - ratio`, or `null` for an empty stream.
    pub savings: Option<f64>,
    /// SHA-256 of the exact concatenated uncompressed stream.
    pub sha256: String,
}

/// Metrics for one canonical language.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LanguageResult {
    /// Stable canonical language name.
    pub language: String,
    /// Stream metrics for this language.
    #[serde(flatten)]
    pub metric: MetricResult,
}

/// Complete schema-v1 result from one analysis.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// Analysis schema version.
    pub schema_version: u32,
    /// Codensity package version.
    pub codensity_version: String,
    /// Linked zstd runtime/library version.
    pub zstd_version: String,
    /// Metric protocol identifier.
    pub protocol: String,
    /// Logical input label that does not expose an absolute local path.
    pub input_label: String,
    /// Overall concatenated source-stream metrics.
    pub overall: MetricResult,
    /// Per-language metrics in canonical language-table order.
    pub languages: Vec<LanguageResult>,
    /// Count of walked regular files with unknown extensions.
    pub skipped_file_count: u64,
}

/// Schema-v1 database manifest.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Manifest schema version; must be 1.
    pub schema_version: u32,
    /// Projects to analyze.
    pub projects: Vec<ManifestProject>,
}

/// One local project declaration in a manifest.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestProject {
    /// Stable project name.
    pub name: String,
    /// Stable project version.
    pub version: String,
    /// Optional pinned source revision.
    pub revision: Option<String>,
    /// Source provenance URL.
    pub source_url: String,
    /// Optional SHA-256 for the source archive.
    pub archive_sha256: Option<String>,
    /// Local extraction directory, omitted from database output.
    pub path: std::path::PathBuf,
}

/// Stable schema-v1 database.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Database {
    /// Database schema version.
    pub schema_version: u32,
    /// Codensity package version.
    pub codensity_version: String,
    /// Linked zstd runtime/library version.
    pub zstd_version: String,
    /// Metric protocol identifier.
    pub protocol: String,
    /// Analyzed projects sorted by `(name, version)`.
    pub projects: Vec<DatabaseProject>,
}

/// One project record in a database.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DatabaseProject {
    /// Project name from the manifest.
    pub name: String,
    /// Project version from the manifest.
    pub version: String,
    /// Optional source revision from the manifest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// Source provenance URL from the manifest.
    pub source_url: String,
    /// Optional source archive SHA-256 from the manifest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_sha256: Option<String>,
    /// Analysis result produced by the shared analyzer.
    pub analysis: AnalysisResult,
}

/// Renders an analysis as deterministic concise text.
#[must_use]
pub fn render_text(result: &AnalysisResult) -> String {
    let mut output = format!(
        "schema: {}\ncodensity: {}\nzstd: {}\nprotocol: {}\ninput: {}\nfiles: {}\nskipped: {}\noriginal: {}\ncompressed: {}\nratio: {:.6}\nsavings: {:.6}\nsha256: {}\nlanguages:\n",
        result.schema_version,
        result.codensity_version,
        result.zstd_version,
        result.protocol,
        result.input_label,
        result.overall.file_count,
        result.skipped_file_count,
        result.overall.original_bytes,
        result.overall.compressed_bytes,
        result.overall.ratio.unwrap_or(0.0),
        result.overall.savings.unwrap_or(0.0),
        result.overall.sha256,
    );
    for language in &result.languages {
        let ratio = language
            .metric
            .ratio
            .map_or_else(|| "null".to_owned(), |value| format!("{value:.6}"));
        let savings = language
            .metric
            .savings
            .map_or_else(|| "null".to_owned(), |value| format!("{value:.6}"));
        output.push_str(&format!(
            "  {}: files={} original={} compressed={} ratio={} savings={} sha256={}\n",
            language.language,
            language.metric.file_count,
            language.metric.original_bytes,
            language.metric.compressed_bytes,
            ratio,
            savings,
            language.metric.sha256,
        ));
    }
    output
}
