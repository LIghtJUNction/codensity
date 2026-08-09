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
/// Schema version for the two-file cross-stream relation result.
pub const RELATION_SCHEMA_VERSION: u32 = 1;
/// Stable identifier for the two-file cross-stream relation rules.
pub const RELATION_PROTOCOL_ID: &str = "codensity-cross-stream-relation-v1";
/// Schema version for repository URL analysis envelopes.
pub const REPOSITORY_ANALYSIS_SCHEMA_VERSION: u32 = 1;
/// Stable identifier for immutable GitHub snapshot analysis.
pub const REPOSITORY_ANALYSIS_PROTOCOL_ID: &str = "codensity-github-snapshot-v1";
/// Stable identifier for repository/file/function analysis envelopes.
pub const GRANULAR_ANALYSIS_PROTOCOL_ID: &str = "codensity-granular-analysis-v1";
/// Schema version for repository cross-stream comparison results.
pub const REPOSITORY_COMPARISON_SCHEMA_VERSION: u32 = 1;
/// Stable identifier for repository cross-stream comparison.
pub const REPOSITORY_COMPARISON_PROTOCOL_ID: &str = "codensity-repository-cross-stream-v1";
/// Stable identifier for parser-backed Rust function extraction.
pub const RUST_FUNCTION_PROTOCOL_ID: &str = "codensity-rust-function-ast-v1";
/// Small function samples have high fixed-frame variance below this many bytes.
pub const FUNCTION_SMALL_SAMPLE_BYTES: u64 = 512;

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

/// One selected source file in a cross-stream relation result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RelationFileResult {
    /// Normalized root-relative POSIX path.
    pub path: String,
    /// Canonical language name from the protocol table.
    pub language: String,
    /// Independent zstd-19 stream metric.
    #[serde(flatten)]
    pub metric: MetricResult,
}

/// Deterministic two-file cross-stream regularity signal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RelationResult {
    /// Relation result schema version.
    pub schema_version: u32,
    /// Codensity package version.
    pub codensity_version: String,
    /// Linked zstd runtime/library version.
    pub zstd_version: String,
    /// Relation protocol identifier.
    pub protocol: String,
    /// First selected file after canonical root-relative path sorting.
    pub first: RelationFileResult,
    /// Second selected file after canonical root-relative path sorting.
    pub second: RelationFileResult,
    /// Metric for the canonical concatenation of first then second.
    pub combined: MetricResult,
    /// Byte size of one empty zstd frame under the pinned implementation.
    pub empty_frame_bytes: u64,
    /// `C(first) + C(second) - C(combined)`, including one removed frame.
    pub raw_cross_stream_gain_bytes: i64,
    /// Raw gain less the one-frame baseline advantage.
    pub adjusted_cross_stream_gain_bytes: i64,
    /// Adjusted gain divided by independent compressed payload bytes, when non-zero.
    pub adjusted_cross_stream_gain_ratio: Option<f64>,
    /// Interpretation boundary for this non-structural signal.
    pub interpretation: String,
}

/// Immutable provenance for a downloaded GitHub repository snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RepositoryProvenance {
    /// Canonical public GitHub repository URL.
    pub repository_url: String,
    /// Resolved immutable Git commit SHA.
    pub commit: String,
    /// SHA-256 of the exact downloaded GitHub archive bytes.
    pub archive_sha256: String,
}

/// One independently compressed recognized source file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileResult {
    /// Normalized root-relative POSIX path.
    pub path: String,
    /// Canonical language name from the protocol table.
    pub language: String,
    /// Independent zstd-19 stream metric.
    #[serde(flatten)]
    pub metric: MetricResult,
}

/// One parser-backed Rust function, method, trait method, or closure.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FunctionResult {
    /// Normalized root-relative POSIX source path.
    pub path: String,
    /// Stable parser-derived kind: `function`, `method`, `trait_method`, or `closure`.
    pub kind: String,
    /// Function identifier, or a location-derived closure identifier.
    pub symbol: String,
    /// One-based first source line covered by the parsed node.
    pub start_line: u32,
    /// One-based final source line covered by the parsed node.
    pub end_line: u32,
    /// Whether the independent sample is below the fixed-frame variance threshold.
    pub small_sample: bool,
    /// Independent zstd-19 stream metric for the exact parsed source span.
    #[serde(flatten)]
    pub metric: MetricResult,
}

/// A complete repository/file/function analysis envelope.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GranularAnalysisResult {
    /// Granular-result schema version.
    pub schema_version: u32,
    /// Granular-result protocol identifier.
    pub protocol: String,
    /// Whole-repository analysis under the existing analysis protocol.
    pub repository: AnalysisResult,
    /// Independently measured recognized source files in canonical path order.
    pub files: Vec<FileResult>,
    /// Parser-backed Rust functions in canonical path and source order.
    pub functions: Vec<FunctionResult>,
    /// Function extraction protocol when parser-backed functions were requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_protocol: Option<String>,
    /// Languages present in the repository without function parser support.
    pub unsupported_function_languages: Vec<String>,
    /// Boundary for file/function metrics.
    pub interpretation: String,
}

/// Analysis of an immutable GitHub repository snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RepositoryAnalysisResult {
    /// Repository-result schema version.
    pub schema_version: u32,
    /// Repository-result protocol identifier.
    pub protocol: String,
    /// Immutable GitHub provenance for the analyzed source tree.
    pub provenance: RepositoryProvenance,
    /// Whole-repository, file, and optional function measurements.
    pub analysis: GranularAnalysisResult,
    /// Boundary for this immutable snapshot result.
    pub interpretation: String,
}

/// One function-pair compression similarity signal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FunctionSimilarityResult {
    /// Function from the first canonical repository.
    pub first: FunctionResult,
    /// Function from the second canonical repository.
    pub second: FunctionResult,
    /// Metric for exact concatenation of first then second parsed spans.
    pub combined: MetricResult,
    /// `C(first) + C(second) - C(combined)`, including one removed frame.
    pub raw_cross_stream_gain_bytes: i64,
    /// Raw gain less one empty zstd-frame baseline advantage.
    pub adjusted_cross_stream_gain_bytes: i64,
    /// Adjusted gain divided by independent compressed payload bytes, when non-zero.
    pub adjusted_cross_stream_gain_ratio: Option<f64>,
    /// Whether either function is below the fixed-frame variance threshold.
    pub high_variance: bool,
}

/// Deterministic comparison of two immutable repository source streams.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RepositoryComparisonResult {
    /// Comparison schema version.
    pub schema_version: u32,
    /// Comparison protocol identifier.
    pub protocol: String,
    /// First repository after canonical provenance sorting.
    pub first: RepositoryProvenance,
    /// Second repository after canonical provenance sorting.
    pub second: RepositoryProvenance,
    /// Whole-repository analysis for the first repository.
    pub first_analysis: AnalysisResult,
    /// Whole-repository analysis for the second repository.
    pub second_analysis: AnalysisResult,
    /// Metric for canonical concatenation of both repository source streams.
    pub combined: MetricResult,
    /// Byte size of one empty zstd frame under the pinned implementation.
    pub empty_frame_bytes: u64,
    /// `C(first) + C(second) - C(combined)`, including one removed frame.
    pub raw_cross_stream_gain_bytes: i64,
    /// Raw gain less one empty zstd-frame baseline advantage.
    pub adjusted_cross_stream_gain_bytes: i64,
    /// Adjusted gain divided by independent compressed payload bytes, when non-zero.
    pub adjusted_cross_stream_gain_ratio: Option<f64>,
    /// Parser-backed Rust function-pair candidates, when requested.
    pub function_similarities: Vec<FunctionSimilarityResult>,
    /// Maximum emitted parser-backed function-pair candidates.
    pub function_candidate_limit: u64,
    /// Whether deterministic candidate enumeration exceeded the emitted limit.
    pub function_similarity_truncated: bool,
    /// Boundary for this byte-level similarity signal.
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

/// One local or remotely acquired project declaration in a manifest.
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
    /// Optional local extraction directory, omitted from database output.
    ///
    /// When absent (or when the configured directory is unavailable),
    /// `source_url`, `revision`, and `archive_sha256` define an immutable
    /// public GitHub snapshot downloaded by `database build`.
    pub path: Option<std::path::PathBuf>,
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
    let ratio = result.overall.ratio.unwrap_or(0.0);
    let savings = result.overall.savings.unwrap_or(0.0);
    let mut output = format!(
        "Codensity summary · {}\ncompression: {} {:.2}% saved · {} → {} bytes\nfiles: {} recognized · {} skipped · sha256: {}\n",
        result.input_label,
        visual_bar(savings),
        savings * 100.0,
        result.overall.original_bytes,
        result.overall.compressed_bytes,
        result.overall.file_count,
        result.skipped_file_count,
        result.overall.sha256,
    );
    if let Some(profile) = &result.profile {
        output.push_str(&format!(
            "information_density: {:.2}/100 · confidence: {} · template risk: {}\n",
            profile.score.information_density,
            profile.score.confidence,
            profile.score.template_repetition_risk,
        ));
    }
    output.push_str("languages:\n");
    for language in result
        .languages
        .iter()
        .filter(|language| language.metric.file_count > 0)
    {
        let language_savings = language.metric.savings.unwrap_or(0.0);
        output.push_str(&format!(
            "  {:<16} {} {:>6.2}% · {} files\n",
            language.language,
            visual_bar(language_savings),
            language_savings * 100.0,
            language.metric.file_count,
        ));
    }
    output.push_str(&format!(
        "ratio: {ratio:.6}\nschema: {}\ncodensity: {}\nzstd: {}\nprotocol: {}\n",
        result.schema_version, result.codensity_version, result.zstd_version, result.protocol,
    ));
    output
}

fn visual_bar(value: f64) -> String {
    const WIDTH: usize = 12;
    let filled = (value.clamp(0.0, 1.0) * WIDTH as f64).round() as usize;
    format!("[{}{}]", "█".repeat(filled), "·".repeat(WIDTH - filled))
}

/// Renders a two-file cross-stream relation as deterministic concise text.
#[must_use]
pub fn render_relation(result: &RelationResult) -> String {
    format!(
        "schema: {}\ncodensity: {}\nzstd: {}\nprotocol: {}\nfirst: {} ({}) compressed={}\nsecond: {} ({}) compressed={}\ncombined: {}\nempty_frame: {}\nraw_cross_stream_gain: {}\nadjusted_cross_stream_gain: {}\nadjusted_cross_stream_gain_ratio: {}\ninterpretation: {}\n",
        result.schema_version,
        result.codensity_version,
        result.zstd_version,
        result.protocol,
        result.first.path,
        result.first.language,
        result.first.metric.compressed_bytes,
        result.second.path,
        result.second.language,
        result.second.metric.compressed_bytes,
        result.combined.compressed_bytes,
        result.empty_frame_bytes,
        result.raw_cross_stream_gain_bytes,
        result.adjusted_cross_stream_gain_bytes,
        result
            .adjusted_cross_stream_gain_ratio
            .map_or_else(|| "null".to_owned(), |value| format!("{value:.6}")),
        result.interpretation,
    )
}

/// Renders repository, file, and optional function metrics as deterministic text.
#[must_use]
pub fn render_granular_analysis(result: &GranularAnalysisResult) -> String {
    let mut output = render_text(&result.repository);
    output.push_str(&format!("granular_protocol: {}\nfiles:\n", result.protocol));
    for file in &result.files {
        output.push_str(&format!(
            "  {}: {} original={} compressed={} ratio={} sha256={}\n",
            file.path,
            file.language,
            file.metric.original_bytes,
            file.metric.compressed_bytes,
            format_optional_ratio(file.metric.ratio),
            file.metric.sha256,
        ));
    }
    if let Some(protocol) = &result.function_protocol {
        output.push_str(&format!("function_protocol: {protocol}\n"));
    }
    output.push_str("functions:\n");
    for function in &result.functions {
        output.push_str(&format!(
            "  {}:{}-{} {} {} original={} compressed={} ratio={} small_sample={} sha256={}\n",
            function.path,
            function.start_line,
            function.end_line,
            function.kind,
            function.symbol,
            function.metric.original_bytes,
            function.metric.compressed_bytes,
            format_optional_ratio(function.metric.ratio),
            function.small_sample,
            function.metric.sha256,
        ));
    }
    if !result.unsupported_function_languages.is_empty() {
        output.push_str(&format!(
            "unsupported_function_languages: {}\n",
            result.unsupported_function_languages.join(",")
        ));
    }
    output.push_str(&format!("interpretation: {}\n", result.interpretation));
    output
}

/// Renders one immutable GitHub snapshot analysis as deterministic text.
#[must_use]
pub fn render_repository_analysis(result: &RepositoryAnalysisResult) -> String {
    let mut output = format!(
        "schema: {}\nprotocol: {}\nrepository_url: {}\ncommit: {}\narchive_sha256: {}\n",
        result.schema_version,
        result.protocol,
        result.provenance.repository_url,
        result.provenance.commit,
        result.provenance.archive_sha256,
    );
    output.push_str(&render_granular_analysis(&result.analysis));
    output.push_str(&format!(
        "snapshot_interpretation: {}\n",
        result.interpretation
    ));
    output
}

/// Renders an immutable repository comparison as deterministic text.
#[must_use]
pub fn render_repository_comparison(result: &RepositoryComparisonResult) -> String {
    let mut output = format!(
        "schema: {}\nprotocol: {}\nfirst: {}@{} archive_sha256={}\nsecond: {}@{} archive_sha256={}\nfirst_compressed: {}\nsecond_compressed: {}\ncombined: {}\nempty_frame: {}\nraw_cross_stream_gain: {}\nadjusted_cross_stream_gain: {}\nadjusted_cross_stream_gain_ratio: {}\nfunction_candidate_limit: {}\nfunction_similarity_truncated: {}\nfunction_similarities:\n",
        result.schema_version,
        result.protocol,
        result.first.repository_url,
        result.first.commit,
        result.first.archive_sha256,
        result.second.repository_url,
        result.second.commit,
        result.second.archive_sha256,
        result.first_analysis.overall.compressed_bytes,
        result.second_analysis.overall.compressed_bytes,
        result.combined.compressed_bytes,
        result.empty_frame_bytes,
        result.raw_cross_stream_gain_bytes,
        result.adjusted_cross_stream_gain_bytes,
        format_optional_ratio(result.adjusted_cross_stream_gain_ratio),
        result.function_candidate_limit,
        result.function_similarity_truncated,
    );
    for similarity in &result.function_similarities {
        output.push_str(&format!(
            "  {}:{}:{} <-> {}:{}:{} combined={} adjusted_gain={} ratio={} high_variance={}\n",
            similarity.first.path,
            similarity.first.start_line,
            similarity.first.symbol,
            similarity.second.path,
            similarity.second.start_line,
            similarity.second.symbol,
            similarity.combined.compressed_bytes,
            similarity.adjusted_cross_stream_gain_bytes,
            format_optional_ratio(similarity.adjusted_cross_stream_gain_ratio),
            similarity.high_variance,
        ));
    }
    output.push_str(&format!("interpretation: {}\n", result.interpretation));
    output
}

fn format_optional_ratio(value: Option<f64>) -> String {
    value.map_or_else(|| "null".to_owned(), |value| format!("{value:.6}"))
}
