use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};

use crate::{CodensityError, Result};

const CODELOAD_ROOT: &str = "https://codeload.github.com";
const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ADVERTISEMENT_BYTES: u64 = 8 * 1024 * 1024;
const MAX_COMMIT_PAGE_BYTES: u64 = 4 * 1024 * 1024;
const CURRENT_OID_MARKER: &[u8] = br#""currentOid":""#;
static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Immutable provenance and temporary extraction root for one public GitHub repository.
#[derive(Debug)]
pub struct GithubSnapshot {
    /// Canonical public GitHub repository URL without a moving revision.
    pub repository_url: String,
    /// Exact immutable commit resolved before the archive was downloaded.
    pub commit: String,
    /// SHA-256 of the exact downloaded archive bytes.
    pub archive_sha256: String,
    root: PathBuf,
    _temporary: TemporaryDirectory,
}

impl GithubSnapshot {
    /// Returns the extracted repository root while this snapshot is alive.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// Downloads a public GitHub repository archive after resolving an immutable commit.
///
/// The accepted input is `https://github.com/OWNER/REPO`, optionally suffixed
/// with `.git`, `/tree/REF`, or `/commit/<40-hex-sha>`. The returned root is
/// owned by a temporary directory and is removed when the value is dropped.
pub fn fetch_github_snapshot(input: &str) -> Result<GithubSnapshot> {
    let request = RepositoryRequest::parse(input)?;
    let commit = resolve_commit(&request)?;
    let archive_url = format!(
        "{CODELOAD_ROOT}/{}/{}/tar.gz/{commit}",
        request.owner, request.repository,
    );
    let archive = fetch_archive_bytes(&archive_url)?;
    let archive_sha256 = format!("{:x}", Sha256::digest(&archive));
    let temporary = TemporaryDirectory::new()?;
    let archive_commit = extract_archive(&archive_url, &archive, temporary.path())?;
    if archive_commit != commit {
        return Err(CodensityError::RepositoryArchiveCommitMismatch {
            expected: commit,
            found: archive_commit,
        });
    }

    Ok(GithubSnapshot {
        repository_url: request.canonical_url(),
        commit: archive_commit,
        archive_sha256,
        root: temporary.path().to_path_buf(),
        _temporary: temporary,
    })
}

fn resolve_commit(request: &RepositoryRequest) -> Result<String> {
    if let Some(revision) = request
        .requested_revision
        .as_deref()
        .filter(|revision| is_commit_sha(revision))
    {
        return Ok(revision.to_ascii_lowercase());
    }

    let advertisement_url = format!(
        "https://github.com/{}/{}.git/info/refs?service=git-upload-pack",
        request.owner, request.repository,
    );
    let advertisement = match fetch_advertisement_bytes(&advertisement_url) {
        Ok(advertisement) => advertisement,
        Err(
            CodensityError::RepositoryRequest { .. } | CodensityError::RepositoryResponse { .. },
        ) => {
            return resolve_commit_from_page(request);
        }
        Err(error) => return Err(error),
    };
    let references = parse_upload_pack_advertisement(&advertisement_url, &advertisement)?;
    resolve_advertised_revision(request, &references)
}

fn resolve_commit_from_page(request: &RepositoryRequest) -> Result<String> {
    let revision = request.requested_revision.as_deref().unwrap_or("HEAD");
    let page_url = format!("{}/commit/{revision}", request.canonical_url());
    let page = fetch_commit_page_bytes(&page_url)?;
    extract_current_oid(&page_url, &page)
}

fn resolve_advertised_revision(
    request: &RepositoryRequest,
    references: &BTreeMap<String, String>,
) -> Result<String> {
    let revision = request.requested_revision.as_deref().unwrap_or("HEAD");
    let candidates = if revision == "HEAD" {
        vec!["HEAD".to_owned()]
    } else {
        vec![
            format!("refs/heads/{revision}"),
            format!("refs/tags/{revision}^{{}}"),
            format!("refs/tags/{revision}"),
        ]
    };
    candidates
        .into_iter()
        .find_map(|reference| references.get(&reference).cloned())
        .ok_or_else(|| CodensityError::RepositoryRevisionNotFound {
            repository_url: request.canonical_url(),
            revision: revision.to_owned(),
        })
}

#[derive(Debug, PartialEq, Eq)]
struct RepositoryRequest {
    owner: String,
    repository: String,
    requested_revision: Option<String>,
}

impl RepositoryRequest {
    fn parse(input: &str) -> Result<Self> {
        let trimmed = input.trim().trim_end_matches('/');
        let Some(path) = trimmed.strip_prefix("https://github.com/") else {
            return Err(CodensityError::UnsupportedRepositoryUrl(input.to_owned()));
        };
        let segments: Vec<_> = path.split('/').collect();
        let Some(owner) = segments
            .first()
            .filter(|value| is_repository_segment(value))
        else {
            return Err(CodensityError::UnsupportedRepositoryUrl(input.to_owned()));
        };
        let Some(repository_segment) = segments.get(1) else {
            return Err(CodensityError::UnsupportedRepositoryUrl(input.to_owned()));
        };
        let repository = repository_segment
            .strip_suffix(".git")
            .unwrap_or(repository_segment);
        if !is_repository_segment(repository) {
            return Err(CodensityError::UnsupportedRepositoryUrl(input.to_owned()));
        }

        let requested_revision = match segments.as_slice() {
            [_, _] => None,
            [_, _, "tree", revision] if is_revision_segment(revision) => {
                Some((*revision).to_owned())
            }
            [_, _, "commit", revision] if is_commit_sha(revision) => Some((*revision).to_owned()),
            _ => return Err(CodensityError::UnsupportedRepositoryUrl(input.to_owned())),
        };

        Ok(Self {
            owner: (*owner).to_owned(),
            repository: repository.to_owned(),
            requested_revision,
        })
    }

    fn canonical_url(&self) -> String {
        format!("https://github.com/{}/{}", self.owner, self.repository)
    }
}

fn fetch_archive_bytes(url: &str) -> Result<Vec<u8>> {
    let mut response = ureq::get(url)
        .header("Accept", "application/x-gzip")
        .header(
            "User-Agent",
            concat!("codensity/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|source| CodensityError::RepositoryRequest {
            url: url.to_owned(),
            source,
        })?;
    if response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|length| length > MAX_ARCHIVE_BYTES)
    {
        return Err(CodensityError::RepositoryArchiveTooLarge {
            url: url.to_owned(),
            maximum_bytes: MAX_ARCHIVE_BYTES,
        });
    }

    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX_ARCHIVE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| CodensityError::RepositoryResponse {
            url: url.to_owned(),
            source,
        })?;
    if u64::try_from(bytes.len()).is_ok_and(|length| length > MAX_ARCHIVE_BYTES) {
        return Err(CodensityError::RepositoryArchiveTooLarge {
            url: url.to_owned(),
            maximum_bytes: MAX_ARCHIVE_BYTES,
        });
    }
    Ok(bytes)
}

fn fetch_advertisement_bytes(url: &str) -> Result<Vec<u8>> {
    let mut response = ureq::get(url)
        .header("Accept", "application/x-git-upload-pack-advertisement")
        .header(
            "User-Agent",
            concat!("codensity/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|source| CodensityError::RepositoryRequest {
            url: url.to_owned(),
            source,
        })?;
    if response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|length| length > MAX_ADVERTISEMENT_BYTES)
    {
        return Err(CodensityError::RepositoryAdvertisementTooLarge {
            url: url.to_owned(),
            maximum_bytes: MAX_ADVERTISEMENT_BYTES,
        });
    }

    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX_ADVERTISEMENT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| CodensityError::RepositoryResponse {
            url: url.to_owned(),
            source,
        })?;
    if u64::try_from(bytes.len()).is_ok_and(|length| length > MAX_ADVERTISEMENT_BYTES) {
        return Err(CodensityError::RepositoryAdvertisementTooLarge {
            url: url.to_owned(),
            maximum_bytes: MAX_ADVERTISEMENT_BYTES,
        });
    }
    Ok(bytes)
}

fn fetch_commit_page_bytes(url: &str) -> Result<Vec<u8>> {
    let mut response = ureq::get(url)
        .header("Accept", "text/html")
        .header(
            "User-Agent",
            concat!("codensity/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|source| CodensityError::RepositoryRequest {
            url: url.to_owned(),
            source,
        })?;
    if response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|length| length > MAX_COMMIT_PAGE_BYTES)
    {
        return Err(CodensityError::RepositoryCommitPageTooLarge {
            url: url.to_owned(),
            maximum_bytes: MAX_COMMIT_PAGE_BYTES,
        });
    }

    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX_COMMIT_PAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| CodensityError::RepositoryResponse {
            url: url.to_owned(),
            source,
        })?;
    if u64::try_from(bytes.len()).is_ok_and(|length| length > MAX_COMMIT_PAGE_BYTES) {
        return Err(CodensityError::RepositoryCommitPageTooLarge {
            url: url.to_owned(),
            maximum_bytes: MAX_COMMIT_PAGE_BYTES,
        });
    }
    Ok(bytes)
}

fn parse_upload_pack_advertisement(url: &str, bytes: &[u8]) -> Result<BTreeMap<String, String>> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum State {
        Service,
        ServiceFlush,
        References,
        Complete,
    }

    let mut state = State::Service;
    let mut offset = 0;
    let mut references = BTreeMap::new();
    while offset < bytes.len() {
        if bytes.len() - offset < 4 {
            return advertisement_error(url, "truncated pkt-line length");
        }
        let length_text = std::str::from_utf8(&bytes[offset..offset + 4])
            .map_err(|_| advertisement_error_value(url, "non-UTF-8 pkt-line length"))?;
        let length = usize::from_str_radix(length_text, 16)
            .map_err(|_| advertisement_error_value(url, "non-hexadecimal pkt-line length"))?;
        offset += 4;
        if length == 0 {
            state = match state {
                State::ServiceFlush => State::References,
                State::References => State::Complete,
                State::Service | State::Complete => {
                    return advertisement_error(url, "unexpected flush pkt-line");
                }
            };
            continue;
        }
        if length < 4 || length - 4 > bytes.len() - offset {
            return advertisement_error(url, "truncated pkt-line payload");
        }
        let payload = &bytes[offset..offset + length - 4];
        offset += length - 4;
        match state {
            State::Service => {
                if payload != b"# service=git-upload-pack\n" {
                    return advertisement_error(url, "missing upload-pack service banner");
                }
                state = State::ServiceFlush;
            }
            State::References => parse_advertised_reference(url, payload, &mut references)?,
            State::ServiceFlush | State::Complete => {
                return advertisement_error(url, "unexpected data pkt-line");
            }
        }
    }
    if state != State::Complete {
        return advertisement_error(url, "advertisement did not terminate with a flush pkt-line");
    }
    Ok(references)
}

fn parse_advertised_reference(
    url: &str,
    payload: &[u8],
    references: &mut BTreeMap<String, String>,
) -> Result<()> {
    let payload = payload.strip_suffix(b"\n").ok_or_else(|| {
        advertisement_error_value(url, "reference pkt-line did not end with a newline")
    })?;
    let record = payload
        .split(|byte| *byte == b'\0')
        .next()
        .expect("split always yields one item");
    let Some(separator) = record.iter().position(|byte| *byte == b' ') else {
        return advertisement_error(url, "reference pkt-line did not contain a space");
    };
    let (commit, reference_with_separator) = record.split_at(separator);
    let reference = &reference_with_separator[1..];
    let Ok(commit) = std::str::from_utf8(commit) else {
        return advertisement_error(url, "reference commit was not UTF-8");
    };
    let Ok(reference) = std::str::from_utf8(reference) else {
        return advertisement_error(url, "reference name was not UTF-8");
    };
    if !is_commit_sha(commit) || !is_advertised_reference(reference) {
        return advertisement_error(url, "reference pkt-line had an invalid commit or name");
    }
    if references
        .insert(reference.to_owned(), commit.to_ascii_lowercase())
        .is_some()
    {
        return advertisement_error(url, "duplicate advertised reference");
    }
    Ok(())
}

fn is_advertised_reference(value: &str) -> bool {
    (value == "HEAD" || value.starts_with("refs/"))
        && value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b' ')
}

fn advertisement_error<T>(url: &str, reason: &str) -> Result<T> {
    Err(advertisement_error_value(url, reason))
}

fn advertisement_error_value(url: &str, reason: &str) -> CodensityError {
    CodensityError::RepositoryAdvertisementMalformed {
        url: url.to_owned(),
        reason: reason.to_owned(),
    }
}

fn extract_current_oid(url: &str, page: &[u8]) -> Result<String> {
    let mut commits = BTreeSet::new();
    let mut offset = 0;
    while let Some(index) = page[offset..]
        .windows(CURRENT_OID_MARKER.len())
        .position(|window| window == CURRENT_OID_MARKER)
    {
        let start = offset + index + CURRENT_OID_MARKER.len();
        let end = start.saturating_add(40);
        if end >= page.len() || page.get(end) != Some(&b'"') {
            return commit_page_error(url, "currentOid marker did not contain a 40-hex commit");
        }
        let commit = std::str::from_utf8(&page[start..end])
            .map_err(|_| commit_page_error_value(url, "currentOid marker was not UTF-8"))?;
        if !is_commit_sha(commit) {
            return commit_page_error(url, "currentOid marker did not contain a 40-hex commit");
        }
        commits.insert(commit.to_ascii_lowercase());
        offset = end + 1;
    }
    match commits.len() {
        0 => commit_page_error(url, "missing currentOid marker"),
        1 => Ok(commits
            .into_iter()
            .next()
            .expect("one currentOid commit was collected")),
        _ => commit_page_error(url, "multiple distinct currentOid markers"),
    }
}

fn commit_page_error<T>(url: &str, reason: &str) -> Result<T> {
    Err(commit_page_error_value(url, reason))
}

fn commit_page_error_value(url: &str, reason: &str) -> CodensityError {
    CodensityError::RepositoryCommitPageUnresolved {
        url: url.to_owned(),
        reason: reason.to_owned(),
    }
}

fn extract_archive(url: &str, bytes: &[u8], destination: &Path) -> Result<String> {
    let decoder = GzDecoder::new(Cursor::new(bytes));
    let mut archive = tar::Archive::new(decoder);
    let mut top_level = None::<OsString>;
    for entry in archive
        .entries()
        .map_err(|source| CodensityError::RepositoryArchive {
            url: url.to_owned(),
            source,
        })?
    {
        let mut entry = entry.map_err(|source| CodensityError::RepositoryArchive {
            url: url.to_owned(),
            source,
        })?;
        let entry_type = entry.header().entry_type();
        if entry_type.is_pax_global_extensions()
            || entry_type.is_pax_local_extensions()
            || entry_type.is_gnu_longname()
            || entry_type.is_gnu_longlink()
        {
            continue;
        }
        let entry_path = entry
            .path()
            .map_err(|source| CodensityError::RepositoryArchive {
                url: url.to_owned(),
                source,
            })?
            .into_owned();
        let relative = archive_relative_path(&entry_path, &mut top_level)?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let output = destination.join(&relative);
        if entry_type.is_dir() {
            fs::create_dir_all(&output).map_err(|source| CodensityError::RepositoryArchive {
                url: url.to_owned(),
                source,
            })?;
        } else if entry_type.is_file() {
            let Some(parent) = output.parent() else {
                return Err(CodensityError::RepositoryArchiveUnsafeEntry(entry_path));
            };
            fs::create_dir_all(parent).map_err(|source| CodensityError::RepositoryArchive {
                url: url.to_owned(),
                source,
            })?;
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output)
                .map_err(|source| CodensityError::RepositoryArchive {
                    url: url.to_owned(),
                    source,
                })?;
            std::io::copy(&mut entry, &mut file).map_err(|source| {
                CodensityError::RepositoryArchive {
                    url: url.to_owned(),
                    source,
                }
            })?;
        } else {
            // GitHub archives can contain harmless symbolic-link or metadata
            // entries. Never materialize or follow them; the source scanner
            // also deliberately excludes links.
            continue;
        }
    }
    let Some(top_level) = top_level.and_then(|value| value.into_string().ok()) else {
        return Err(CodensityError::RepositoryArchiveCommitMissing(
            url.to_owned(),
        ));
    };
    let Some((_, commit)) = top_level
        .rsplit_once('-')
        .filter(|(_, commit)| is_commit_sha(commit))
    else {
        return Err(CodensityError::RepositoryArchiveCommitMissing(
            url.to_owned(),
        ));
    };
    Ok(commit.to_ascii_lowercase())
}

fn archive_relative_path(path: &Path, top_level: &mut Option<OsString>) -> Result<PathBuf> {
    let Some(path) = path.to_str() else {
        return Err(CodensityError::RepositoryArchiveUnsafeEntry(
            path.to_path_buf(),
        ));
    };
    if path.starts_with('/') || path.contains('\\') {
        return Err(CodensityError::RepositoryArchiveUnsafeEntry(PathBuf::from(
            path,
        )));
    }
    let mut components = path.split('/').filter(|component| !component.is_empty());
    let Some(top) = components.next() else {
        return Err(CodensityError::RepositoryArchiveUnsafeEntry(PathBuf::from(
            path,
        )));
    };
    if !is_normal_archive_component(top) {
        return Err(CodensityError::RepositoryArchiveUnsafeEntry(PathBuf::from(
            path,
        )));
    }
    if let Some(expected) = top_level {
        if expected != top {
            return Err(CodensityError::RepositoryArchiveUnsafeEntry(PathBuf::from(
                path,
            )));
        }
    } else {
        *top_level = Some(OsString::from(top));
    }

    let mut relative = PathBuf::new();
    for component in components {
        if !is_normal_archive_component(component) {
            return Err(CodensityError::RepositoryArchiveUnsafeEntry(PathBuf::from(
                path,
            )));
        }
        relative.push(component);
    }
    Ok(relative)
}

fn is_normal_archive_component(component: &str) -> bool {
    if component.contains('\\') || component.contains(':') {
        return false;
    }
    let mut components = Path::new(component).components();
    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

fn is_repository_segment(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn is_revision_segment(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn is_commit_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Debug)]
struct TemporaryDirectory {
    path: PathBuf,
}

impl TemporaryDirectory {
    fn new() -> Result<Self> {
        for _ in 0..100 {
            let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "codensity-github-{}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(source) => {
                    return Err(CodensityError::RepositoryTemporaryDirectory { path, source });
                }
            }
        }
        let path = std::env::temp_dir().join("codensity-github-exhausted");
        Err(CodensityError::RepositoryTemporaryDirectory {
            path,
            source: std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "could not allocate a unique temporary directory",
            ),
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};
    use std::path::{Path, PathBuf};

    use flate2::Compression;
    use flate2::write::GzEncoder;

    use super::{
        RepositoryRequest, archive_relative_path, extract_archive, extract_current_oid,
        is_commit_sha, parse_upload_pack_advertisement, resolve_advertised_revision,
    };

    #[test]
    fn repository_url_parser_accepts_only_canonical_safe_forms() {
        assert_eq!(
            RepositoryRequest::parse("https://github.com/BurntSushi/ripgrep/tree/master")
                .expect("parse tree URL"),
            RepositoryRequest {
                owner: "BurntSushi".to_owned(),
                repository: "ripgrep".to_owned(),
                requested_revision: Some("master".to_owned()),
            }
        );
        assert!(RepositoryRequest::parse("https://example.com/a/b").is_err());
        assert!(RepositoryRequest::parse("https://github.com/a/b/tree/feature/x").is_err());
        assert!(is_commit_sha("0123456789abcdef0123456789abcdef01234567"));
    }

    #[test]
    fn archive_paths_require_one_normal_top_level_directory() {
        let mut top_level = None;
        assert_eq!(
            archive_relative_path(Path::new("repo-commit/src/lib.rs"), &mut top_level)
                .expect("safe path"),
            PathBuf::from("src/lib.rs")
        );
        assert!(
            archive_relative_path(Path::new("repo-commit/../escape.rs"), &mut top_level).is_err()
        );
        assert!(
            archive_relative_path(Path::new("repo-commit/..\\escape.rs"), &mut top_level).is_err()
        );
        assert!(archive_relative_path(Path::new("other/file.rs"), &mut top_level).is_err());
    }

    #[test]
    fn archive_top_level_directory_resolves_a_full_commit() {
        let commit = "0123456789abcdef0123456789abcdef01234567";
        let archive = compressed_tar(&format!("example-{commit}/src/lib.rs"), b"pub fn run() {}");
        let temporary = super::TemporaryDirectory::new().expect("create temporary directory");

        assert_eq!(
            extract_archive(
                "https://example.invalid/archive",
                &archive,
                temporary.path()
            )
            .expect("extract archive"),
            commit,
        );
        assert_eq!(
            std::fs::read(temporary.path().join("src/lib.rs")).expect("read extracted source"),
            b"pub fn run() {}",
        );
    }

    #[test]
    fn upload_pack_advertisement_resolves_head_branches_and_peeled_tags() {
        let head = "1111111111111111111111111111111111111111";
        let branch = "2222222222222222222222222222222222222222";
        let tag_object = "3333333333333333333333333333333333333333";
        let peeled_tag = "4444444444444444444444444444444444444444";
        let advertisement = upload_pack_advertisement(&[
            format!("{head} HEAD\0symref=HEAD:refs/heads/main\n"),
            format!("{branch} refs/heads/main\n"),
            format!("{tag_object} refs/tags/v1.0.0\n"),
            format!("{peeled_tag} refs/tags/v1.0.0^{{}}\n"),
        ]);
        let references =
            parse_upload_pack_advertisement("https://example.invalid/refs", &advertisement)
                .expect("parse valid advertisement");

        let base = RepositoryRequest::parse("https://github.com/example/repository")
            .expect("parse base URL");
        let branch_request =
            RepositoryRequest::parse("https://github.com/example/repository/tree/main")
                .expect("parse branch URL");
        let tag_request =
            RepositoryRequest::parse("https://github.com/example/repository/tree/v1.0.0")
                .expect("parse tag URL");
        assert_eq!(
            resolve_advertised_revision(&base, &references).expect("resolve HEAD"),
            head,
        );
        assert_eq!(
            resolve_advertised_revision(&branch_request, &references).expect("resolve branch"),
            branch,
        );
        assert_eq!(
            resolve_advertised_revision(&tag_request, &references).expect("resolve tag"),
            peeled_tag,
        );
    }

    #[test]
    fn upload_pack_advertisement_rejects_invalid_pkt_line_framing() {
        assert!(parse_upload_pack_advertisement("https://example.invalid/refs", b"0003").is_err());
        assert!(parse_upload_pack_advertisement("https://example.invalid/refs", b"zzzz").is_err());
        assert!(
            parse_upload_pack_advertisement(
                "https://example.invalid/refs",
                b"001e# service=git-upload-pack\n0000",
            )
            .is_err()
        );
    }

    #[test]
    fn commit_page_marker_resolves_once_and_rejects_missing_or_ambiguous_values() {
        let first = "0123456789abcdef0123456789abcdef01234567";
        let second = "fedcba9876543210fedcba9876543210fedcba98";
        let page = format!(r#"<script>{{"currentOid":"{first}"}}</script>"#);
        assert_eq!(
            extract_current_oid("https://example.invalid/commit/main", page.as_bytes())
                .expect("extract current OID"),
            first,
        );
        assert!(
            extract_current_oid("https://example.invalid/commit/main", b"<html></html>").is_err()
        );
        let ambiguous =
            format!(r#"<script>{{"currentOid":"{first}","currentOid":"{second}"}}</script>"#);
        assert!(
            extract_current_oid("https://example.invalid/commit/main", ambiguous.as_bytes(),)
                .is_err()
        );
    }

    #[test]
    fn temporary_directory_removes_extracted_contents_on_drop() {
        let temporary = super::TemporaryDirectory::new().expect("create temporary directory");
        let marker = temporary.path().join("marker");
        let mut file = std::fs::File::create(&marker).expect("create marker");
        file.write_all(b"temporary").expect("write marker");
        let path = temporary.path().to_path_buf();
        drop(temporary);
        assert!(!path.exists());
    }

    fn compressed_tar(path: &str, contents: &[u8]) -> Vec<u8> {
        let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
        {
            let mut archive = tar::Builder::new(&mut gzip);
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            archive
                .append_data(&mut header, path, Cursor::new(contents))
                .expect("append tar data");
            archive.finish().expect("finish tar archive");
        }
        gzip.finish().expect("finish gzip archive")
    }

    fn upload_pack_advertisement(records: &[String]) -> Vec<u8> {
        let mut output = pkt_line(b"# service=git-upload-pack\n");
        output.extend_from_slice(b"0000");
        for record in records {
            output.extend(pkt_line(record.as_bytes()));
        }
        output.extend_from_slice(b"0000");
        output
    }

    fn pkt_line(payload: &[u8]) -> Vec<u8> {
        let length = payload.len() + 4;
        let mut output = format!("{length:04x}").into_bytes();
        output.extend_from_slice(payload);
        output
    }
}
