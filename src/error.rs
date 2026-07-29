use std::path::PathBuf;

/// Errors returned by the codensity library.
#[derive(Debug, thiserror::Error)]
pub enum CodensityError {
    /// The requested input does not exist.
    #[error("input path does not exist: {0}")]
    InputNotFound(PathBuf),

    /// A relation root is not a directory.
    #[error("relation root is not a directory: {0}")]
    RelationRootNotDirectory(PathBuf),

    /// A relation path is not safely relative to its root.
    #[error("relation source path must be a normalized relative path within its root: {0}")]
    RelationPathOutsideRoot(PathBuf),

    /// A relation path is not an included recognized regular source file.
    #[error("relation source is not an included recognized regular file under `{root}`: {path}")]
    RelationSourceUnavailable {
        /// Relation root.
        root: PathBuf,
        /// Requested relative path.
        path: PathBuf,
    },

    /// Both relation arguments select the same canonical source file.
    #[error("relation requires two distinct source files: {0}")]
    RelationDuplicateSource(PathBuf),

    /// A manifest project path is not a directory.
    #[error("project path is not a directory: {0}")]
    ProjectPathNotDirectory(PathBuf),

    /// An initialization target is not a directory.
    #[error("initialization path is not a directory: {0}")]
    InitializationPathNotDirectory(PathBuf),

    /// An initialization target could not be resolved to a canonical path.
    #[error("failed to resolve initialization path `{path}`: {source}")]
    InitializationCanonicalize {
        /// Initialization path supplied by the user.
        path: PathBuf,
        /// Underlying canonicalization error.
        source: std::io::Error,
    },

    /// An initialization target is too broad without explicit confirmation.
    #[error("refusing to initialize filesystem root or home directory without --force: {0}")]
    InitializationRequiresForce(PathBuf),

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

    /// Stable JSON serialization failed.
    #[error("failed to serialize output JSON: {0}")]
    OutputJson(#[source] serde_json::Error),

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

    /// A release tag cannot safely form a GitHub API URL segment.
    #[error("release tag contains unsupported characters: {0}")]
    InvalidReleaseTag(String),

    /// GitHub release metadata or an asset could not be requested.
    #[error("failed to request GitHub release resource `{url}`: {source}")]
    ReleaseRequest {
        /// Requested HTTPS URL.
        url: String,
        /// HTTP client failure.
        source: ureq::Error,
    },

    /// A GitHub release response body could not be read.
    #[error("failed to read GitHub release resource `{url}`: {source}")]
    ReleaseResponse {
        /// Requested HTTPS URL.
        url: String,
        /// Response stream failure.
        source: ureq::Error,
    },

    /// GitHub release metadata was not valid JSON.
    #[error("failed to parse GitHub release metadata `{url}`: {source}")]
    ReleaseMetadataJson {
        /// Requested release API URL.
        url: String,
        /// JSON parsing failure.
        source: serde_json::Error,
    },

    /// The expected protocol-versioned database asset was absent from a release.
    #[error("GitHub release does not contain required asset `{asset}`")]
    ReleaseAssetMissing {
        /// Required asset name.
        asset: &'static str,
    },

    /// The release asset did not provide a SHA-256 digest in GitHub's format.
    #[error("GitHub release asset `{asset}` has no valid sha256 digest")]
    ReleaseAssetDigestInvalid {
        /// Required asset name.
        asset: &'static str,
    },

    /// The release asset URL was not an official repository download URL.
    #[error("GitHub release asset `{asset}` has an unexpected download URL: {url}")]
    ReleaseAssetUrl {
        /// Required asset name.
        asset: &'static str,
        /// Untrusted download URL returned in release metadata.
        url: String,
    },

    /// The downloaded bytes did not match GitHub's published SHA-256 digest.
    #[error("GitHub release asset `{asset}` failed SHA-256 verification")]
    ReleaseDigestMismatch {
        /// Required asset name.
        asset: &'static str,
    },

    /// A verified release asset was not a valid database JSON document.
    #[error("failed to parse verified release database: {0}")]
    ReleaseDatabaseJson(#[source] serde_json::Error),

    /// A verified release database used an unsupported schema.
    #[error("unsupported release database schema version {found}; expected 1")]
    UnsupportedDatabaseSchema {
        /// Schema version found in the downloaded database.
        found: u32,
    },

    /// A verified release database used a different metric protocol.
    #[error("release database protocol `{found}` does not match `{expected}`")]
    ReleaseProtocolMismatch {
        /// Protocol in the downloaded database.
        found: String,
        /// Protocol required by this binary.
        expected: &'static str,
    },

    /// A release database contains the same project identity more than once.
    #[error("release database contains duplicate project identity: ({name}, {version})")]
    DuplicateReleaseProject {
        /// Duplicate project name.
        name: String,
        /// Duplicate project version.
        version: String,
    },
}

/// Library result type.
pub type Result<T> = std::result::Result<T, CodensityError>;
