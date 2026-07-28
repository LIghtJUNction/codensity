use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use ignore::WalkBuilder;
use sha2::{Digest, Sha256};

use crate::error::{CodensityError, Result};
use crate::language::{LANGUAGES, language_for_path};
use crate::model::{
    ANALYSIS_SCHEMA_VERSION, AnalysisResult, LanguageResult, MetricResult, PROTOCOL_ID,
};
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

#[derive(Debug)]
struct SourceFile {
    path: PathBuf,
    relative: String,
    language_index: usize,
}

#[derive(Default)]
struct CountingWriter {
    bytes: u64,
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

    Ok(AnalysisResult {
        schema_version: ANALYSIS_SCHEMA_VERSION,
        codensity_version: CODENSITY_VERSION.to_owned(),
        zstd_version: zstd_version().to_owned(),
        protocol: PROTOCOL_ID.to_owned(),
        input_label: input_label.to_owned(),
        overall,
        languages,
        skipped_file_count,
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
