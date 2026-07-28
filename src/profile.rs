use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use brotli::CompressorWriter;
use flate2::{Compression, GzBuilder};
use lzma_rust2::{XzOptions, XzWriter};

use crate::analyzer::{CountingWriter, SourceFile};
use crate::baseline;
use crate::error::{CodensityError, Result};
use crate::model::{
    CompressionCurvePoint, CompressionMeasurement, CompressionProfile, DuplicationProfile,
    EntropyProfile, InformationProfile, LanguageBaseline, LanguageResult, NoiseProfile,
    PROFILE_PROTOCOL_ID, ScoreProfile, ScoreWeights, StructureProfile,
};

const IO_BUFFER_SIZE: usize = 64 * 1024;
const ENTROPY_WINDOW: usize = 4096;
const ENTROPY_MIN_WINDOW: usize = 256;
const HIGH_ENTROPY_THRESHOLD: f64 = 7.5;
const FINGERPRINT_WINDOW: usize = 64;
const FINGERPRINT_SAMPLE_MASK: u64 = 0x3f;
const RANDOM_TOKEN_MIN_LENGTH: usize = 24;
const ZSTD_CURVE_LEVELS: &[i32] = &[1, 3, 9, 19, 22];

struct SignalTotals {
    byte_counts: [u64; 256],
    high_entropy_bytes: u64,
    random_token_bytes: u64,
    minified_file_bytes: u64,
    generated_marker_bytes: u64,
    flagged_bytes: u64,
    duplicate_bytes: u64,
    file_sizes: Vec<u64>,
}

impl Default for SignalTotals {
    fn default() -> Self {
        Self {
            byte_counts: [0; 256],
            high_entropy_bytes: 0,
            random_token_bytes: 0,
            minified_file_bytes: 0,
            generated_marker_bytes: 0,
            flagged_bytes: 0,
            duplicate_bytes: 0,
            file_sizes: Vec::new(),
        }
    }
}

/// Builds the deterministic schema-v2 multi-signal profile.
pub(crate) fn build_profile(
    root: &Path,
    files: &[SourceFile],
    original_bytes: u64,
    zstd_19_bytes: u64,
    languages: &[LanguageResult],
) -> Result<InformationProfile> {
    let compression = compression_profile(root, files, original_bytes, zstd_19_bytes)?;
    let signals = inspect_signals(root, files)?;
    let entropy = entropy_profile(&signals, original_bytes);
    let duplication = DuplicationProfile {
        detector: "rolling-double-fingerprint-64-sampled-v1".to_owned(),
        window_bytes: FINGERPRINT_WINDOW as u32,
        duplicate_bytes: signals.duplicate_bytes,
        duplicate_ratio: ratio(signals.duplicate_bytes, original_bytes),
    };
    let noise = NoiseProfile {
        high_entropy_bytes: signals.high_entropy_bytes,
        random_token_bytes: signals.random_token_bytes,
        minified_file_bytes: signals.minified_file_bytes,
        generated_marker_bytes: signals.generated_marker_bytes,
        flagged_bytes: signals.flagged_bytes,
        noise_ratio: ratio(signals.flagged_bytes, original_bytes),
    };
    let structure = structure_profile(&signals.file_sizes, original_bytes);
    let baselines = language_baselines(languages);
    let score = score_profile(
        &compression,
        &entropy,
        &duplication,
        &noise,
        &structure,
        &baselines,
        original_bytes,
    );

    Ok(InformationProfile {
        protocol: PROFILE_PROTOCOL_ID.to_owned(),
        compression,
        entropy,
        duplication,
        noise,
        structure,
        baselines,
        score,
        interpretation: "Measures byte-level source characteristics, not code quality, correctness, security, maintainability, or AI authorship.".to_owned(),
    })
}

fn compression_profile(
    root: &Path,
    files: &[SourceFile],
    original_bytes: u64,
    zstd_19_bytes: u64,
) -> Result<CompressionProfile> {
    let gzip_bytes = gzip_size(root, files)?;
    let brotli_bytes = brotli_size(root, files)?;
    let xz_bytes = xz_size(root, files)?;
    let algorithms = vec![
        measurement("gzip", "level=9,mtime=0", gzip_bytes, original_bytes),
        measurement("zstd", "level=19", zstd_19_bytes, original_bytes),
        measurement(
            "brotli",
            "quality=11,lgwin=22",
            brotli_bytes,
            original_bytes,
        ),
        measurement("xz", "preset=9,check=crc64", xz_bytes, original_bytes),
    ];

    let mut ratios: Vec<_> = algorithms
        .iter()
        .map(|measurement| measurement.ratio)
        .collect();
    ratios.sort_by(f64::total_cmp);
    let consensus_ratio = even_median(&ratios);
    let ratio_spread =
        ratios.last().copied().unwrap_or(0.0) - ratios.first().copied().unwrap_or(0.0);

    let mut zstd_curve = Vec::with_capacity(ZSTD_CURVE_LEVELS.len());
    for &level in ZSTD_CURVE_LEVELS {
        let compressed_bytes = if level == 19 {
            zstd_19_bytes
        } else {
            zstd_size(root, files, level)?
        };
        zstd_curve.push(CompressionCurvePoint {
            level,
            compressed_bytes,
            ratio: ratio(compressed_bytes, original_bytes),
        });
    }

    Ok(CompressionProfile {
        algorithms,
        zstd_curve,
        consensus_ratio,
        ratio_spread,
    })
}

fn measurement(
    algorithm: &str,
    configuration: &str,
    compressed_bytes: u64,
    original_bytes: u64,
) -> CompressionMeasurement {
    CompressionMeasurement {
        algorithm: algorithm.to_owned(),
        configuration: configuration.to_owned(),
        compressed_bytes,
        ratio: ratio(compressed_bytes, original_bytes),
    }
}

fn gzip_size(root: &Path, files: &[SourceFile]) -> Result<u64> {
    let counter = CountingWriter::default();
    let mut encoder = GzBuilder::new()
        .mtime(0)
        .write(counter, Compression::best());
    write_sources(root, files, &mut encoder)?;
    Ok(encoder.finish().map_err(CodensityError::Compression)?.bytes)
}

fn brotli_size(root: &Path, files: &[SourceFile]) -> Result<u64> {
    let counter = CountingWriter::default();
    let mut encoder = CompressorWriter::new(counter, IO_BUFFER_SIZE, 11, 22);
    write_sources(root, files, &mut encoder)?;
    Ok(encoder.into_inner().bytes)
}

fn xz_size(root: &Path, files: &[SourceFile]) -> Result<u64> {
    let counter = CountingWriter::default();
    let mut encoder =
        XzWriter::new(counter, XzOptions::with_preset(9)).map_err(CodensityError::Compression)?;
    write_sources(root, files, &mut encoder)?;
    Ok(encoder.finish().map_err(CodensityError::Compression)?.bytes)
}

fn zstd_size(root: &Path, files: &[SourceFile], level: i32) -> Result<u64> {
    let counter = CountingWriter::default();
    let mut encoder =
        zstd::stream::write::Encoder::new(counter, level).map_err(CodensityError::Compression)?;
    write_sources(root, files, &mut encoder)?;
    Ok(encoder.finish().map_err(CodensityError::Compression)?.bytes)
}

fn write_sources<W: Write>(_root: &Path, files: &[SourceFile], output: &mut W) -> Result<()> {
    let mut buffer = vec![0_u8; IO_BUFFER_SIZE];
    for source in files {
        let mut input =
            fs::File::open(&source.path).map_err(|source_error| CodensityError::SourceIo {
                path: source.path.clone(),
                source: source_error,
            })?;
        loop {
            let read =
                input
                    .read(&mut buffer)
                    .map_err(|source_error| CodensityError::SourceIo {
                        path: source.path.clone(),
                        source: source_error,
                    })?;
            if read == 0 {
                break;
            }
            output
                .write_all(&buffer[..read])
                .map_err(CodensityError::Compression)?;
        }
    }
    output.flush().map_err(CodensityError::Compression)
}

fn inspect_signals(root: &Path, files: &[SourceFile]) -> Result<SignalTotals> {
    let mut totals = SignalTotals::default();
    let mut seen_fingerprints = HashSet::new();
    for source in files {
        let data = fs::read(&source.path).map_err(|source_error| CodensityError::SourceIo {
            path: source.path.clone(),
            source: source_error,
        })?;
        let file_len = u64::try_from(data.len())
            .map_err(|_| CodensityError::CounterOverflow(root.to_path_buf()))?;
        totals.file_sizes.push(file_len);
        for &byte in &data {
            totals.byte_counts[usize::from(byte)] = totals.byte_counts[usize::from(byte)]
                .checked_add(1)
                .ok_or_else(|| CodensityError::CounterOverflow(root.to_path_buf()))?;
        }

        let mut noise_intervals = Vec::new();
        let high_entropy = high_entropy_intervals(&data);
        totals.high_entropy_bytes = checked_add(
            totals.high_entropy_bytes,
            interval_bytes(&high_entropy),
            root,
        )?;
        noise_intervals.extend(high_entropy);

        let random_tokens = random_token_intervals(&data);
        totals.random_token_bytes = checked_add(
            totals.random_token_bytes,
            interval_bytes(&random_tokens),
            root,
        )?;
        noise_intervals.extend(random_tokens);

        if is_minified(&data) {
            totals.minified_file_bytes = checked_add(totals.minified_file_bytes, file_len, root)?;
            noise_intervals.push((0, data.len()));
        }
        if has_generated_marker(&data) {
            totals.generated_marker_bytes =
                checked_add(totals.generated_marker_bytes, file_len, root)?;
            noise_intervals.push((0, data.len()));
        }
        totals.flagged_bytes = checked_add(
            totals.flagged_bytes,
            merged_interval_bytes(&mut noise_intervals),
            root,
        )?;

        let mut duplicate_intervals = duplicate_intervals(&data, &mut seen_fingerprints);
        totals.duplicate_bytes = checked_add(
            totals.duplicate_bytes,
            merged_interval_bytes(&mut duplicate_intervals),
            root,
        )?;
    }
    Ok(totals)
}

fn entropy_profile(signals: &SignalTotals, original_bytes: u64) -> EntropyProfile {
    let mut bits_per_byte = 0.0;
    for &count in &signals.byte_counts {
        if count == 0 {
            continue;
        }
        let probability = count as f64 / original_bytes as f64;
        bits_per_byte -= probability * probability.log2();
    }
    EntropyProfile {
        bits_per_byte,
        high_entropy_bytes: signals.high_entropy_bytes,
        high_entropy_ratio: ratio(signals.high_entropy_bytes, original_bytes),
    }
}

fn high_entropy_intervals(data: &[u8]) -> Vec<(usize, usize)> {
    if data.len() < ENTROPY_MIN_WINDOW {
        return Vec::new();
    }
    let mut intervals = Vec::new();
    for start in (0..data.len()).step_by(ENTROPY_WINDOW) {
        let end = (start + ENTROPY_WINDOW).min(data.len());
        let window = &data[start..end];
        if window.len() >= ENTROPY_MIN_WINDOW && shannon_entropy(window) >= HIGH_ENTROPY_THRESHOLD {
            intervals.push((start, end));
        }
    }
    intervals
}

fn shannon_entropy(data: &[u8]) -> f64 {
    let mut counts = [0_u64; 256];
    for &byte in data {
        counts[usize::from(byte)] += 1;
    }
    let length = data.len() as f64;
    counts
        .into_iter()
        .filter(|&count| count != 0)
        .map(|count| {
            let probability = count as f64 / length;
            -probability * probability.log2()
        })
        .sum()
}

fn random_token_intervals(data: &[u8]) -> Vec<(usize, usize)> {
    let mut intervals = Vec::new();
    let mut start = None;
    for (index, &byte) in data.iter().enumerate() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'_' | b'-' | b'=') {
            start.get_or_insert(index);
        } else if let Some(token_start) = start.take() {
            maybe_random_token(data, token_start, index, &mut intervals);
        }
    }
    if let Some(token_start) = start {
        maybe_random_token(data, token_start, data.len(), &mut intervals);
    }
    intervals
}

fn maybe_random_token(data: &[u8], start: usize, end: usize, intervals: &mut Vec<(usize, usize)>) {
    if end - start < RANDOM_TOKEN_MIN_LENGTH {
        return;
    }
    let token = &data[start..end];
    let has_lower = token.iter().any(u8::is_ascii_lowercase);
    let has_upper = token.iter().any(u8::is_ascii_uppercase);
    let has_digit = token.iter().any(u8::is_ascii_digit);
    let classes = u8::from(has_lower) + u8::from(has_upper) + u8::from(has_digit);
    if classes >= 2 && shannon_entropy(token) >= 3.5 {
        intervals.push((start, end));
    }
}

fn is_minified(data: &[u8]) -> bool {
    if data.len() < 1024 {
        return false;
    }
    let mut max_line = 0_usize;
    let mut current_line = 0_usize;
    let mut line_count = 1_usize;
    for &byte in data {
        if byte == b'\n' {
            max_line = max_line.max(current_line);
            current_line = 0;
            line_count += 1;
        } else {
            current_line += 1;
        }
    }
    max_line = max_line.max(current_line);
    max_line >= 1000 && data.len() / line_count >= 300
}

fn has_generated_marker(data: &[u8]) -> bool {
    let prefix = &data[..data.len().min(8192)];
    let lowercase = String::from_utf8_lossy(prefix).to_ascii_lowercase();
    [
        "do not edit",
        "@generated",
        "code generated",
        "automatically generated",
        "auto-generated",
    ]
    .iter()
    .any(|marker| lowercase.contains(marker))
}

fn duplicate_intervals(data: &[u8], seen: &mut HashSet<(u64, u64)>) -> Vec<(usize, usize)> {
    if data.is_empty() {
        return Vec::new();
    }
    if data.len() < FINGERPRINT_WINDOW {
        let key = fingerprint(data);
        return if seen.insert(key) {
            Vec::new()
        } else {
            vec![(0, data.len())]
        };
    }

    let base_a = 257_u64;
    let base_b = 263_u64;
    let power_a = base_a.wrapping_pow((FINGERPRINT_WINDOW - 1) as u32);
    let power_b = base_b.wrapping_pow((FINGERPRINT_WINDOW - 1) as u32);
    let mut hash_a = 0_u64;
    let mut hash_b = 0_u64;
    for &byte in &data[..FINGERPRINT_WINDOW] {
        let value = u64::from(byte) + 1;
        hash_a = hash_a.wrapping_mul(base_a).wrapping_add(value);
        hash_b = hash_b.wrapping_mul(base_b).wrapping_add(value);
    }

    let mut intervals = Vec::new();
    for start in 0..=data.len() - FINGERPRINT_WINDOW {
        if hash_a & FINGERPRINT_SAMPLE_MASK == 0 && !seen.insert((hash_a, hash_b)) {
            intervals.push((start, start + FINGERPRINT_WINDOW));
        }
        if start + FINGERPRINT_WINDOW == data.len() {
            break;
        }
        let old = u64::from(data[start]) + 1;
        let new = u64::from(data[start + FINGERPRINT_WINDOW]) + 1;
        hash_a = hash_a
            .wrapping_sub(old.wrapping_mul(power_a))
            .wrapping_mul(base_a)
            .wrapping_add(new);
        hash_b = hash_b
            .wrapping_sub(old.wrapping_mul(power_b))
            .wrapping_mul(base_b)
            .wrapping_add(new);
    }
    intervals
}

fn fingerprint(data: &[u8]) -> (u64, u64) {
    let mut left = 0xcbf2_9ce4_8422_2325_u64;
    let mut right = 0x8422_2325_cbf2_9ce4_u64;
    for &byte in data {
        left ^= u64::from(byte);
        left = left.wrapping_mul(0x100_0000_01b3);
        right ^= u64::from(byte).wrapping_add(1);
        right = right.wrapping_mul(0x100_0000_01d5);
    }
    (left, right)
}

fn merged_interval_bytes(intervals: &mut [(usize, usize)]) -> u64 {
    if intervals.is_empty() {
        return 0;
    }
    intervals.sort_unstable();
    let mut total = 0_u64;
    let (mut current_start, mut current_end) = intervals[0];
    for &(start, end) in &intervals[1..] {
        if start <= current_end {
            current_end = current_end.max(end);
        } else {
            total += (current_end - current_start) as u64;
            current_start = start;
            current_end = end;
        }
    }
    total + (current_end - current_start) as u64
}

fn interval_bytes(intervals: &[(usize, usize)]) -> u64 {
    intervals
        .iter()
        .map(|(start, end)| (end - start) as u64)
        .sum()
}

fn structure_profile(file_sizes: &[u64], original_bytes: u64) -> StructureProfile {
    let mut ascending = file_sizes.to_vec();
    ascending.sort_unstable();
    let median_file_bytes = ascending[(ascending.len() - 1) / 2];
    let p95_index = (ascending.len() * 95).div_ceil(100).saturating_sub(1);
    let p95_file_bytes = ascending[p95_index];
    let largest_file_share = ratio(*ascending.last().unwrap_or(&0), original_bytes);
    let top_10_file_share = ratio(
        ascending.iter().rev().take(10).copied().sum(),
        original_bytes,
    );
    let weighted_sum: f64 = ascending
        .iter()
        .enumerate()
        .map(|(index, &size)| (index + 1) as f64 * size as f64)
        .sum();
    let count = ascending.len() as f64;
    let gini = (2.0 * weighted_sum) / (count * original_bytes as f64) - (count + 1.0) / count;

    let threshold = original_bytes as f64 * 0.8;
    let mut cumulative = 0_u64;
    let mut files_for_80 = 0_usize;
    for size in ascending.iter().rev() {
        cumulative += size;
        files_for_80 += 1;
        if cumulative as f64 >= threshold {
            break;
        }
    }

    StructureProfile {
        median_file_bytes,
        p95_file_bytes,
        largest_file_share,
        top_10_file_share,
        gini: gini.max(0.0),
        long_tail: ascending.len() >= 5 && files_for_80 * 2 >= ascending.len(),
    }
}

fn language_baselines(languages: &[LanguageResult]) -> Vec<LanguageBaseline> {
    languages
        .iter()
        .filter_map(|language| {
            let project_ratio = language.metric.ratio?;
            let samples = baseline::samples(&language.language);
            if samples.is_empty() {
                return None;
            }
            let median_ratio = even_median(samples);
            let percentile = (samples.len() >= 3).then(|| {
                let less = samples
                    .iter()
                    .filter(|&&sample| sample < project_ratio)
                    .count() as f64;
                let equal = samples
                    .iter()
                    .filter(|&&sample| sample == project_ratio)
                    .count() as f64;
                (less + equal * 0.5) * 100.0 / samples.len() as f64
            });
            Some(LanguageBaseline {
                language: language.language.clone(),
                source_bytes: language.metric.original_bytes,
                project_ratio,
                sample_count: samples.len() as u64,
                median_ratio,
                percentile,
            })
        })
        .collect()
}

fn score_profile(
    compression: &CompressionProfile,
    entropy: &EntropyProfile,
    duplication: &DuplicationProfile,
    noise: &NoiseProfile,
    structure: &StructureProfile,
    baselines: &[LanguageBaseline],
    original_bytes: u64,
) -> ScoreProfile {
    let signal = clamp_score((1.0 - noise.noise_ratio) * 100.0);
    let baseline_bytes: u64 = baselines.iter().map(|baseline| baseline.source_bytes).sum();
    let normalized_compression = if baseline_bytes == 0 {
        clamp_score((compression.consensus_ratio - 0.05) / 0.30 * 100.0)
    } else {
        baselines
            .iter()
            .map(|baseline| {
                let relative = baseline.project_ratio / baseline.median_ratio;
                let component = clamp_score((relative - 0.5) * 100.0);
                component * baseline.source_bytes as f64 / baseline_bytes as f64
            })
            .sum()
    };
    // Incompressible flagged data must not inflate the compression component.
    let compression_component = normalized_compression * signal / 100.0;
    let entropy_component = if entropy.bits_per_byte <= 3.0 {
        clamp_score(entropy.bits_per_byte / 3.0 * 70.0)
    } else if entropy.bits_per_byte <= 6.5 {
        70.0 + (entropy.bits_per_byte - 3.0) / 3.5 * 30.0
    } else {
        clamp_score((8.0 - entropy.bits_per_byte) / 1.5 * 100.0)
    };
    let uniqueness = clamp_score((1.0 - duplication.duplicate_ratio) * 100.0);
    let distribution = if structure.top_10_file_share >= 0.999 && original_bytes < 64 * 1024 {
        50.0
    } else {
        clamp_score((1.0 - 0.55 * structure.largest_file_share - 0.45 * structure.gini) * 100.0)
    };
    let weights = ScoreWeights {
        compression: 0.30,
        entropy: 0.15,
        uniqueness: 0.25,
        signal: 0.20,
        distribution: 0.10,
    };
    let information_density = compression_component * weights.compression
        + entropy_component * weights.entropy
        + uniqueness * weights.uniqueness
        + signal * weights.signal
        + distribution * weights.distribution;

    let weighted_samples = if baseline_bytes == 0 {
        0.0
    } else {
        baselines
            .iter()
            .map(|baseline| {
                baseline.sample_count as f64 * baseline.source_bytes as f64 / baseline_bytes as f64
            })
            .sum()
    };
    let confidence = if original_bytes >= 256 * 1024 && weighted_samples >= 5.0 {
        "high"
    } else if original_bytes >= 64 * 1024 && weighted_samples >= 2.0 {
        "medium"
    } else {
        "low"
    };
    let template_repetition_risk =
        if duplication.duplicate_ratio >= 0.30 || compression.consensus_ratio < 0.08 {
            "high"
        } else if duplication.duplicate_ratio >= 0.15 || compression.consensus_ratio < 0.14 {
            "medium"
        } else {
            "low"
        };

    ScoreProfile {
        information_density,
        confidence: confidence.to_owned(),
        compression: compression_component,
        entropy: entropy_component,
        uniqueness,
        signal,
        distribution,
        weights,
        template_repetition_risk: template_repetition_risk.to_owned(),
    }
}

fn checked_add(left: u64, right: u64, root: &Path) -> Result<u64> {
    left.checked_add(right)
        .ok_or_else(|| CodensityError::CounterOverflow(root.to_path_buf()))
}

fn ratio(numerator: u64, denominator: u64) -> f64 {
    numerator as f64 / denominator as f64
}

fn even_median(values: &[f64]) -> f64 {
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
}

fn clamp_score(value: f64) -> f64 {
    value.clamp(0.0, 100.0)
}
