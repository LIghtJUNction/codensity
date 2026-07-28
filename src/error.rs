use std::path::PathBuf;

/// Errors returned by the codensity library.
#[derive(Debug, thiserror::Error)]
pub enum CodensityError {
    /// The requested input does not exist.
    #[error("input path does not exist: {0}")]
    InputNotFound(PathBuf),

    /// A manifest project path is not a directory.
    #[error("project path is not a directory: {0}")]
    ProjectPathNotDirectory(PathBuf),

    /// A filesystem walker could not inspect the input.
    #[error("failed to walk input `{root}`: {source}")]
    Walk {
        /// Root being walked.
        root: PathBuf,
        /// Underlying walker error.
        source: ignore::Error,
    },

    /// A source file could not be read.
    #[error("failed to read source file `{path}`: {source}")]
    SourceIo {
        /// Source file path.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// A path cannot be represented by the protocol's POSIX UTF-8 rule.
    #[error("path cannot be represented as a normalized UTF-8 relative path: {0}")]
    InvalidRelativePath(PathBuf),

    /// The selected source stream contains no bytes.
    #[error("input has no recognized non-empty source bytes: {0}")]
    NoSourceBytes(PathBuf),

    /// A metric counter overflowed.
    #[error("metric counter overflowed while analyzing `{0}`")]
    CounterOverflow(PathBuf),

    /// zstd stream creation or completion failed.
    #[error("zstd compression failed: {0}")]
    Compression(#[source] std::io::Error),

    /// The manifest could not be read.
    #[error("failed to read manifest `{path}`: {source}")]
    ManifestIo {
        /// Manifest path.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// The manifest is not valid JSON schema v1.
    #[error("failed to parse manifest `{path}`: {source}")]
    ManifestJson {
        /// Manifest path.
        path: PathBuf,
        /// Underlying JSON error.
        source: serde_json::Error,
    },

    /// The manifest schema version is unsupported.
    #[error("unsupported manifest schema version {found}; expected 1")]
    UnsupportedManifestSchema {
        /// Version found in the manifest.
        found: u32,
    },

    /// A required manifest field is empty.
    #[error("manifest project {index} has an empty `{field}` field")]
    EmptyManifestField {
        /// Zero-based project index.
        index: usize,
        /// Field name.
        field: &'static str,
    },

    /// An optional archive digest has an invalid representation.
    #[error("manifest project {index} has an invalid archive_sha256")]
    InvalidArchiveSha256 {
        /// Zero-based project index.
        index: usize,
    },

    /// Two manifest projects share the same identity.
    #[error("duplicate manifest project identity: ({name}, {version})")]
    DuplicateProject {
        /// Duplicate project name.
        name: String,
        /// Duplicate project version.
        version: String,
    },

    /// A project root could not be resolved to its canonical path.
    #[error("failed to resolve project path `{path}`: {source}")]
    ProjectCanonicalize {
        /// Project path from the manifest.
        path: PathBuf,
        /// Underlying canonicalization error.
        source: std::io::Error,
    },

    /// The database output parent could not be resolved.
    #[error("failed to resolve database output parent `{path}`: {source}")]
    OutputParentCanonicalize {
        /// Parent path being resolved.
        path: PathBuf,
        /// Underlying canonicalization error.
        source: std::io::Error,
    },

    /// A project-internal output is outside its managed directory.
    #[error(
        "database output `{output}` is inside project `{project}` but outside its direct `.codensity` directory"
    )]
    OutputInsideProject {
        /// Requested output path.
        output: PathBuf,
        /// Manifest project name.
        project: String,
    },

    /// The managed directory itself was requested as database output.
    #[error("database output cannot replace the reserved managed directory: {0}")]
    ManagedDirectoryOutputReserved(PathBuf),

    /// The managed ignore file was requested as database output.
    #[error("database output cannot replace the reserved managed ignore file: {0}")]
    ManagedIgnoreOutputReserved(PathBuf),

    /// A project's managed path exists but is not a real directory.
    #[error("managed codensity path must be a real directory, not a file or symlink: {0}")]
    InvalidManagedDirectory(PathBuf),

    /// A managed `.gitignore` path exists but is not a file.
    #[error("managed codensity ignore path is not a file: {0}")]
    InvalidManagedIgnore(PathBuf),

    /// An existing managed ignore file does not contain the exact managed rules.
    #[error("managed codensity ignore file has unexpected contents: {0}")]
    ManagedIgnoreContentsMismatch(PathBuf),

    /// A managed directory or ignore file could not be created.
    #[error("failed to prepare managed codensity path `{path}`: {source}")]
    ManagedPathIo {
        /// Managed path being prepared.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// The output path has no usable sibling filename.
    #[error("output path has no filename: {0}")]
    InvalidOutputPath(PathBuf),

    /// A sibling temporary output file could not be created or written.
    #[error("failed to write temporary database output `{path}`: {source}")]
    OutputIo {
        /// Temporary output path.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// Stable database JSON serialization failed.
    #[error("failed to serialize database: {0}")]
    DatabaseJson(#[source] serde_json::Error),

    /// The completed sibling file could not be atomically renamed.
    #[error("failed to rename `{from}` to `{to}`: {source}")]
    AtomicRename {
        /// Temporary sibling path.
        from: PathBuf,
        /// Destination path.
        to: PathBuf,
        /// Underlying rename error.
        source: std::io::Error,
    },
}

/// Library result type.
pub type Result<T> = std::result::Result<T, CodensityError>;
