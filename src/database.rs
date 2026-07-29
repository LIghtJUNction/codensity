use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::analyzer::analyze_path;
use crate::error::{CodensityError, Result};
use crate::model::{
    DATABASE_SCHEMA_VERSION, Database, DatabaseProject, Manifest, ManifestProject, PROTOCOL_ID,
};
use crate::{CODENSITY_VERSION, zstd_version};
use serde::Serialize;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
const MANAGED_IGNORE_CONTENTS: &[u8] = b"*\n!.gitignore\n";

struct PreparedProject {
    project: ManifestProject,
    canonical_root: PathBuf,
}

struct TempOutput {
    path: PathBuf,
    committed: bool,
}

impl Drop for TempOutput {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Loads and validates a schema-v1 database manifest.
///
/// # Errors
///
/// Returns a typed error for I/O, JSON, schema, field, duplicate identity, and
/// local project-path validation failures.
pub fn load_manifest(path: &Path) -> Result<Manifest> {
    let input = fs::read(path).map_err(|source| CodensityError::ManifestIo {
        path: path.to_path_buf(),
        source,
    })?;
    let manifest: Manifest =
        serde_json::from_slice(&input).map_err(|source| CodensityError::ManifestJson {
            path: path.to_path_buf(),
            source,
        })?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

/// Builds and atomically writes a deterministic schema-v1 database.
///
/// Every project is processed through the same [`analyze_path`] entry point as
/// the CLI. The destination is untouched until a complete sibling temporary
/// file has been flushed and synchronized.
///
/// # Errors
///
/// Returns a typed manifest, analysis, serialization, I/O, or rename error.
pub fn build_database(manifest_path: &Path, output_path: &Path) -> Result<Database> {
    let mut manifest = load_manifest(manifest_path)?;
    manifest
        .projects
        .sort_by(|left, right| (&left.name, &left.version).cmp(&(&right.name, &right.version)));
    let prepared_projects = prepare_projects(manifest.projects)?;
    let resolved_output = prepare_output_path(output_path, &prepared_projects)?;

    let mut projects = Vec::with_capacity(prepared_projects.len());
    for prepared in prepared_projects {
        let analysis = analyze_path(&prepared.canonical_root, &prepared.project.name)?;
        projects.push(database_project(prepared.project, analysis));
    }

    let database = Database {
        schema_version: DATABASE_SCHEMA_VERSION,
        codensity_version: CODENSITY_VERSION.to_owned(),
        zstd_version: zstd_version().to_owned(),
        protocol: PROTOCOL_ID.to_owned(),
        projects,
    };
    write_atomic_json(&resolved_output, &database)?;
    Ok(database)
}

fn prepare_projects(projects: Vec<ManifestProject>) -> Result<Vec<PreparedProject>> {
    projects
        .into_iter()
        .map(|project| {
            let canonical_root = fs::canonicalize(&project.path).map_err(|source| {
                CodensityError::ProjectCanonicalize {
                    path: project.path.clone(),
                    source,
                }
            })?;
            Ok(PreparedProject {
                project,
                canonical_root,
            })
        })
        .collect()
}

fn prepare_output_path(path: &Path, projects: &[PreparedProject]) -> Result<PathBuf> {
    validate_output_filename(path)?;
    let filename = path
        .file_name()
        .ok_or_else(|| CodensityError::InvalidOutputPath(path.to_path_buf()))?;
    let parent = usable_parent(path);
    let resolved = match fs::canonicalize(parent) {
        Ok(canonical_parent) => canonical_parent.join(filename),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            prepare_missing_managed_parent(parent, filename, projects, source)?
        }
        Err(source) => {
            return Err(CodensityError::OutputParentCanonicalize {
                path: parent.to_path_buf(),
                source,
            });
        }
    };
    let managed_projects = validate_resolved_output(&resolved, projects)?;
    for index in managed_projects {
        ensure_managed_directory(&projects[index].canonical_root)?;
    }
    Ok(resolved)
}

fn prepare_missing_managed_parent(
    parent: &Path,
    filename: &std::ffi::OsStr,
    projects: &[PreparedProject],
    original_error: std::io::Error,
) -> Result<PathBuf> {
    if parent.file_name() != Some(std::ffi::OsStr::new(".codensity")) {
        return Err(CodensityError::OutputParentCanonicalize {
            path: parent.to_path_buf(),
            source: original_error,
        });
    }
    let owner = usable_parent(parent);
    let canonical_owner =
        fs::canonicalize(owner).map_err(|source| CodensityError::OutputParentCanonicalize {
            path: owner.to_path_buf(),
            source,
        })?;
    let owner_projects: Vec<_> = projects
        .iter()
        .enumerate()
        .filter_map(|(index, project)| (project.canonical_root == canonical_owner).then_some(index))
        .collect();
    if owner_projects.is_empty() {
        return Err(CodensityError::OutputParentCanonicalize {
            path: parent.to_path_buf(),
            source: original_error,
        });
    }

    let candidate = canonical_owner.join(".codensity").join(filename);
    validate_resolved_output(&candidate, projects)?;
    for index in owner_projects {
        ensure_managed_directory(&projects[index].canonical_root)?;
    }
    let canonical_parent =
        fs::canonicalize(parent).map_err(|source| CodensityError::OutputParentCanonicalize {
            path: parent.to_path_buf(),
            source,
        })?;
    let resolved = canonical_parent.join(filename);
    Ok(resolved)
}

fn validate_resolved_output(
    resolved: &Path,
    projects: &[PreparedProject],
) -> Result<BTreeSet<usize>> {
    let mut managed_projects = BTreeSet::new();
    for (index, project) in projects.iter().enumerate() {
        if let Ok(relative) = resolved.strip_prefix(&project.canonical_root) {
            require_managed_output(relative, resolved, &project.project.name)?;
            managed_projects.insert(index);
        }
    }
    Ok(managed_projects)
}

fn validate_output_filename(path: &Path) -> Result<()> {
    if path
        .file_name()
        .and_then(|filename| filename.to_str())
        .is_none()
    {
        Err(CodensityError::InvalidOutputPath(path.to_path_buf()))
    } else {
        Ok(())
    }
}

fn require_managed_output(relative: &Path, output: &Path, project: &str) -> Result<()> {
    let mut components = relative.components();
    if !matches!(
        components.next(),
        Some(Component::Normal(component)) if component == ".codensity"
    ) {
        return Err(CodensityError::OutputInsideProject {
            output: output.to_path_buf(),
            project: project.to_owned(),
        });
    }
    match (components.next(), components.next()) {
        (None, _) => Err(CodensityError::ManagedDirectoryOutputReserved(
            output.to_path_buf(),
        )),
        (Some(Component::Normal(component)), None) if component == ".gitignore" => Err(
            CodensityError::ManagedIgnoreOutputReserved(output.to_path_buf()),
        ),
        _ => Ok(()),
    }
}

pub(crate) fn ensure_managed_directory(project_root: &Path) -> Result<()> {
    let managed = project_root.join(".codensity");
    match fs::symlink_metadata(&managed) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => return Err(CodensityError::InvalidManagedDirectory(managed)),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            match fs::create_dir(&managed) {
                Ok(()) => {}
                Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
                    let metadata = fs::symlink_metadata(&managed).map_err(|source| {
                        CodensityError::ManagedPathIo {
                            path: managed.clone(),
                            source,
                        }
                    })?;
                    if !metadata.is_dir() || metadata.file_type().is_symlink() {
                        return Err(CodensityError::InvalidManagedDirectory(managed));
                    }
                }
                Err(source) => {
                    return Err(CodensityError::ManagedPathIo {
                        path: managed,
                        source,
                    });
                }
            }
        }
        Err(source) => {
            return Err(CodensityError::ManagedPathIo {
                path: managed,
                source,
            });
        }
    }
    ensure_managed_ignore(&managed)
}

fn ensure_managed_ignore(managed: &Path) -> Result<()> {
    let ignore_path = managed.join(".gitignore");
    match fs::symlink_metadata(&ignore_path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
            return validate_managed_ignore_contents(&ignore_path, metadata.len());
        }
        Ok(_) => return Err(CodensityError::InvalidManagedIgnore(ignore_path)),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(CodensityError::ManagedPathIo {
                path: ignore_path,
                source,
            });
        }
    }

    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&ignore_path)
    {
        Ok(mut file) => {
            if let Err(source) = file
                .write_all(MANAGED_IGNORE_CONTENTS)
                .and_then(|()| file.sync_all())
            {
                let _ = fs::remove_file(&ignore_path);
                return Err(CodensityError::ManagedPathIo {
                    path: ignore_path,
                    source,
                });
            }
            Ok(())
        }
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(&ignore_path).map_err(|source| {
                CodensityError::ManagedPathIo {
                    path: ignore_path.clone(),
                    source,
                }
            })?;
            if metadata.is_file() && !metadata.file_type().is_symlink() {
                validate_managed_ignore_contents(&ignore_path, metadata.len())
            } else {
                Err(CodensityError::InvalidManagedIgnore(ignore_path))
            }
        }
        Err(source) => Err(CodensityError::ManagedPathIo {
            path: ignore_path,
            source,
        }),
    }
}

fn validate_managed_ignore_contents(path: &Path, length: u64) -> Result<()> {
    if length
        != u64::try_from(MANAGED_IGNORE_CONTENTS.len()).map_err(|source| {
            CodensityError::ManagedPathIo {
                path: path.to_path_buf(),
                source: std::io::Error::other(source),
            }
        })?
    {
        return Err(CodensityError::ManagedIgnoreContentsMismatch(
            path.to_path_buf(),
        ));
    }
    let contents = fs::read(path).map_err(|source| CodensityError::ManagedPathIo {
        path: path.to_path_buf(),
        source,
    })?;
    if contents == MANAGED_IGNORE_CONTENTS {
        Ok(())
    } else {
        Err(CodensityError::ManagedIgnoreContentsMismatch(
            path.to_path_buf(),
        ))
    }
}

fn usable_parent(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn validate_manifest(manifest: &Manifest) -> Result<()> {
    if manifest.schema_version != DATABASE_SCHEMA_VERSION {
        return Err(CodensityError::UnsupportedManifestSchema {
            found: manifest.schema_version,
        });
    }
    let mut identities = BTreeSet::new();
    for (index, project) in manifest.projects.iter().enumerate() {
        validate_nonempty(index, "name", &project.name)?;
        validate_nonempty(index, "version", &project.version)?;
        validate_nonempty(index, "source_url", &project.source_url)?;
        if project
            .revision
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(CodensityError::EmptyManifestField {
                index,
                field: "revision",
            });
        }
        if let Some(digest) = &project.archive_sha256
            && (digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
        {
            return Err(CodensityError::InvalidArchiveSha256 { index });
        }
        if !project.path.exists() {
            return Err(CodensityError::InputNotFound(project.path.clone()));
        }
        if !project.path.is_dir() {
            return Err(CodensityError::ProjectPathNotDirectory(
                project.path.clone(),
            ));
        }
        if !identities.insert((&project.name, &project.version)) {
            return Err(CodensityError::DuplicateProject {
                name: project.name.clone(),
                version: project.version.clone(),
            });
        }
    }
    Ok(())
}

fn validate_nonempty(index: usize, field: &'static str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        Err(CodensityError::EmptyManifestField { index, field })
    } else {
        Ok(())
    }
}

fn database_project(
    project: ManifestProject,
    analysis: crate::model::AnalysisResult,
) -> DatabaseProject {
    DatabaseProject {
        name: project.name,
        version: project.version,
        revision: project.revision,
        source_url: project.source_url,
        archive_sha256: project.archive_sha256,
        analysis,
    }
}

pub(crate) fn write_atomic_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(CodensityError::OutputJson)?;
    bytes.push(b'\n');
    write_atomic_bytes(path, &bytes)
}

pub(crate) fn write_atomic_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    let (mut temporary, file) = create_temp_sibling(path)?;
    {
        let mut writer = file;
        writer
            .write_all(bytes)
            .map_err(|source| CodensityError::OutputIo {
                path: temporary.path.clone(),
                source,
            })?;
        writer.flush().map_err(|source| CodensityError::OutputIo {
            path: temporary.path.clone(),
            source,
        })?;
        writer
            .sync_all()
            .map_err(|source| CodensityError::OutputIo {
                path: temporary.path.clone(),
                source,
            })?;
    }
    fs::rename(&temporary.path, path).map_err(|source| CodensityError::AtomicRename {
        from: temporary.path.clone(),
        to: path.to_path_buf(),
        source,
    })?;
    temporary.committed = true;
    Ok(())
}

fn create_temp_sibling(path: &Path) -> Result<(TempOutput, File)> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| CodensityError::InvalidOutputPath(path.to_path_buf()))?;

    for _ in 0..100 {
        let sequence = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            ".{filename}.codensity.tmp.{}.{}",
            std::process::id(),
            sequence
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                return Ok((
                    TempOutput {
                        path: candidate,
                        committed: false,
                    },
                    file,
                ));
            }
            Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(source) => {
                return Err(CodensityError::OutputIo {
                    path: candidate,
                    source,
                });
            }
        }
    }
    Err(CodensityError::OutputIo {
        path: path.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "could not allocate a unique sibling temporary file",
        ),
    })
}
