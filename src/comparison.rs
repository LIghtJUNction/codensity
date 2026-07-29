use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::analyzer::{analyze_granular_path, analyze_path, metric_for_bytes, scan, stream_metric};
use crate::function::{ExtractedFunction, extract_rust_functions};
use crate::model::{
    FunctionSimilarityResult, REPOSITORY_ANALYSIS_PROTOCOL_ID, REPOSITORY_ANALYSIS_SCHEMA_VERSION,
    REPOSITORY_COMPARISON_PROTOCOL_ID, REPOSITORY_COMPARISON_SCHEMA_VERSION,
    RepositoryAnalysisResult, RepositoryComparisonResult, RepositoryProvenance,
};
use crate::repository::GithubSnapshot;
use crate::{CodensityError, Result};

const FUNCTION_CANDIDATE_LIMIT: usize = 1_024;
const FINGERPRINT_WINDOW_BYTES: usize = 64;

/// Downloads one immutable public GitHub snapshot and analyzes repository/file/function levels.
pub fn analyze_github_repository(
    input: &str,
    include_files: bool,
    include_functions: bool,
) -> Result<RepositoryAnalysisResult> {
    let snapshot = crate::repository::fetch_github_snapshot(input)?;
    let analysis = analyze_granular_path(
        snapshot.root(),
        &snapshot.repository_url,
        include_files,
        include_functions,
    )?;
    Ok(RepositoryAnalysisResult {
        schema_version: REPOSITORY_ANALYSIS_SCHEMA_VERSION,
        protocol: REPOSITORY_ANALYSIS_PROTOCOL_ID.to_owned(),
        provenance: RepositoryProvenance {
            repository_url: snapshot.repository_url,
            commit: snapshot.commit,
            archive_sha256: snapshot.archive_sha256,
        },
        analysis,
        interpretation: "The repository was resolved to an immutable GitHub commit before scanning. File metrics are independent source streams and function metrics are parser-backed Rust spans; neither is a code-quality or semantic-equivalence result.".to_owned(),
    })
}

/// Downloads two immutable public GitHub snapshots and compares their source streams.
pub fn compare_github_repositories(
    first: &str,
    second: &str,
    include_functions: bool,
) -> Result<RepositoryComparisonResult> {
    let first = crate::repository::fetch_github_snapshot(first)?;
    let second = crate::repository::fetch_github_snapshot(second)?;
    compare_snapshots(&first, &second, include_functions)
}

/// Compares two immutable GitHub repository snapshots with a deterministic byte stream.
pub(crate) fn compare_snapshots(
    first: &GithubSnapshot,
    second: &GithubSnapshot,
    include_functions: bool,
) -> Result<RepositoryComparisonResult> {
    let mut repositories = [snapshot_input(first), snapshot_input(second)];
    repositories.sort_by(|left, right| {
        provenance_key(&left.provenance).cmp(&provenance_key(&right.provenance))
    });
    let first = &repositories[0];
    let second = &repositories[1];
    let first_analysis = analyze_path(first.root, &first.provenance.repository_url)?;
    let second_analysis = analyze_path(second.root, &second.provenance.repository_url)?;
    let (first_sources, _) = scan(first.root)?;
    let (second_sources, _) = scan(second.root)?;
    let mut combined_sources = Vec::with_capacity(first_sources.len() + second_sources.len());
    combined_sources.extend(first_sources.iter().cloned());
    combined_sources.extend(second_sources.iter().cloned());
    let combined = stream_metric(first.root, &combined_sources, None)?;
    let empty_frame_bytes = metric_for_bytes(&[], first.root)?.compressed_bytes;
    let (
        raw_cross_stream_gain_bytes,
        adjusted_cross_stream_gain_bytes,
        adjusted_cross_stream_gain_ratio,
    ) = cross_stream_gain(
        first_analysis.overall.compressed_bytes,
        second_analysis.overall.compressed_bytes,
        combined.compressed_bytes,
        empty_frame_bytes,
        first.root,
    )?;
    let (function_similarities, function_similarity_truncated) = if include_functions {
        let first_functions = extract_rust_functions(&first_sources)?;
        let second_functions = extract_rust_functions(&second_sources)?;
        function_similarities(
            &first_functions,
            &second_functions,
            empty_frame_bytes,
            first.root,
        )?
    } else {
        (Vec::new(), false)
    };

    Ok(RepositoryComparisonResult {
        schema_version: REPOSITORY_COMPARISON_SCHEMA_VERSION,
        protocol: REPOSITORY_COMPARISON_PROTOCOL_ID.to_owned(),
        first: first.provenance.clone(),
        second: second.provenance.clone(),
        first_analysis,
        second_analysis,
        combined,
        empty_frame_bytes,
        raw_cross_stream_gain_bytes,
        adjusted_cross_stream_gain_bytes,
        adjusted_cross_stream_gain_ratio,
        function_similarities,
        function_candidate_limit: FUNCTION_CANDIDATE_LIMIT as u64,
        function_similarity_truncated,
        interpretation: "Measures shared byte-level source patterns between two immutable repository streams after fixed zstd frame overhead. Function candidates are parser-backed Rust spans selected by shared 64-byte fingerprints or exact symbols. This is not semantic equivalence, plagiarism proof, structural coupling, dependency direction, causality, or a quality score.".to_owned(),
    })
}

#[derive(Clone)]
struct SnapshotInput<'a> {
    provenance: RepositoryProvenance,
    root: &'a Path,
}

fn snapshot_input(snapshot: &GithubSnapshot) -> SnapshotInput<'_> {
    SnapshotInput {
        provenance: RepositoryProvenance {
            repository_url: snapshot.repository_url.clone(),
            commit: snapshot.commit.clone(),
            archive_sha256: snapshot.archive_sha256.clone(),
        },
        root: snapshot.root(),
    }
}

fn provenance_key(provenance: &RepositoryProvenance) -> (&str, &str, &str) {
    (
        &provenance.repository_url,
        &provenance.commit,
        &provenance.archive_sha256,
    )
}

fn cross_stream_gain(
    first: u64,
    second: u64,
    combined: u64,
    empty_frame_bytes: u64,
    root: &Path,
) -> Result<(i64, i64, Option<f64>)> {
    let independent = first
        .checked_add(second)
        .ok_or_else(|| CodensityError::CounterOverflow(root.to_path_buf()))?;
    let independent_i64 = i64::try_from(independent)
        .map_err(|_| CodensityError::CounterOverflow(root.to_path_buf()))?;
    let combined_i64 =
        i64::try_from(combined).map_err(|_| CodensityError::CounterOverflow(root.to_path_buf()))?;
    let raw = independent_i64
        .checked_sub(combined_i64)
        .ok_or_else(|| CodensityError::CounterOverflow(root.to_path_buf()))?;
    let frame_i64 = i64::try_from(empty_frame_bytes)
        .map_err(|_| CodensityError::CounterOverflow(root.to_path_buf()))?;
    let adjusted = raw
        .checked_sub(frame_i64)
        .ok_or_else(|| CodensityError::CounterOverflow(root.to_path_buf()))?;
    let payload = independent.saturating_sub(
        empty_frame_bytes
            .checked_mul(2)
            .ok_or_else(|| CodensityError::CounterOverflow(root.to_path_buf()))?,
    );
    Ok((
        raw,
        adjusted,
        (payload != 0).then_some(adjusted as f64 / payload as f64),
    ))
}

fn function_similarities(
    first: &[ExtractedFunction],
    second: &[ExtractedFunction],
    empty_frame_bytes: u64,
    root: &Path,
) -> Result<(Vec<FunctionSimilarityResult>, bool)> {
    let mut second_fingerprints: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
    let mut second_symbols: BTreeMap<(&str, &str), Vec<usize>> = BTreeMap::new();
    for (index, function) in second.iter().enumerate() {
        for fingerprint in fingerprints(&function.bytes) {
            second_fingerprints
                .entry(fingerprint)
                .or_default()
                .push(index);
        }
        second_symbols
            .entry((&function.result.kind, &function.result.symbol))
            .or_default()
            .push(index);
    }
    let mut candidates = BTreeSet::new();
    for (index, function) in first.iter().enumerate() {
        for fingerprint in fingerprints(&function.bytes) {
            if let Some(matches) = second_fingerprints.get(&fingerprint) {
                candidates.extend(matches.iter().map(|other| (index, *other)));
            }
        }
        if let Some(matches) = second_symbols.get(&(&function.result.kind, &function.result.symbol))
        {
            candidates.extend(matches.iter().map(|other| (index, *other)));
        }
    }
    let truncated = candidates.len() > FUNCTION_CANDIDATE_LIMIT;
    let mut results = Vec::with_capacity(candidates.len().min(FUNCTION_CANDIDATE_LIMIT));
    for (first_index, second_index) in candidates.into_iter().take(FUNCTION_CANDIDATE_LIMIT) {
        let left = &first[first_index];
        let right = &second[second_index];
        let mut bytes = Vec::with_capacity(left.bytes.len() + right.bytes.len());
        bytes.extend_from_slice(&left.bytes);
        bytes.extend_from_slice(&right.bytes);
        let combined = metric_for_bytes(&bytes, root)?;
        let (
            raw_cross_stream_gain_bytes,
            adjusted_cross_stream_gain_bytes,
            adjusted_cross_stream_gain_ratio,
        ) = cross_stream_gain(
            left.result.metric.compressed_bytes,
            right.result.metric.compressed_bytes,
            combined.compressed_bytes,
            empty_frame_bytes,
            root,
        )?;
        results.push(FunctionSimilarityResult {
            first: left.result.clone(),
            second: right.result.clone(),
            combined,
            raw_cross_stream_gain_bytes,
            adjusted_cross_stream_gain_bytes,
            adjusted_cross_stream_gain_ratio,
            high_variance: left.result.small_sample || right.result.small_sample,
        });
    }
    Ok((results, truncated))
}

fn fingerprints(bytes: &[u8]) -> BTreeSet<u64> {
    bytes
        .windows(FINGERPRINT_WINDOW_BYTES)
        .step_by(FINGERPRINT_WINDOW_BYTES)
        .map(|window| {
            let digest = Sha256::digest(window);
            u64::from_be_bytes(digest[..8].try_into().expect("fixed digest prefix length"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{fingerprints, function_similarities};
    use crate::analyzer::metric_for_bytes;
    use crate::function::ExtractedFunction;
    use crate::model::FunctionResult;

    fn function(kind: &str, symbol: &str, bytes: &[u8]) -> ExtractedFunction {
        ExtractedFunction {
            result: FunctionResult {
                path: "fixture.rs".to_owned(),
                kind: kind.to_owned(),
                symbol: symbol.to_owned(),
                start_line: 1,
                end_line: 1,
                small_sample: true,
                metric: metric_for_bytes(bytes, Path::new(".")).expect("measure fixture"),
            },
            bytes: bytes.to_vec(),
        }
    }

    #[test]
    fn fingerprints_are_stable_and_require_a_full_window() {
        assert!(fingerprints(b"short").is_empty());
        assert_eq!(fingerprints(&[b'x'; 128]), fingerprints(&[b'x'; 128]));
    }

    #[test]
    fn function_candidates_are_deterministic_for_symbol_and_fingerprint_matches() {
        let shared_prefix =
            b"fn shared_body() { let repeated = 1234567890; let repeated_again = 1234567890; ";
        assert!(shared_prefix.len() >= 64);
        let mut first_bytes = shared_prefix.to_vec();
        first_bytes.extend_from_slice(b"alpha(); }");
        let mut second_bytes = shared_prefix.to_vec();
        second_bytes.extend_from_slice(b"beta(); }");
        let first = vec![
            function("function", "same_symbol", b"fn same_symbol() {}"),
            function("function", "only_first", &first_bytes),
        ];
        let second = vec![
            function("function", "same_symbol", b"fn same_symbol() { 1 + 1; }"),
            function("function", "only_second", &second_bytes),
        ];
        let empty_frame_bytes = metric_for_bytes(&[], Path::new("."))
            .expect("measure empty frame")
            .compressed_bytes;

        let first_run = function_similarities(&first, &second, empty_frame_bytes, Path::new("."))
            .expect("compare functions");
        let second_run = function_similarities(&first, &second, empty_frame_bytes, Path::new("."))
            .expect("repeat comparison");

        assert_eq!(first_run, second_run);
        assert!(!first_run.1);
        assert_eq!(first_run.0.len(), 2);
        assert!(first_run.0.iter().all(|result| result.high_variance));
        assert!(first_run.0.iter().any(|result| {
            result.first.symbol == "same_symbol" && result.second.symbol == "same_symbol"
        }));
        assert!(first_run.0.iter().any(|result| {
            result.first.symbol == "only_first" && result.second.symbol == "only_second"
        }));
    }
}
