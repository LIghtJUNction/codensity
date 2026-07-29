use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::database::write_atomic_bytes;
use crate::{CodensityError, DATABASE_SCHEMA_VERSION, Database, PROTOCOL_ID, Result};

const RELEASE_ASSET: &str = "database-v1.json";
const API_ROOT: &str = "https://api.github.com/repos/LIghtJUNction/codensity/releases";
const DOWNLOAD_ROOT: &str = "https://github.com/LIghtJUNction/codensity/releases/download/";

#[derive(Deserialize)]
struct GithubRelease {
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Deserialize)]
struct GithubReleaseAsset {
    name: String,
    digest: Option<String>,
    browser_download_url: String,
}

trait ReleaseFetcher {
    fn fetch(&self, url: &str) -> Result<Vec<u8>>;
}

struct GithubFetcher;

impl ReleaseFetcher for GithubFetcher {
    fn fetch(&self, url: &str) -> Result<Vec<u8>> {
        let mut response = ureq::get(url)
            .header("Accept", "application/vnd.github+json")
            .header(
                "User-Agent",
                concat!("codensity/", env!("CARGO_PKG_VERSION")),
            )
            .call()
            .map_err(|source| CodensityError::ReleaseRequest {
                url: url.to_owned(),
                source,
            })?;
        response
            .body_mut()
            .read_to_vec()
            .map_err(|source| CodensityError::ReleaseResponse {
                url: url.to_owned(),
                source,
            })
    }
}

/// Downloads, verifies, and atomically stores the official release database.
///
/// The downloaded bytes must match GitHub's SHA-256 asset digest and decode as
/// this binary's schema- and protocol-compatible database before the existing
/// output is replaced.
pub fn update_database(tag: Option<&str>, output_path: &Path) -> Result<Database> {
    update_database_with(&GithubFetcher, tag, output_path)
}

fn update_database_with<F: ReleaseFetcher>(
    fetcher: &F,
    tag: Option<&str>,
    output_path: &Path,
) -> Result<Database> {
    let release_url = release_url(tag)?;
    let release_bytes = fetcher.fetch(&release_url)?;
    let release: GithubRelease = serde_json::from_slice(&release_bytes).map_err(|source| {
        CodensityError::ReleaseMetadataJson {
            url: release_url,
            source,
        }
    })?;
    let asset = release
        .assets
        .into_iter()
        .find(|asset| asset.name == RELEASE_ASSET)
        .ok_or(CodensityError::ReleaseAssetMissing {
            asset: RELEASE_ASSET,
        })?;
    let expected_digest = parse_digest(&asset.digest)?;
    if !asset.browser_download_url.starts_with(DOWNLOAD_ROOT) {
        return Err(CodensityError::ReleaseAssetUrl {
            asset: RELEASE_ASSET,
            url: asset.browser_download_url,
        });
    }

    let database_bytes = fetcher.fetch(&asset.browser_download_url)?;
    let actual_digest = format!("{:x}", Sha256::digest(&database_bytes));
    if actual_digest != expected_digest {
        return Err(CodensityError::ReleaseDigestMismatch {
            asset: RELEASE_ASSET,
        });
    }
    let database: Database =
        serde_json::from_slice(&database_bytes).map_err(CodensityError::ReleaseDatabaseJson)?;
    validate_database(&database)?;
    write_atomic_bytes(output_path, &database_bytes)?;
    Ok(database)
}

fn release_url(tag: Option<&str>) -> Result<String> {
    match tag {
        None => Ok(format!("{API_ROOT}/latest")),
        Some(tag) if !tag.is_empty() && tag.bytes().all(is_release_tag_byte) => {
            Ok(format!("{API_ROOT}/tags/{tag}"))
        }
        Some(tag) => Err(CodensityError::InvalidReleaseTag(tag.to_owned())),
    }
}

fn is_release_tag_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
}

fn parse_digest(digest: &Option<String>) -> Result<String> {
    let Some(value) = digest
        .as_deref()
        .and_then(|value| value.strip_prefix("sha256:"))
    else {
        return Err(CodensityError::ReleaseAssetDigestInvalid {
            asset: RELEASE_ASSET,
        });
    };
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CodensityError::ReleaseAssetDigestInvalid {
            asset: RELEASE_ASSET,
        });
    }
    Ok(value.to_ascii_lowercase())
}

fn validate_database(database: &Database) -> Result<()> {
    if database.schema_version != DATABASE_SCHEMA_VERSION {
        return Err(CodensityError::UnsupportedDatabaseSchema {
            found: database.schema_version,
        });
    }
    if database.protocol != PROTOCOL_ID {
        return Err(CodensityError::ReleaseProtocolMismatch {
            found: database.protocol.clone(),
            expected: PROTOCOL_ID,
        });
    }
    let mut identities = BTreeMap::new();
    for project in &database.projects {
        if identities
            .insert((&project.name, &project.version), ())
            .is_some()
        {
            return Err(CodensityError::DuplicateReleaseProject {
                name: project.name.clone(),
                version: project.version.clone(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{API_ROOT, RELEASE_ASSET, ReleaseFetcher, update_database_with};
    use crate::{CodensityError, Result as CodensityResult};
    use sha2::{Digest, Sha256};

    const RELEASE_REPOSITORY: &str = "LIghtJUNction/codensity";

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        path: PathBuf,
    }

    impl Fixture {
        fn new() -> std::result::Result<Self, std::io::Error> {
            let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "codensity-release-test-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path)?;
            Ok(Self { path })
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    struct MockFetcher {
        responses: BTreeMap<String, Vec<u8>>,
    }

    impl ReleaseFetcher for MockFetcher {
        fn fetch(&self, url: &str) -> CodensityResult<Vec<u8>> {
            self.responses
                .get(url)
                .cloned()
                .ok_or_else(|| CodensityError::InputNotFound(PathBuf::from(url)))
        }
    }

    #[test]
    fn update_should_verify_and_atomically_store_the_official_asset()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let fixture = Fixture::new()?;
        let output = fixture.path.join("database-v1.json");
        let asset_url = format!(
            "https://github.com/{RELEASE_REPOSITORY}/releases/download/v1.0.0/{RELEASE_ASSET}"
        );
        let database = database_bytes();
        let digest = format!("sha256:{:x}", Sha256::digest(&database));
        let metadata = format!(
            r#"{{"assets":[{{"name":"{RELEASE_ASSET}","digest":"{digest}","browser_download_url":"{asset_url}"}}]}}"#
        );
        let fetcher = MockFetcher {
            responses: BTreeMap::from([
                (format!("{API_ROOT}/latest"), metadata.into_bytes()),
                (asset_url, database.clone()),
            ]),
        };

        let result = update_database_with(&fetcher, None, &output)?;

        assert!(result.projects.is_empty() && fs::read(output)? == database);
        Ok(())
    }

    #[test]
    fn update_should_preserve_existing_output_when_digest_validation_fails()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let fixture = Fixture::new()?;
        let output = fixture.path.join("database-v1.json");
        fs::write(&output, b"previous database")?;
        let asset_url = format!(
            "https://github.com/{RELEASE_REPOSITORY}/releases/download/v1.0.0/{RELEASE_ASSET}"
        );
        let metadata = format!(
            r#"{{"assets":[{{"name":"{RELEASE_ASSET}","digest":"sha256:{}","browser_download_url":"{asset_url}"}}]}}"#,
            "0".repeat(64)
        );
        let fetcher = MockFetcher {
            responses: BTreeMap::from([
                (format!("{API_ROOT}/latest"), metadata.into_bytes()),
                (asset_url, database_bytes()),
            ]),
        };

        let result = update_database_with(&fetcher, None, &output);

        assert!(
            matches!(result, Err(CodensityError::ReleaseDigestMismatch { .. }))
                && fs::read(output)? == b"previous database"
        );
        Ok(())
    }

    fn database_bytes() -> Vec<u8> {
        br#"{
  "schema_version": 1,
  "codensity_version": "0.1.0",
  "zstd_version": "1.5.7",
  "protocol": "codensity-zstd19-concat-v1",
  "projects": []
}
"#
        .to_vec()
    }
}
