use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use ignore::WalkBuilder;
use sha2::{Digest, Sha256};

use crate::error::{CodensityError, Result};
use crate::language::{LANGUAGES, language_for_path};
use crate::model::{
    ANALYSIS_SCHEMA_VERSION, AnalysisResult, LEDGER_SCHEMA_VERSION, LanguageResult, MetricResult,
    PROTOCOL_ID, RELATION_PROTOCOL_ID, RELATION_SCHEMA_VERSION, RelationFileResult, RelationResult,
};
use crate::profile::build_profile;
use crate::{CODENSITY_VERSION, zstd_version};

const BUFFER_SIZE: usize = 64 * 1024;
const ZSTD_LEVEL: i32 = 19;
const EXCLUDED_DIRECTORIES: &[&str] = &[
    ".git",
    ".codensity",
    "target",
    "node_modules",
    "vendor",
    "dist",
    "build",
    ".next",
    ".cache",
];

#[derive(Clone, Debug)]
pub(crate) struct SourceFile {
    pub(crate) path: PathBuf,
    pub(crate) relative: String,
    pub(crate) language_index: usize,
}

#[derive(Default)]
pub(crate) struct CountingWriter {
    pub(crate) bytes: u64,
}

impl Write for CountingWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let length = u64::try_from(buffer.len()).map_err(std::io::Error::other)?;
        self.bytes = self
            .bytes
            .checked_add(length)
            .ok_or_else(|| std::io::Error::other("compressed byte counter overflow"))?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Produces a safe logical label from the final input path component.
///
/// Absolute parent directories are never included.
///
/// # Errors
///
/// Returns [`CodensityError::InvalidRelativePath`] when the final component is
/// not valid UTF-8.
pub fn safe_input_label(path: &Path) -> Result<String> {
    if path == Path::new(".") {
        return Ok(".".to_owned());
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .ok_or_else(|| CodensityError::InvalidRelativePath(path.to_path_buf()))
}

/// Analyzes an input with protocol `codensity-zstd19-concat-v1`.
///
/// The implementation retains only selected path metadata. File contents and
/// compressed output flow through bounded buffers.
///
/// # Errors
///
/// Returns a typed error for missing inputs, walk/read/compression failures,
/// unrepresentable relative paths, counter overflow, or a corpus without
/// recognized non-empty source bytes.
pub fn analyze_path(path: &Path, input_label: &str) -> Result<AnalysisResult> {
    analyze(path, input_label, true)
}

/// Analyzes only the frozen schema-v1 compression ledger.
///
/// Database generation uses this entry point so the published v0.1 benchmark
/// ledgers remain byte-for-byte reproducible.
pub fn analyze_ledger_path(path: &Path, input_label: &str) -> Result<AnalysisResult> {
    analyze(path, input_label, false)
}

/// Measures cross-stream regularity for exactly two included source files.
pub fn relate_paths(root: &Path, first: &Path, second: &Path) -> Result<RelationResult> {
    if !root.exists() {
        return Err(CodensityError::InputNotFound(root.to_path_buf()));
    }
    if !root.is_dir() {
        return Err(CodensityError::RelationRootNotDirectory(root.to_path_buf()));
    }
    let first_relative = relation_relative_path(first)?;
    let second_relative = relation_relative_path(second)?;
    if first_relative == second_relative {
        return Err(CodensityError::RelationDuplicateSource(first.to_path_buf()));
    }

    let (files, _) = scan(root)?;
    let mut selected = [
        select_relation_source(root, &files, first, &first_relative)?,
        select_relation_source(root, &files, second, &second_relative)?,
    ];
    selected.sort_by(|left, right| left.relative.as_bytes().cmp(right.relative.as_bytes()));

    let first_metric = stream_metric(root, std::slice::from_ref(&selected[0]), None)?;
    let second_metric = stream_metric(root, std::slice::from_ref(&selected[1]), None)?;
    let combined = stream_metric(root, &selected, None)?;
    let empty_frame_bytes = stream_metric(root, &[], None)?.compressed_bytes;
    let independent_sum = first_metric
        .compressed_bytes
        .checked_add(second_metric.compressed_bytes)
        .ok_or_else(|| CodensityError::CounterOverflow(root.to_path_buf()))?;
    let raw_cross_stream_gain_bytes =
        signed_difference(independent_sum, combined.compressed_bytes, root)?;
    let adjusted_cross_stream_gain_bytes = raw_cross_stream_gain_bytes
        .checked_sub(
            i64::try_from(empty_frame_bytes)
                .map_err(|_| CodensityError::CounterOverflow(root.to_path_buf()))?,
        )
        .ok_or_else(|| CodensityError::CounterOverflow(root.to_path_buf()))?;
    let independent_payload_bytes = independent_sum.saturating_sub(
        empty_frame_bytes
            .checked_mul(2)
            .ok_or_else(|| CodensityError::CounterOverflow(root.to_path_buf()))?,
    );

    Ok(RelationResult {
        schema_version: RELATION_SCHEMA_VERSION,
        codensity_version: CODENSITY_VERSION.to_owned(),
        zstd_version: zstd_version().to_owned(),
        protocol: RELATION_PROTOCOL_ID.to_owned(),
        first: relation_file_result(&selected[0], first_metric),
        second: relation_file_result(&selected[1], second_metric),
        combined,
        empty_frame_bytes,
        raw_cross_stream_gain_bytes,
        adjusted_cross_stream_gain_bytes,
        adjusted_cross_stream_gain_ratio: (independent_payload_bytes != 0).then_some(
            adjusted_cross_stream_gain_bytes as f64 / independent_payload_bytes as f64,
        ),
        interpretation: "Measures shared byte-level patterns after fixed frame overhead; it is not structural coupling, dependency direction, causality, or a quality score.".to_owned(),
    })
}

fn relation_relative_path(path: &Path) -> Result<String> {
    if path.is_absolute() {
        return Err(CodensityError::RelationPathOutsideRoot(path.to_path_buf()));
    }
    let mut normalized = String::new();
    for component in path.components() {
        let Component::Normal(value) = component else {
            return Err(CodensityError::RelationPathOutsideRoot(path.to_path_buf()));
        };
        let value = value
            .to_str()
            .ok_or_else(|| CodensityError::RelationPathOutsideRoot(path.to_path_buf()))?;
        if !normalized.is_empty() {
            normalized.push('/');
        }
        normalized.push_str(value);
    }
    if normalized.is_empty() {
        return Err(CodensityError::RelationPathOutsideRoot(path.to_path_buf()));
    }
    Ok(normalized)
}

fn select_relation_source(
    root: &Path,
    files: &[SourceFile],
    requested: &Path,
    relative: &str,
) -> Result<SourceFile> {
    files
        .iter()
        .find(|file| file.relative == relative)
        .cloned()
        .ok_or_else(|| CodensityError::RelationSourceUnavailable {
            root: root.to_path_buf(),
            path: requested.to_path_buf(),
        })
}

fn relation_file_result(file: &SourceFile, metric: MetricResult) -> RelationFileResult {
    RelationFileResult {
        path: file.relative.clone(),
        language: LANGUAGES[file.language_index].name.to_owned(),
        metric,
    }
}

fn signed_difference(left: u64, right: u64, root: &Path) -> Result<i64> {
    let left =
        i64::try_from(left).map_err(|_| CodensityError::CounterOverflow(root.to_path_buf()))?;
    let right =
        i64::try_from(right).map_err(|_| CodensityError::CounterOverflow(root.to_path_buf()))?;
    left.checked_sub(right)
        .ok_or_else(|| CodensityError::CounterOverflow(root.to_path_buf()))
}

fn analyze(path: &Path, input_label: &str, include_profile: bool) -> Result<AnalysisResult> {
    if !path.exists() {
        return Err(CodensityError::InputNotFound(path.to_path_buf()));
    }

    let (files, skipped_file_count) = scan(path)?;
    let overall = stream_metric(path, &files, None)?;
    if overall.original_bytes == 0 {
        return Err(CodensityError::NoSourceBytes(path.to_path_buf()));
    }

    let mut languages = Vec::new();
    for (language_index, language) in LANGUAGES.iter().enumerate() {
        let file_count = files
            .iter()
            .filter(|file| file.language_index == language_index)
            .count();
        if file_count == 0 {
            continue;
        }
        let metric = stream_metric(path, &files, Some(language_index))?;
        languages.push(LanguageResult {
            language: language.name.to_owned(),
            metric,
        });
    }

    let profile = if include_profile {
        Some(build_profile(
            path,
            &files,
            overall.original_bytes,
            overall.compressed_bytes,
            &languages,
        )?)
    } else {
        None
    };

    Ok(AnalysisResult {
        schema_version: if include_profile {
            ANALYSIS_SCHEMA_VERSION
        } else {
            LEDGER_SCHEMA_VERSION
        },
        codensity_version: CODENSITY_VERSION.to_owned(),
        zstd_version: zstd_version().to_owned(),
        protocol: PROTOCOL_ID.to_owned(),
        input_label: input_label.to_owned(),
        overall,
        languages,
        skipped_file_count,
        profile,
    })
}

fn scan(root: &Path) -> Result<(Vec<SourceFile>, u64)> {
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false)
        .ignore(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(false)
        .parents(false)
        .require_git(false)
        .follow_links(false)
        .filter_entry(|entry| {
            entry.depth() == 0
                || !entry
                    .file_type()
                    .is_some_and(|file_type| file_type.is_dir())
                || !entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| EXCLUDED_DIRECTORIES.contains(&name))
        });

    let input_is_file = root.is_file();
    let mut files = Vec::new();
    let mut skipped = 0_u64;
    for entry in builder.build() {
        let entry = entry.map_err(|source| CodensityError::Walk {
            root: root.to_path_buf(),
            source,
        })?;
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        if let Some(language_index) = language_for_path(entry.path()) {
            let relative = normalized_relative(root, entry.path(), input_is_file)?;
            files.push(SourceFile {
                path: entry.into_path(),
                relative,
                language_index,
            });
        } else {
            skipped = skipped
                .checked_add(1)
                .ok_or_else(|| CodensityError::CounterOverflow(root.to_path_buf()))?;
        }
    }
    files.sort_by(|left, right| left.relative.as_bytes().cmp(right.relative.as_bytes()));
    Ok((files, skipped))
}

fn normalized_relative(root: &Path, path: &Path, input_is_file: bool) -> Result<String> {
    let relative = if input_is_file {
        path.file_name()
            .map(Path::new)
            .ok_or_else(|| CodensityError::InvalidRelativePath(path.to_path_buf()))?
    } else {
        path.strip_prefix(root)
            .map_err(|_| CodensityError::InvalidRelativePath(path.to_path_buf()))?
    };

    let mut normalized = String::new();
    for component in relative.components() {
        let Component::Normal(value) = component else {
            return Err(CodensityError::InvalidRelativePath(path.to_path_buf()));
        };
        let value = value
            .to_str()
            .ok_or_else(|| CodensityError::InvalidRelativePath(path.to_path_buf()))?;
        if !normalized.is_empty() {
            normalized.push('/');
        }
        normalized.push_str(value);
    }
    if normalized.is_empty() {
        return Err(CodensityError::InvalidRelativePath(path.to_path_buf()));
    }
    Ok(normalized)
}

fn stream_metric(
    root: &Path,
    files: &[SourceFile],
    language_index: Option<usize>,
) -> Result<MetricResult> {
    let counter = CountingWriter::default();
    let mut encoder = zstd::stream::write::Encoder::new(counter, ZSTD_LEVEL)
        .map_err(CodensityError::Compression)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; BUFFER_SIZE];
    let mut file_count = 0_u64;
    let mut original_bytes = 0_u64;

    for source in files
        .iter()
        .filter(|file| language_index.is_none_or(|index| file.language_index == index))
    {
        file_count = file_count
            .checked_add(1)
            .ok_or_else(|| CodensityError::CounterOverflow(root.to_path_buf()))?;
        let mut input =
            File::open(&source.path).map_err(|source_error| CodensityError::SourceIo {
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
            let bytes = &buffer[..read];
            hasher.update(bytes);
            encoder
                .write_all(bytes)
                .map_err(CodensityError::Compression)?;
            original_bytes = original_bytes
                .checked_add(
                    u64::try_from(read)
                        .map_err(|_| CodensityError::CounterOverflow(root.to_path_buf()))?,
                )
                .ok_or_else(|| CodensityError::CounterOverflow(root.to_path_buf()))?;
        }
    }

    let compressed_bytes = encoder.finish().map_err(CodensityError::Compression)?.bytes;
    let ratio = (original_bytes != 0).then_some(compressed_bytes as f64 / original_bytes as f64);
    let savings = ratio.map(|value| 1.0 - value);

    Ok(MetricResult {
        file_count,
        original_bytes,
        compressed_bytes,
        ratio,
        savings,
        sha256: format!("{:x}", hasher.finalize()),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::relate_paths;
    use crate::{CodensityError, RELATION_PROTOCOL_ID};

    struct Fixture {
        path: std::path::PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time is after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("codensity-relation-{nonce}"));
            fs::create_dir_all(&path).expect("create fixture root");
            Self { path }
        }

        fn write(&self, relative: &str, contents: &[u8]) {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create fixture parent");
            }
            fs::write(path, contents).expect("write fixture file");
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.path).expect("remove fixture root");
        }
    }

    #[test]
    fn relation_is_order_independent_and_reports_shared_patterns() {
        let fixture = Fixture::new();
        let repeated = b"fn repeated_pattern() { let value = 42; }\n".repeat(512);
        fixture.write("z.rs", &repeated);
        fixture.write("a.rs", &repeated);

        let forward = relate_paths(&fixture.path, Path::new("z.rs"), Path::new("a.rs"))
            .expect("measure relation");
        let reverse = relate_paths(&fixture.path, Path::new("a.rs"), Path::new("z.rs"))
            .expect("measure reversed relation");

        assert_eq!(forward, reverse);
        assert_eq!(forward.protocol, RELATION_PROTOCOL_ID);
        assert_eq!(forward.first.path, "a.rs");
        assert_eq!(forward.second.path, "z.rs");
        assert!(forward.adjusted_cross_stream_gain_bytes > 0);
        assert!(forward.adjusted_cross_stream_gain_ratio.is_some());
        let json = serde_json::to_value(&forward).expect("serialize relation result");
        assert_eq!(json["protocol"], RELATION_PROTOCOL_ID);
        assert!(json["first"]["compressed_bytes"].is_number());
    }

    #[test]
    fn relation_rejects_outside_duplicate_and_unavailable_sources() {
        let fixture = Fixture::new();
        fixture.write("kept.rs", b"fn kept() {}\n");
        fixture.write("ignored.rs", b"fn ignored() {}\n");
        fixture.write("data.txt", b"not source\n");
        fixture.write(".gitignore", b"ignored.rs\n");

        assert!(matches!(
            relate_paths(
                &fixture.path,
                Path::new("../outside.rs"),
                Path::new("kept.rs")
            ),
            Err(CodensityError::RelationPathOutsideRoot(_))
        ));
        assert!(matches!(
            relate_paths(&fixture.path, Path::new("kept.rs"), Path::new("kept.rs")),
            Err(CodensityError::RelationDuplicateSource(_))
        ));
        assert!(matches!(
            relate_paths(&fixture.path, Path::new("ignored.rs"), Path::new("kept.rs")),
            Err(CodensityError::RelationSourceUnavailable { .. })
        ));
        assert!(matches!(
            relate_paths(&fixture.path, Path::new("data.txt"), Path::new("kept.rs")),
            Err(CodensityError::RelationSourceUnavailable { .. })
        ));
    }
}
