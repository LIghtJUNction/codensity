use std::env;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::analyzer::scan;
use crate::database::{ensure_managed_directory, write_atomic_json};
use crate::{
    ANALYSIS_SCHEMA_VERSION, AnalysisResult, CODENSITY_VERSION, CodensityError, PROTOCOL_ID,
    Result, analyze_path, safe_input_label, zstd_version,
};

const SNAPSHOT_FILENAME: &str = "analysis.json";
const CACHE_FILENAME: &str = "cache-v1.json";
const MANAGED_IGNORE_FILENAME: &str = ".gitignore";
const MANAGED_IGNORE_CONTENTS: &[u8] = b"*\n!.gitignore\n";
const CACHE_SCHEMA_VERSION: u32 = 1;
const CACHE_DOMAIN: &[u8] = b"codensity-recognized-source-manifest-v1\0";

/// Whether an initialization reused an exact local snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheStatus {
    /// A fully validated snapshot was reused.
    Hit,
    /// The snapshot was absent or stale and was rebuilt.
    Miss,
}

impl CacheStatus {
    /// Stable lowercase label for human CLI output.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hit => "hit",
            Self::Miss => "miss",
        }
    }
}

/// Result of initializing a project-local snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct InitializationResult {
    /// Reused or freshly computed analysis.
    pub analysis: AnalysisResult,
    /// Cache outcome for this invocation.
    pub cache_status: CacheStatus,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheState {
    schema_version: u32,
    source_manifest_sha256: String,
    snapshot_sha256: String,
    analysis_schema_version: u32,
    codensity_version: String,
    zstd_version: String,
    protocol: String,
    input_label: String,
}

/// Initializes a project-local Codensity state directory and records a snapshot.
///
/// The managed directory is excluded by the analysis protocol, so refreshing the
/// snapshot cannot alter the source stream it records.
pub fn initialize_project(path: &Path, force: bool) -> Result<AnalysisResult> {
    Ok(initialize_project_with_status(path, force)?.analysis)
}

/// Initializes a project and reports whether its validated local snapshot was reused.
pub fn initialize_project_with_status(path: &Path, force: bool) -> Result<InitializationResult> {
    let canonical_root = canonical_project_root(path)?;
    if !force && requires_force(&canonical_root) {
        return Err(CodensityError::InitializationRequiresForce(canonical_root));
    }

    let label = safe_input_label(path)?;
    let managed = managed_directory(&canonical_root);
    if managed.exists() {
        ensure_managed_directory(&canonical_root)?;
    }
    let manifest_sha256 = recognized_source_manifest(&canonical_root)?;
    if let Some(analysis) = read_cached_analysis(&canonical_root, &label, &manifest_sha256)? {
        return Ok(InitializationResult {
            analysis,
            cache_status: CacheStatus::Hit,
        });
    }

    let analysis = analyze_path(&canonical_root, &label)?;
    ensure_managed_directory(&canonical_root)?;
    let snapshot_path = managed.join(SNAPSHOT_FILENAME);
    write_atomic_json(&snapshot_path, &analysis)?;
    let snapshot = fs::read(&snapshot_path).map_err(|source| CodensityError::CodensityStateIo {
        path: snapshot_path,
        source,
    })?;
    let cache = CacheState {
        schema_version: CACHE_SCHEMA_VERSION,
        source_manifest_sha256: manifest_sha256,
        snapshot_sha256: sha256_hex(&snapshot),
        analysis_schema_version: analysis.schema_version,
        codensity_version: analysis.codensity_version.clone(),
        zstd_version: analysis.zstd_version.clone(),
        protocol: analysis.protocol.clone(),
        input_label: analysis.input_label.clone(),
    };
    write_atomic_json(&managed.join(CACHE_FILENAME), &cache)?;
    Ok(InitializationResult {
        analysis,
        cache_status: CacheStatus::Miss,
    })
}

/// Safely removes only the complete Codensity state owned by `path`.
pub fn clean_project(path: &Path, force: bool) -> Result<()> {
    let canonical_root = canonical_project_root(path)?;
    if !force && requires_force(&canonical_root) {
        return Err(CodensityError::CleanRequiresForce(canonical_root));
    }
    let managed = managed_directory(&canonical_root);
    preflight_clean(&managed)?;

    for filename in [MANAGED_IGNORE_FILENAME, SNAPSHOT_FILENAME, CACHE_FILENAME] {
        let target = managed.join(filename);
        if target.exists() {
            fs::remove_file(&target).map_err(|source| CodensityError::CodensityStateIo {
                path: target,
                source,
            })?;
        }
    }
    fs::remove_dir(&managed).map_err(|source| CodensityError::CodensityStateIo {
        path: managed,
        source,
    })
}

fn canonical_project_root(path: &Path) -> Result<PathBuf> {
    let canonical_root =
        fs::canonicalize(path).map_err(|source| CodensityError::InitializationCanonicalize {
            path: path.to_path_buf(),
            source,
        })?;
    if !canonical_root.is_dir() {
        return Err(CodensityError::InitializationPathNotDirectory(
            canonical_root,
        ));
    }
    Ok(canonical_root)
}

fn managed_directory(root: &Path) -> PathBuf {
    root.join(".codensity")
}

fn read_cached_analysis(
    root: &Path,
    label: &str,
    manifest_sha256: &str,
) -> Result<Option<AnalysisResult>> {
    let managed = managed_directory(root);
    let Some(snapshot) = read_regular_state_file(&managed.join(SNAPSHOT_FILENAME))? else {
        return Ok(None);
    };
    let Some(cache_bytes) = read_regular_state_file(&managed.join(CACHE_FILENAME))? else {
        return Ok(None);
    };
    let Ok(cache) = serde_json::from_slice::<CacheState>(&cache_bytes) else {
        return Ok(None);
    };
    let Ok(analysis) = serde_json::from_slice::<AnalysisResult>(&snapshot) else {
        return Ok(None);
    };
    let valid = cache.schema_version == CACHE_SCHEMA_VERSION
        && cache.source_manifest_sha256 == manifest_sha256
        && cache.snapshot_sha256 == sha256_hex(&snapshot)
        && cache.analysis_schema_version == ANALYSIS_SCHEMA_VERSION
        && cache.codensity_version == CODENSITY_VERSION
        && cache.zstd_version == zstd_version()
        && cache.protocol == PROTOCOL_ID
        && cache.input_label == label
        && analysis.schema_version == cache.analysis_schema_version
        && analysis.codensity_version == cache.codensity_version
        && analysis.zstd_version == cache.zstd_version
        && analysis.protocol == cache.protocol
        && analysis.input_label == cache.input_label;
    Ok(valid.then_some(analysis))
}

fn read_regular_state_file(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(CodensityError::InvalidCodensityState(path.to_path_buf()))
        }
        Ok(_) => fs::read(path)
            .map(Some)
            .map_err(|source| CodensityError::CodensityStateIo {
                path: path.to_path_buf(),
                source,
            }),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(CodensityError::CodensityStateIo {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn recognized_source_manifest(root: &Path) -> Result<String> {
    let (sources, _) = scan(root)?;
    let mut manifest = Sha256::new();
    manifest.update(CACHE_DOMAIN);
    let mut buffer = [0_u8; 64 * 1024];
    for source in sources {
        let relative = source.relative.as_bytes();
        let relative_length = u64::try_from(relative.len())
            .map_err(|_| CodensityError::CounterOverflow(root.to_path_buf()))?;
        manifest.update(relative_length.to_be_bytes());
        manifest.update(relative);
        let mut file =
            File::open(&source.path).map_err(|source_error| CodensityError::SourceIo {
                path: source.path.clone(),
                source: source_error,
            })?;
        let mut content = Sha256::new();
        loop {
            let read = file
                .read(&mut buffer)
                .map_err(|source_error| CodensityError::SourceIo {
                    path: source.path.clone(),
                    source: source_error,
                })?;
            if read == 0 {
                break;
            }
            content.update(&buffer[..read]);
        }
        manifest.update(content.finalize());
    }
    Ok(format!("{:x}", manifest.finalize()))
}

fn preflight_clean(managed: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(managed).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            CodensityError::CleanStateNotFound(managed.to_path_buf())
        } else {
            CodensityError::CodensityStateIo {
                path: managed.to_path_buf(),
                source,
            }
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CodensityError::InvalidManagedDirectory(
            managed.to_path_buf(),
        ));
    }
    let mut managed_ignore_found = false;
    for entry in fs::read_dir(managed).map_err(|source| CodensityError::CodensityStateIo {
        path: managed.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| CodensityError::CodensityStateIo {
            path: managed.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let name = entry.file_name();
        let allowed = [MANAGED_IGNORE_FILENAME, SNAPSHOT_FILENAME, CACHE_FILENAME]
            .iter()
            .any(|expected| name == *expected);
        if !allowed {
            return Err(CodensityError::UnknownCodensityStateContent(path));
        }
        let metadata =
            fs::symlink_metadata(&path).map_err(|source| CodensityError::CodensityStateIo {
                path: path.clone(),
                source,
            })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(CodensityError::InvalidCodensityState(path));
        }
        if name == MANAGED_IGNORE_FILENAME {
            let contents = fs::read(&path).map_err(|source| CodensityError::CodensityStateIo {
                path: path.clone(),
                source,
            })?;
            if contents != MANAGED_IGNORE_CONTENTS {
                return Err(CodensityError::ManagedIgnoreContentsMismatch(path));
            }
            managed_ignore_found = true;
        }
    }
    if !managed_ignore_found {
        return Err(CodensityError::UnknownCodensityStateContent(
            managed.join(MANAGED_IGNORE_FILENAME),
        ));
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn requires_force(path: &Path) -> bool {
    path.parent().is_none() || home_directory().is_some_and(|home| home == path)
}

fn home_directory() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .and_then(|path| fs::canonicalize(path).ok())
}
