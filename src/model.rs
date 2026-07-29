use serde::{Deserialize, Serialize};

/// Full analysis JSON schema version.
pub const ANALYSIS_SCHEMA_VERSION: u32 = 2;
/// Legacy compression-ledger analysis schema version.
pub const LEDGER_SCHEMA_VERSION: u32 = 1;
/// Database and manifest JSON schema version.
pub const DATABASE_SCHEMA_VERSION: u32 = 1;
/// Stable identifier for every metric-affecting protocol-v1 rule.
pub const PROTOCOL_ID: &str = "codensity-zstd19-concat-v1";
/// Stable identifier for the multi-signal profile rules.
pub const PROFILE_PROTOCOL_ID: &str = "codensity-information-profile-v2";

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
    /// Multi-signal information-density profile.
    ///
    /// This is absent only in frozen schema-v1 database ledgers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<InformationProfile>,
}

/// Compression result from one algorithm/configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompressionMeasurement {
    /// Stable algorithm name.
    pub algorithm: String,
    /// Stable human-readable configuration.
    pub configuration: String,
    /// Compressed stream size.
    pub compressed_bytes: u64,
    /// `compressed_bytes / original_bytes`.
    pub ratio: f64,
}

/// One point on the zstd compression-level curve.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompressionCurvePoint {
    /// zstd compression level.
    pub level: i32,
    /// Compressed stream size.
    pub compressed_bytes: u64,
    /// `compressed_bytes / original_bytes`.
    pub ratio: f64,
}

/// Cross-compressor and compression-curve measurements.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompressionProfile {
    /// Comparable high-compression measurements in stable algorithm order.
    pub algorithms: Vec<CompressionMeasurement>,
    /// zstd levels 1, 3, 9, 19, and 22.
    pub zstd_curve: Vec<CompressionCurvePoint>,
    /// Median ratio across the high-compression algorithm measurements.
    pub consensus_ratio: f64,
    /// Largest minus smallest high-compression ratio.
    pub ratio_spread: f64,
}

/// Byte-level Shannon entropy measurements.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntropyProfile {
    /// Shannon entropy over the complete source stream.
    pub bits_per_byte: f64,
    /// Bytes covered by windows above the noise threshold.
    pub high_entropy_bytes: u64,
    /// High-entropy bytes divided by original bytes.
    pub high_entropy_ratio: f64,
}

/// Language-independent exact-copy approximation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DuplicationProfile {
    /// Stable detector identifier.
    pub detector: String,
    /// Rolling fingerprint window size.
    pub window_bytes: u32,
    /// Bytes covered by repeated sampled windows.
    pub duplicate_bytes: u64,
    /// Duplicate bytes divided by original bytes.
    pub duplicate_ratio: f64,
}

/// Heuristic non-source/noise risk measurements.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NoiseProfile {
    /// Bytes in high-entropy windows.
    pub high_entropy_bytes: u64,
    /// Bytes in long random-looking tokens.
    pub random_token_bytes: u64,
    /// Bytes in minified-looking files.
    pub minified_file_bytes: u64,
    /// Bytes in files with generated-source markers.
    pub generated_marker_bytes: u64,
    /// Union of all flagged byte ranges.
    pub flagged_bytes: u64,
    /// Flagged bytes divided by original bytes.
    pub noise_ratio: f64,
}

/// Distribution of source bytes across files.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StructureProfile {
    /// Median recognized source-file size.
    pub median_file_bytes: u64,
    /// Nearest-rank 95th percentile source-file size.
    pub p95_file_bytes: u64,
    /// Share of bytes in the largest file.
    pub largest_file_share: f64,
    /// Share of bytes in the ten largest files.
    pub top_10_file_share: f64,
    /// Gini coefficient of recognized source-file sizes.
    pub gini: f64,
    /// Whether at least half the files are needed to account for 80% of bytes.
    pub long_tail: bool,
}

/// Comparison with the frozen OSS language cohort.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LanguageBaseline {
    /// Canonical language name.
    pub language: String,
    /// Bytes of this language in the analyzed project.
    pub source_bytes: u64,
    /// Project zstd-19 ratio for this language.
    pub project_ratio: f64,
    /// Number of qualifying OSS snapshots in the baseline.
    pub sample_count: u64,
    /// Median zstd-19 ratio in the qualifying cohort.
    pub median_ratio: f64,
    /// Percentile of the project ratio; only reported with at least three samples.
    pub percentile: Option<f64>,
}

/// Fixed score weights. No additive component exceeds 30%.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScoreWeights {
    /// Language-normalized compression contribution.
    pub compression: f64,
    /// Entropy contribution.
    pub entropy: f64,
    /// Non-duplication contribution.
    pub uniqueness: f64,
    /// Noise-free signal contribution.
    pub signal: f64,
    /// File-distribution contribution.
    pub distribution: f64,
}

/// Component values and composite information-density score.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScoreProfile {
    /// Composite score in `[0, 100]`.
    pub information_density: f64,
    /// `low`, `medium`, or `high` baseline confidence.
    pub confidence: String,
    /// Noise-adjusted language-normalized compression component.
    pub compression: f64,
    /// Code-range entropy component.
    pub entropy: f64,
    /// Non-duplication component.
    pub uniqueness: f64,
    /// Non-noise component.
    pub signal: f64,
    /// File-distribution component.
    pub distribution: f64,
    /// Fixed additive weights.
    pub weights: ScoreWeights,
    /// `low`, `medium`, or `high` repetition/template risk.
    pub template_repetition_risk: String,
}

/// Complete schema-v2 information profile.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InformationProfile {
    /// Profile protocol identifier.
    pub protocol: String,
    /// Cross-compressor results.
    pub compression: CompressionProfile,
    /// Entropy results.
    pub entropy: EntropyProfile,
    /// Exact-copy approximation.
    pub duplication: DuplicationProfile,
    /// Noise risk.
    pub noise: NoiseProfile,
    /// File-size distribution.
    pub structure: StructureProfile,
    /// Per-language normalization against the frozen cohort.
    pub baselines: Vec<LanguageBaseline>,
    /// Component and composite scores.
    pub score: ScoreProfile,
    /// Scientific interpretation boundary.
    pub interpretation: String,
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
    if let Some(profile) = &result.profile {
        output.push_str("profile:\n");
        output.push_str(&format!("  protocol: {}\n", profile.protocol));
        output.push_str(&format!(
            "  information_density: {:.2}\n  confidence: {}\n",
            profile.score.information_density, profile.score.confidence
        ));
        output.push_str(&format!(
            "  components: compression={:.2} entropy={:.2} uniqueness={:.2} signal={:.2} distribution={:.2}\n",
            profile.score.compression,
            profile.score.entropy,
            profile.score.uniqueness,
            profile.score.signal,
            profile.score.distribution
        ));
        output.push_str("  compression:\n");
        for measurement in &profile.compression.algorithms {
            output.push_str(&format!(
                "    {} ({}): bytes={} ratio={:.6}\n",
                measurement.algorithm,
                measurement.configuration,
                measurement.compressed_bytes,
                measurement.ratio
            ));
        }
        output.push_str(&format!(
            "    consensus_ratio: {:.6}\n    ratio_spread: {:.6}\n",
            profile.compression.consensus_ratio, profile.compression.ratio_spread
        ));
        output.push_str("  zstd_curve:\n");
        for point in &profile.compression.zstd_curve {
            output.push_str(&format!(
                "    level {}: bytes={} ratio={:.6}\n",
                point.level, point.compressed_bytes, point.ratio
            ));
        }
        output.push_str(&format!(
            "  entropy: {:.4} bits/byte (high_entropy={:.2}%)\n",
            profile.entropy.bits_per_byte,
            profile.entropy.high_entropy_ratio * 100.0
        ));
        output.push_str(&format!(
            "  duplication: {:.2}% ({})\n",
            profile.duplication.duplicate_ratio * 100.0,
            profile.duplication.detector
        ));
        output.push_str(&format!(
            "  noise: {:.2}%\n  structure: largest={:.2}% top10={:.2}% gini={:.4} long_tail={}\n",
            profile.noise.noise_ratio * 100.0,
            profile.structure.largest_file_share * 100.0,
            profile.structure.top_10_file_share * 100.0,
            profile.structure.gini,
            profile.structure.long_tail
        ));
        output.push_str(&format!(
            "  template_repetition_risk: {}\n",
            profile.score.template_repetition_risk
        ));
        if !profile.baselines.is_empty() {
            output.push_str("  baselines:\n");
            for baseline in &profile.baselines {
                let percentile = baseline.percentile.map_or_else(
                    || "insufficient-samples".to_owned(),
                    |value| format!("{value:.2}"),
                );
                output.push_str(&format!(
                    "    {}: bytes={} ratio={:.6} median={:.6} samples={} percentile={}\n",
                    baseline.language,
                    baseline.source_bytes,
                    baseline.project_ratio,
                    baseline.median_ratio,
                    baseline.sample_count,
                    percentile
                ));
            }
        }
    }
    output
}
