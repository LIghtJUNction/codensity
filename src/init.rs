use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::database::{ensure_managed_directory, write_atomic_json};
use crate::{AnalysisResult, CodensityError, Result, analyze_path, safe_input_label};

const SNAPSHOT_FILENAME: &str = "analysis.json";

/// Initializes a project-local Codensity state directory and records a snapshot.
///
/// The managed directory is excluded by the analysis protocol, so refreshing the
/// snapshot cannot alter the source stream it records.
pub fn initialize_project(path: &Path, force: bool) -> Result<AnalysisResult> {
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
    if !force && requires_force(&canonical_root) {
        return Err(CodensityError::InitializationRequiresForce(canonical_root));
    }

    let label = safe_input_label(path)?;
    let analysis = analyze_path(&canonical_root, &label)?;
    ensure_managed_directory(&canonical_root)?;
    write_atomic_json(
        &canonical_root.join(".codensity").join(SNAPSHOT_FILENAME),
        &analysis,
    )?;
    Ok(analysis)
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
