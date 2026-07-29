use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use codensity::{
    CodensityError, LANGUAGES, PROTOCOL_ID, RELATION_PROTOCOL_ID, analyze_path, build_database,
    initialize_project, language_for_path, relate_paths, render_text,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    path: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Result<Self> {
        let sequence = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "codensity-test-{name}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).with_context(|| format!("create fixture {}", path.display()))?;
        Ok(Self { path })
    }

    fn write(&self, relative: &str, contents: &[u8]) -> Result<()> {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create parent {}", parent.display()))?;
        }
        fs::write(&path, contents).with_context(|| format!("write {}", path.display()))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn language_table_should_cover_every_protocol_v1_mapping_in_canonical_order() {
    let expected = [
        ("Rust", &["rs"][..]),
        ("C", &["c"][..]),
        ("C Header", &["h"][..]),
        ("C++", &["cc", "cpp", "cxx"][..]),
        ("C++ Header", &["hh", "hpp", "hxx"][..]),
        ("Assembly", &["S", "s", "asm"][..]),
        ("Python", &["py"][..]),
        ("Go", &["go"][..]),
        ("JavaScript", &["js", "mjs", "cjs"][..]),
        ("JSX", &["jsx"][..]),
        ("TypeScript", &["ts", "mts", "cts"][..]),
        ("TSX", &["tsx"][..]),
        ("Java", &["java"][..]),
        ("Kotlin", &["kt", "kts"][..]),
        ("Swift", &["swift"][..]),
        ("Objective-C", &["m"][..]),
        ("Objective-C++", &["mm"][..]),
        ("C#", &["cs"][..]),
        ("Ruby", &["rb"][..]),
        ("PHP", &["php"][..]),
        ("Shell", &["sh", "bash", "zsh"][..]),
        ("Lua", &["lua"][..]),
        ("Zig", &["zig"][..]),
        ("Scala", &["scala", "sc"][..]),
        ("Haskell", &["hs", "lhs"][..]),
    ];
    let actual: Vec<_> = LANGUAGES
        .iter()
        .map(|language| (language.name, language.extensions))
        .collect();

    assert_eq!(actual, expected);
}

#[test]
fn language_for_path_should_recognize_every_required_extension() {
    for (expected_index, language) in LANGUAGES.iter().enumerate() {
        for extension in language.extensions {
            let path = PathBuf::from(format!("source.{extension}"));
            assert_eq!(
                language_for_path(&path),
                Some(expected_index),
                "extension {extension} did not map to {}",
                language.name
            );
        }
    }
}

#[test]
fn analyze_should_be_deterministic_across_different_creation_orders() -> Result<()> {
    let first = Fixture::new("order-first")?;
    first.write("z.rs", b"fn z() {}\n")?;
    first.write("a.py", b"print('a')\n")?;
    let second = Fixture::new("order-second")?;
    second.write("a.py", b"print('a')\n")?;
    second.write("z.rs", b"fn z() {}\n")?;

    let first_result = analyze_path(&first.path, "same")?;
    let second_result = analyze_path(&second.path, "same")?;

    assert_eq!(
        serde_json::to_vec(&first_result)?,
        serde_json::to_vec(&second_result)?
    );
    Ok(())
}

#[test]
fn analyze_should_respect_gitignore_without_requiring_git_metadata() -> Result<()> {
    let fixture = Fixture::new("gitignore")?;
    fixture.write(".gitignore", b"ignored.rs\n")?;
    fixture.write("ignored.rs", b"ignored")?;
    fixture.write("kept.rs", b"kept")?;

    let result = analyze_path(&fixture.path, "fixture")?;

    assert_eq!(result.overall.file_count, 1);
    Ok(())
}

#[test]
fn analyze_should_exclude_every_fixed_directory_component() -> Result<()> {
    let fixture = Fixture::new("excluded")?;
    fixture.write("kept.rs", b"kept")?;
    for directory in [
        ".git",
        ".codensity",
        "target",
        "node_modules",
        "vendor",
        "dist",
        "build",
        ".next",
        ".cache",
    ] {
        fixture.write(&format!("{directory}/excluded.rs"), b"excluded")?;
    }

    let result = analyze_path(&fixture.path, "fixture")?;

    assert_eq!(result.overall.file_count, 1);
    Ok(())
}

#[test]
fn analyze_should_include_hidden_source_files() -> Result<()> {
    let fixture = Fixture::new("hidden")?;
    fixture.write(".hidden.rs", b"hidden")?;

    let result = analyze_path(&fixture.path, "fixture")?;

    assert_eq!(result.overall.file_count, 1);
    Ok(())
}

#[test]
fn analyze_should_include_source_files_inside_hidden_directories() -> Result<()> {
    let fixture = Fixture::new("hidden-directory")?;
    fixture.write(".hidden/source.rs", b"hidden directory source")?;

    let result = analyze_path(&fixture.path, "fixture")?;

    assert_eq!(result.overall.file_count, 1);
    Ok(())
}

#[cfg(unix)]
#[test]
fn analyze_should_not_follow_source_file_or_directory_symlinks() -> Result<()> {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new("symlink")?;
    fixture.write("real/kept.rs", b"kept")?;
    symlink(
        fixture.path.join("real/kept.rs"),
        fixture.path.join("linked.rs"),
    )?;
    symlink(fixture.path.join("real"), fixture.path.join("linked-dir"))?;

    let result = analyze_path(&fixture.path, "fixture")?;

    assert_eq!(result.overall.file_count, 1);
    Ok(())
}

#[test]
fn analyze_should_count_exact_bytes_and_hash_sorted_raw_concatenation() -> Result<()> {
    let fixture = Fixture::new("accounting")?;
    fixture.write("b.rs", b"second")?;
    fixture.write("a.rs", b"first")?;
    let expected_hash = format!("{:x}", Sha256::digest(b"firstsecond"));

    let result = analyze_path(&fixture.path, "fixture")?;

    assert_eq!(
        (result.overall.original_bytes, result.overall.sha256),
        (11, expected_hash)
    );
    Ok(())
}

#[test]
fn analyze_should_produce_identical_compression_and_hashes_on_repeated_runs() -> Result<()> {
    let fixture = Fixture::new("repeat")?;
    fixture.write("a.rs", b"fn repeated() {}\nfn repeated() {}\n")?;
    fixture.write("b.py", b"print('repeat')\n")?;

    let first = analyze_path(&fixture.path, "fixture")?;
    let second = analyze_path(&fixture.path, "fixture")?;

    assert_eq!(serde_json::to_vec(&first)?, serde_json::to_vec(&second)?);
    Ok(())
}

#[test]
fn analyze_should_preserve_empty_language_with_null_ratios() -> Result<()> {
    let fixture = Fixture::new("empty-language")?;
    fixture.write("empty.py", b"")?;
    fixture.write("nonempty.rs", b"x")?;

    let result = analyze_path(&fixture.path, "fixture")?;
    let python = result
        .languages
        .iter()
        .find(|language| language.language == "Python")
        .context("Python result missing")?;

    assert_eq!(
        (
            result.overall.file_count,
            python.metric.file_count,
            python.metric.original_bytes,
            python.metric.ratio,
            python.metric.savings,
            python.metric.sha256.as_str(),
        ),
        (
            2,
            1,
            0,
            None,
            None,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        )
    );
    Ok(())
}

#[test]
fn analyze_should_reject_all_empty_recognized_sources() -> Result<()> {
    let fixture = Fixture::new("all-empty")?;
    fixture.write("empty.rs", b"")?;
    fixture.write("also-empty.py", b"")?;

    let error = analyze_path(&fixture.path, "fixture").context("expected empty-source error");

    assert!(matches!(
        error,
        Err(source) if matches!(
            source.downcast_ref::<CodensityError>(),
            Some(CodensityError::NoSourceBytes(_))
        )
    ));
    Ok(())
}

#[test]
fn analyze_should_reject_corpora_without_recognized_sources() -> Result<()> {
    let fixture = Fixture::new("no-recognized")?;
    fixture.write("data.bin", b"nonempty but unknown")?;

    let error = analyze_path(&fixture.path, "fixture").context("expected no-source error");

    assert!(matches!(
        error,
        Err(source) if matches!(
            source.downcast_ref::<CodensityError>(),
            Some(CodensityError::NoSourceBytes(_))
        )
    ));
    Ok(())
}

#[test]
fn analyze_should_count_unknown_extensions_without_inspecting_them() -> Result<()> {
    let fixture = Fixture::new("unknown")?;
    fixture.write("known.rs", b"x")?;
    fixture.write("unknown.bin", b"not source")?;

    let result = analyze_path(&fixture.path, "fixture")?;

    assert_eq!(result.skipped_file_count, 1);
    Ok(())
}

#[cfg(unix)]
#[test]
fn analyze_should_not_open_unreadable_unknown_regular_files() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new("unknown-unreadable")?;
    fixture.write("known.rs", b"known source")?;
    fixture.write("unknown.bin", b"must not be opened")?;
    fs::set_permissions(
        fixture.path.join("unknown.bin"),
        fs::Permissions::from_mode(0o000),
    )?;

    let result = analyze_path(&fixture.path, "fixture")?;

    assert_eq!(
        (result.overall.file_count, result.skipped_file_count),
        (1, 1)
    );
    Ok(())
}

#[test]
fn analysis_json_should_have_stable_schema_v2_shape_and_numeric_ratios() -> Result<()> {
    let fixture = Fixture::new("json-shape")?;
    fixture.write("main.rs", b"fn main() {}\n")?;
    let result = analyze_path(&fixture.path, "fixture")?;

    let value = serde_json::to_value(&result)?;
    let top_level_keys: std::collections::BTreeSet<_> = value
        .as_object()
        .context("analysis must be an object")?
        .keys()
        .map(String::as_str)
        .collect();
    let metric_keys: std::collections::BTreeSet<_> = value["overall"]
        .as_object()
        .context("overall must be an object")?
        .keys()
        .map(String::as_str)
        .collect();
    let language_keys: std::collections::BTreeSet<_> = value["languages"][0]
        .as_object()
        .context("language must be an object")?
        .keys()
        .map(String::as_str)
        .collect();
    let profile_keys: std::collections::BTreeSet<_> = value["profile"]
        .as_object()
        .context("profile must be an object")?
        .keys()
        .map(String::as_str)
        .collect();

    assert_eq!(
        (
            value["schema_version"].as_u64(),
            value["codensity_version"].as_str(),
            value["zstd_version"].as_str(),
            value["protocol"].as_str(),
            value["input_label"].as_str(),
            value["skipped_file_count"].as_u64(),
            value["overall"]["ratio"].is_number(),
            top_level_keys,
            metric_keys,
            language_keys,
            profile_keys,
        ),
        (
            Some(2),
            Some(env!("CARGO_PKG_VERSION")),
            Some(codensity::zstd_version()),
            Some(PROTOCOL_ID),
            Some("fixture"),
            Some(0),
            true,
            [
                "codensity_version",
                "input_label",
                "languages",
                "overall",
                "profile",
                "protocol",
                "schema_version",
                "skipped_file_count",
                "zstd_version",
            ]
            .into_iter()
            .collect(),
            [
                "compressed_bytes",
                "file_count",
                "original_bytes",
                "ratio",
                "savings",
                "sha256",
            ]
            .into_iter()
            .collect(),
            [
                "compressed_bytes",
                "file_count",
                "language",
                "original_bytes",
                "ratio",
                "savings",
                "sha256",
            ]
            .into_iter()
            .collect(),
            [
                "baselines",
                "compression",
                "duplication",
                "entropy",
                "interpretation",
                "noise",
                "protocol",
                "score",
                "structure",
            ]
            .into_iter()
            .collect(),
        )
    );
    assert_eq!(
        serde_json::from_value::<codensity::AnalysisResult>(value.clone())?,
        result
    );
    Ok(())
}

#[test]
fn text_rendering_should_be_byte_identical_for_repeated_results() -> Result<()> {
    let fixture = Fixture::new("text-stability")?;
    fixture.write("main.rs", b"fn main() {}\n")?;

    let first = analyze_path(&fixture.path, "fixture")?;
    let second = analyze_path(&fixture.path, "fixture")?;
    let first_text = render_text(&first);
    let second_text = render_text(&second);

    assert!(
        first_text == second_text
            && first_text.contains("schema: 2\n")
            && first_text.contains(&format!("codensity: {}\n", env!("CARGO_PKG_VERSION")))
            && first_text.contains(&format!("zstd: {}\n", codensity::zstd_version()))
            && first_text.contains(&format!("protocol: {PROTOCOL_ID}\n"))
            && first_text.contains("information_density:")
    );
    Ok(())
}

#[test]
fn cli_analyze_should_use_literal_src_as_default_path() -> Result<()> {
    let fixture = Fixture::new("cli-default")?;
    fixture.write("src/main.rs", b"fn main() {}\n")?;

    let output = Command::new(env!("CARGO_BIN_EXE_codensity"))
        .args(["analyze", "--format", "json"])
        .current_dir(&fixture.path)
        .output()?;
    let value: Value = serde_json::from_slice(&output.stdout)?;

    assert!(
        output.status.success() && value["input_label"] == "src",
        "status: {:?}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn relation_should_have_a_stable_json_schema_and_cli_output() -> Result<()> {
    let fixture = Fixture::new("relation-json")?;
    let repeated = b"fn shared_pattern() { let value = 42; }\n".repeat(512);
    fixture.write("src/z.rs", &repeated)?;
    fixture.write("src/a.rs", &repeated)?;
    let root = fixture.path.to_str().context("fixture path is not UTF-8")?;

    let result = relate_paths(&fixture.path, Path::new("src/z.rs"), Path::new("src/a.rs"))?;
    let library_value = serde_json::to_value(&result)?;
    let output = Command::new(env!("CARGO_BIN_EXE_codensity"))
        .args([
            "relation", "--root", root, "src/a.rs", "src/z.rs", "--format", "json",
        ])
        .output()?;
    let cli_value: Value = serde_json::from_slice(&output.stdout)?;
    let keys: std::collections::BTreeSet<_> = library_value
        .as_object()
        .context("relation result must be an object")?
        .keys()
        .map(String::as_str)
        .collect();

    assert_eq!(library_value, cli_value);
    assert!(
        output.status.success()
            && cli_value["schema_version"] == 1
            && cli_value["protocol"] == RELATION_PROTOCOL_ID
            && cli_value["first"]["path"] == "src/a.rs"
            && cli_value["second"]["path"] == "src/z.rs"
            && cli_value["adjusted_cross_stream_gain_bytes"]
                .as_i64()
                .is_some_and(|value| value > 0)
            && cli_value["adjusted_cross_stream_gain_ratio"].is_number()
    );
    assert_eq!(
        keys,
        [
            "adjusted_cross_stream_gain_bytes",
            "adjusted_cross_stream_gain_ratio",
            "codensity_version",
            "combined",
            "empty_frame_bytes",
            "first",
            "interpretation",
            "protocol",
            "raw_cross_stream_gain_bytes",
            "schema_version",
            "second",
            "zstd_version",
        ]
        .into_iter()
        .collect()
    );
    Ok(())
}

#[test]
fn init_should_create_a_managed_deterministic_snapshot() -> Result<()> {
    let fixture = Fixture::new("init")?;
    fixture.write("src/main.rs", b"fn main() {}\n")?;
    let path_argument = fixture.path.to_string_lossy().into_owned();

    let first = Command::new(env!("CARGO_BIN_EXE_codensity"))
        .args(["init", path_argument.as_str()])
        .output()?;
    let snapshot_path = fixture.path.join(".codensity/analysis.json");
    let first_snapshot = fs::read(&snapshot_path)?;
    let second = initialize_project(&fixture.path, false)?;
    let second_snapshot = fs::read(&snapshot_path)?;
    let snapshot: Value = serde_json::from_slice(&second_snapshot)?;

    assert!(
        first.status.success()
            && String::from_utf8_lossy(&first.stdout).contains("initialized:")
            && first_snapshot == second_snapshot
            && snapshot["overall"]["sha256"] == second.overall.sha256
            && fs::read(fixture.path.join(".codensity/.gitignore"))? == b"*\n!.gitignore\n"
    );
    Ok(())
}

#[test]
fn cli_analyze_should_offer_the_frozen_ledger_only_mode() -> Result<()> {
    let fixture = Fixture::new("cli-ledger-only")?;
    fixture.write("main.rs", b"fn main() {}\n")?;

    let output = Command::new(env!("CARGO_BIN_EXE_codensity"))
        .args([
            "analyze",
            fixture.path.to_str().context("fixture path is not UTF-8")?,
            "--format",
            "json",
            "--ledger-only",
        ])
        .output()?;
    let value: Value = serde_json::from_slice(&output.stdout)?;

    assert!(
        output.status.success()
            && value["schema_version"] == 1
            && value
                .as_object()
                .is_some_and(|value| !value.contains_key("profile")),
        "status: {:?}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn init_should_require_force_for_filesystem_root() {
    assert!(matches!(
        initialize_project(Path::new("/"), false),
        Err(CodensityError::InitializationRequiresForce(_))
    ));
}

#[test]
fn database_should_sort_projects_and_omit_every_local_path() -> Result<()> {
    let fixture = Fixture::new("database-sort")?;
    fixture.write("a/src.rs", b"a")?;
    fixture.write("z/src.rs", b"z")?;
    let manifest_path = fixture.path.join("manifest.json");
    let output_path = fixture.path.join("database.json");
    let manifest = json!({
        "schema_version": 1,
        "projects": [
            {
                "name": "zeta",
                "version": "2",
                "source_url": "https://example.com/zeta",
                "path": fixture.path.join("z")
            },
            {
                "name": "alpha",
                "version": "1",
                "source_url": "https://example.com/alpha",
                "path": fixture.path.join("a")
            }
        ]
    });
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;

    build_database(&manifest_path, &output_path)?;
    let bytes = fs::read(&output_path)?;
    let value: Value = serde_json::from_slice(&bytes)?;
    let local_path_leaked = decoded_json_contains(&value, &fixture.path.to_string_lossy());

    assert_eq!(
        (
            value["projects"][0]["name"].as_str(),
            value["projects"][1]["name"].as_str(),
            local_path_leaked,
            bytes.last().copied(),
        ),
        (Some("alpha"), Some("zeta"), false, Some(b'\n'))
    );
    Ok(())
}

#[test]
fn database_should_create_and_exclude_managed_internal_output() -> Result<()> {
    let fixture = Fixture::new("database-managed")?;
    fixture.write("project/main.rs", b"fn main() {}\n")?;
    fixture.write("project/unknown.bin", b"unknown")?;
    let manifest_path = fixture.path.join("manifest.json");
    let project_path = fixture.path.join("project");
    let output_path = project_path.join(".codensity/database.json");
    write_one_project_manifest(&manifest_path, "managed", &project_path)?;

    let first = build_database(&manifest_path, &output_path)?;
    let first_bytes = fs::read(&output_path)?;
    let second = build_database(&manifest_path, &output_path)?;
    let second_bytes = fs::read(&output_path)?;
    let ignore_contents = fs::read(project_path.join(".codensity/.gitignore"))?;

    assert_eq!(
        (
            first.projects[0].analysis.overall.clone(),
            first.projects[0].analysis.skipped_file_count,
            first_bytes,
            ignore_contents,
        ),
        (
            second.projects[0].analysis.overall.clone(),
            second.projects[0].analysis.skipped_file_count,
            second_bytes,
            b"*\n!.gitignore\n".to_vec(),
        )
    );
    Ok(())
}

#[test]
fn database_should_reject_mismatched_managed_ignore_without_mutation() -> Result<()> {
    let fixture = Fixture::new("database-managed-ignore")?;
    fixture.write("project/main.rs", b"fn main() {}\n")?;
    fixture.write("project/.codensity/.gitignore", b"custom existing rules\n")?;
    let manifest_path = fixture.path.join("manifest.json");
    let project_path = fixture.path.join("project");
    write_one_project_manifest(&manifest_path, "managed", &project_path)?;

    let error = build_database(
        &manifest_path,
        &project_path.join(".codensity/database.json"),
    );

    assert!(
        matches!(error, Err(CodensityError::ManagedIgnoreContentsMismatch(_)))
            && fs::read(project_path.join(".codensity/.gitignore"))? == b"custom existing rules\n"
    );
    Ok(())
}

#[test]
fn database_should_reject_empty_managed_ignore_without_mutation() -> Result<()> {
    let fixture = Fixture::new("database-empty-ignore")?;
    fixture.write("project/main.rs", b"fn main() {}\n")?;
    fixture.write("project/.codensity/.gitignore", b"")?;
    let manifest_path = fixture.path.join("manifest.json");
    let project_path = fixture.path.join("project");
    write_one_project_manifest(&manifest_path, "managed", &project_path)?;

    let error = build_database(
        &manifest_path,
        &project_path.join(".codensity/database.json"),
    );

    assert!(
        matches!(error, Err(CodensityError::ManagedIgnoreContentsMismatch(_)))
            && fs::read(project_path.join(".codensity/.gitignore"))?.is_empty()
    );
    Ok(())
}

#[test]
fn database_should_reject_reserved_managed_ignore_output_without_mutation() -> Result<()> {
    let fixture = Fixture::new("database-reserved-ignore")?;
    fixture.write("project/main.rs", b"fn main() {}\n")?;
    fixture.write("project/.codensity/.gitignore", b"*\n!.gitignore\n")?;
    let manifest_path = fixture.path.join("manifest.json");
    let project_path = fixture.path.join("project");
    let ignore_path = project_path.join(".codensity/.gitignore");
    write_one_project_manifest(&manifest_path, "managed", &project_path)?;

    let error = build_database(&manifest_path, &ignore_path);

    assert!(
        matches!(error, Err(CodensityError::ManagedIgnoreOutputReserved(_)))
            && fs::read(&ignore_path)? == b"*\n!.gitignore\n"
    );
    Ok(())
}

#[test]
fn database_should_reject_managed_directory_as_output() -> Result<()> {
    let fixture = Fixture::new("database-reserved-directory")?;
    fixture.write("project/main.rs", b"fn main() {}\n")?;
    fixture.write("project/.codensity/.gitignore", b"*\n!.gitignore\n")?;
    let manifest_path = fixture.path.join("manifest.json");
    let project_path = fixture.path.join("project");
    write_one_project_manifest(&manifest_path, "managed", &project_path)?;

    let error = build_database(&manifest_path, &project_path.join(".codensity"));

    assert!(matches!(
        error,
        Err(CodensityError::ManagedDirectoryOutputReserved(_))
    ));
    Ok(())
}

#[test]
fn database_should_reject_managed_path_when_it_is_a_file() -> Result<()> {
    let fixture = Fixture::new("database-managed-file")?;
    fixture.write("project/main.rs", b"fn main() {}\n")?;
    fixture.write("project/.codensity", b"not a directory")?;
    let manifest_path = fixture.path.join("manifest.json");
    let project_path = fixture.path.join("project");
    write_one_project_manifest(&manifest_path, "managed", &project_path)?;

    let error = build_database(
        &manifest_path,
        &project_path.join(".codensity/database.json"),
    );

    assert!(matches!(
        error,
        Err(CodensityError::InvalidManagedDirectory(_))
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn database_should_allow_lexically_internal_output_whose_parent_symlink_is_external() -> Result<()>
{
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new("database-internal-symlink-external")?;
    fixture.write("project/main.rs", b"fn main() {}\n")?;
    fs::create_dir(fixture.path.join("external-output"))?;
    symlink(
        fixture.path.join("external-output"),
        fixture.path.join("project/output-link"),
    )?;
    let manifest_path = fixture.path.join("manifest.json");
    let project_path = fixture.path.join("project");
    write_one_project_manifest(&manifest_path, "managed", &project_path)?;

    build_database(
        &manifest_path,
        &project_path.join("output-link/database.json"),
    )?;

    assert!(fixture.path.join("external-output/database.json").is_file());
    Ok(())
}

#[cfg(unix)]
#[test]
fn database_should_reject_lexically_external_symlink_into_nonmanaged_project_space() -> Result<()> {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new("database-external-symlink-internal")?;
    fixture.write("project/main.rs", b"fn main() {}\n")?;
    fs::create_dir(fixture.path.join("project/generated"))?;
    symlink(
        fixture.path.join("project/generated"),
        fixture.path.join("external-link"),
    )?;
    let manifest_path = fixture.path.join("manifest.json");
    let project_path = fixture.path.join("project");
    write_one_project_manifest(&manifest_path, "managed", &project_path)?;

    let error = build_database(
        &manifest_path,
        &fixture.path.join("external-link/database.json"),
    );

    assert!(matches!(
        error,
        Err(CodensityError::OutputInsideProject { .. })
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn database_should_prepare_missing_managed_directory_through_project_symlink_alias() -> Result<()> {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new("database-project-alias")?;
    fixture.write("real-project/main.rs", b"fn main() {}\n")?;
    symlink(
        fixture.path.join("real-project"),
        fixture.path.join("project-alias"),
    )?;
    let manifest_path = fixture.path.join("manifest.json");
    let alias_path = fixture.path.join("project-alias");
    write_one_project_manifest(&manifest_path, "managed", &alias_path)?;

    build_database(&manifest_path, &alias_path.join(".codensity/database.json"))?;

    assert!(
        fixture
            .path
            .join("real-project/.codensity/database.json")
            .is_file()
            && fs::read(fixture.path.join("real-project/.codensity/.gitignore"))?
                == b"*\n!.gitignore\n"
    );
    Ok(())
}

#[test]
fn database_should_enforce_managed_output_rule_for_every_overlapping_root() -> Result<()> {
    let fixture = Fixture::new("database-overlapping-roots")?;
    fixture.write("outer/outer.rs", b"outer")?;
    fixture.write("outer/inner/inner.rs", b"inner")?;
    let manifest_path = fixture.path.join("manifest.json");
    let inner_path = fixture.path.join("outer/inner");
    let manifest = json!({
        "schema_version": 1,
        "projects": [
            {
                "name": "a-outer",
                "version": "1",
                "source_url": "https://example.com/outer",
                "path": fixture.path.join("outer")
            },
            {
                "name": "b-inner",
                "version": "1",
                "source_url": "https://example.com/inner",
                "path": inner_path
            }
        ]
    });
    fs::write(&manifest_path, serde_json::to_vec(&manifest)?)?;

    let error = build_database(
        &manifest_path,
        &fixture.path.join("outer/inner/.codensity/database.json"),
    );

    assert!(
        matches!(error, Err(CodensityError::OutputInsideProject { .. }))
            && !fixture.path.join("outer/inner/.codensity").exists()
    );
    Ok(())
}

#[test]
fn database_should_reject_nonmanaged_internal_json_output_before_analysis() -> Result<()> {
    assert_nonmanaged_internal_output_is_rejected("database.json")
}

#[test]
fn database_should_reject_nonmanaged_internal_source_output_before_analysis() -> Result<()> {
    assert_nonmanaged_internal_output_is_rejected("database.rs")
}

#[test]
fn database_should_support_output_external_to_project_roots() -> Result<()> {
    let fixture = Fixture::new("database-external")?;
    fixture.write("project/main.rs", b"fn main() {}\n")?;
    let manifest_path = fixture.path.join("manifest.json");
    let output_path = fixture.path.join("database.json");
    write_one_project_manifest(&manifest_path, "external", &fixture.path.join("project"))?;

    build_database(&manifest_path, &output_path)?;

    assert!(output_path.is_file());
    Ok(())
}

#[test]
fn database_cli_should_resolve_bare_external_output_against_current_directory() -> Result<()> {
    let fixture = Fixture::new("database-bare-output")?;
    fixture.write("project/main.rs", b"fn main() {}\n")?;
    let manifest_path = fixture.path.join("manifest.json");
    write_one_project_manifest(&manifest_path, "external", &fixture.path.join("project"))?;
    let manifest_argument = manifest_path.to_string_lossy().into_owned();

    let output = Command::new(env!("CARGO_BIN_EXE_codensity"))
        .args([
            "database",
            "build",
            manifest_argument.as_str(),
            "--output",
            "database.json",
        ])
        .current_dir(&fixture.path)
        .output()?;

    assert!(
        output.status.success() && fixture.path.join("database.json").is_file(),
        "status: {:?}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn database_should_reject_duplicate_name_and_version_pairs() -> Result<()> {
    let fixture = Fixture::new("database-duplicate")?;
    fixture.write("project/src.rs", b"x")?;
    let manifest_path = fixture.path.join("manifest.json");
    let project = json!({
        "name": "same",
        "version": "1",
        "source_url": "https://example.com/source",
        "path": fixture.path.join("project")
    });
    fs::write(
        &manifest_path,
        serde_json::to_vec(&json!({
            "schema_version": 1,
            "projects": [project.clone(), project]
        }))?,
    )?;

    let error = build_database(&manifest_path, &fixture.path.join("out.json"))
        .context("expected duplicate error");

    assert!(matches!(
        error,
        Err(source) if matches!(
            source.downcast_ref::<CodensityError>(),
            Some(CodensityError::DuplicateProject { .. })
        )
    ));
    Ok(())
}

#[test]
fn database_should_reject_unsupported_manifest_schema_versions() -> Result<()> {
    let fixture = Fixture::new("database-schema")?;
    let manifest_path = fixture.path.join("manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_vec(&json!({
            "schema_version": 2,
            "projects": []
        }))?,
    )?;

    let error = build_database(&manifest_path, &fixture.path.join("out.json"))
        .context("expected schema error");

    assert!(matches!(
        error,
        Err(source) if matches!(
            source.downcast_ref::<CodensityError>(),
            Some(CodensityError::UnsupportedManifestSchema { found: 2 })
        )
    ));
    Ok(())
}

#[test]
fn database_should_reject_missing_project_paths() -> Result<()> {
    let fixture = Fixture::new("database-missing")?;
    let manifest_path = fixture.path.join("manifest.json");
    write_one_project_manifest(
        &manifest_path,
        "missing",
        &fixture.path.join("does-not-exist"),
    )?;

    let error = build_database(&manifest_path, &fixture.path.join("out.json"))
        .context("expected missing-path error");

    assert!(matches!(
        error,
        Err(source) if matches!(
            source.downcast_ref::<CodensityError>(),
            Some(CodensityError::InputNotFound(_))
        )
    ));
    Ok(())
}

#[test]
fn database_should_reject_project_paths_that_are_files() -> Result<()> {
    let fixture = Fixture::new("database-file-path")?;
    fixture.write("project.rs", b"x")?;
    let manifest_path = fixture.path.join("manifest.json");
    write_one_project_manifest(&manifest_path, "file", &fixture.path.join("project.rs"))?;

    let error = build_database(&manifest_path, &fixture.path.join("out.json"))
        .context("expected non-directory error");

    assert!(matches!(
        error,
        Err(source) if matches!(
            source.downcast_ref::<CodensityError>(),
            Some(CodensityError::ProjectPathNotDirectory(_))
        )
    ));
    Ok(())
}

#[test]
fn database_failure_should_not_partially_overwrite_destination() -> Result<()> {
    let fixture = Fixture::new("database-atomic-failure")?;
    fixture.write("empty/empty.rs", b"")?;
    let manifest_path = fixture.path.join("manifest.json");
    let output_path = fixture.path.join("database.json");
    write_one_project_manifest(&manifest_path, "empty", &fixture.path.join("empty"))?;
    fs::write(&output_path, b"existing destination")?;

    let result = build_database(&manifest_path, &output_path);
    let destination = fs::read(&output_path)?;

    assert!(result.is_err() && destination == b"existing destination");
    Ok(())
}

#[test]
fn database_rename_failure_should_preserve_destination_and_remove_temporary_sibling() -> Result<()>
{
    let fixture = Fixture::new("database-rename-failure")?;
    fixture.write("project/main.rs", b"fn main() {}\n")?;
    let manifest_path = fixture.path.join("manifest.json");
    let output_path = fixture.path.join("existing-directory");
    fs::create_dir(&output_path)?;
    write_one_project_manifest(&manifest_path, "project", &fixture.path.join("project"))?;

    let result = build_database(&manifest_path, &output_path);
    let temporary_remains = fs::read_dir(&fixture.path)?.any(|entry| {
        entry
            .ok()
            .and_then(|entry| entry.file_name().into_string().ok())
            .is_some_and(|name| name.contains(".codensity.tmp."))
    });

    assert!(
        matches!(result, Err(CodensityError::AtomicRename { .. }))
            && output_path.is_dir()
            && !temporary_remains
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn database_should_atomically_replace_existing_regular_output() -> Result<()> {
    let fixture = Fixture::new("database-replace")?;
    fixture.write("project/main.rs", b"fn main() {}\n")?;
    let manifest_path = fixture.path.join("manifest.json");
    let output_path = fixture.path.join("database.json");
    fs::write(&output_path, b"old output")?;
    write_one_project_manifest(&manifest_path, "project", &fixture.path.join("project"))?;

    build_database(&manifest_path, &output_path)?;
    let value: Value = serde_json::from_slice(&fs::read(&output_path)?)?;

    assert_eq!(value["schema_version"], 1);
    assert!(
        value["projects"][0]["analysis"]
            .as_object()
            .is_some_and(|analysis| !analysis.contains_key("profile"))
    );
    Ok(())
}

#[test]
fn overall_compressed_size_may_differ_from_per_language_frame_sum() -> Result<()> {
    let fixture = Fixture::new("independent-frames")?;
    fixture.write("a.rs", b"same repeated repeated repeated\n")?;
    fixture.write("b.py", b"same repeated repeated repeated\n")?;

    let result = analyze_path(&fixture.path, "fixture")?;
    let language_sum: u64 = result
        .languages
        .iter()
        .map(|language| language.metric.compressed_bytes)
        .sum();

    assert_ne!(result.overall.compressed_bytes, language_sum);
    Ok(())
}

#[test]
fn profile_should_cross_check_algorithms_and_record_zstd_curve() -> Result<()> {
    let fixture = Fixture::new("compression-profile")?;
    fixture.write(
        "main.rs",
        b"fn repeated() { println!(\"density\"); }\n"
            .repeat(128)
            .as_slice(),
    )?;

    let result = analyze_path(&fixture.path, "fixture")?;
    let profile = result.profile.context("profile missing")?;
    let algorithms: Vec<_> = profile
        .compression
        .algorithms
        .iter()
        .map(|measurement| measurement.algorithm.as_str())
        .collect();
    let levels: Vec<_> = profile
        .compression
        .zstd_curve
        .iter()
        .map(|point| point.level)
        .collect();

    assert_eq!(algorithms, ["gzip", "zstd", "brotli", "xz"]);
    assert_eq!(levels, [1, 3, 9, 19, 22]);
    assert!(profile.compression.consensus_ratio > 0.0);
    assert!(profile.compression.ratio_spread >= 0.0);
    Ok(())
}

#[test]
fn profile_should_detect_repeated_blocks_without_parsing_an_ast() -> Result<()> {
    let fixture = Fixture::new("duplication-profile")?;
    let repeated = b"pub fn copied_block() {\n    let value = 42;\n    println!(\"{value}\");\n}\n"
        .repeat(128);
    fixture.write("first.rs", &repeated)?;
    fixture.write("second.rs", &repeated)?;

    let result = analyze_path(&fixture.path, "fixture")?;
    let profile = result.profile.context("profile missing")?;

    assert!(profile.duplication.duplicate_ratio > 0.30);
    assert_eq!(profile.duplication.window_bytes, 64);
    assert_eq!(profile.score.template_repetition_risk, "high");
    Ok(())
}

#[test]
fn profile_should_penalize_random_looking_noise() -> Result<()> {
    let fixture = Fixture::new("noise-profile")?;
    let mut state = 0x1234_5678_9abc_def0_u64;
    let mut random = Vec::with_capacity(16 * 1024);
    for _ in 0..16 * 1024 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        random.push((state & 0xff) as u8);
    }
    fixture.write("noise.rs", &random)?;

    let result = analyze_path(&fixture.path, "fixture")?;
    let profile = result.profile.context("profile missing")?;

    assert!(profile.noise.noise_ratio > 0.90);
    assert!(profile.score.signal < 10.0);
    assert!(profile.score.compression < 10.0);
    assert!(profile.score.information_density < 50.0);
    Ok(())
}

#[test]
fn profile_score_should_use_bounded_weights_and_expose_baseline_confidence() -> Result<()> {
    let fixture = Fixture::new("score-profile")?;
    fixture.write(
        "main.py",
        b"def calculate(value):\n    return value * 2 + 1\n"
            .repeat(2048)
            .as_slice(),
    )?;

    let result = analyze_path(&fixture.path, "fixture")?;
    let profile = result.profile.context("profile missing")?;
    let weights = &profile.score.weights;
    let total = weights.compression
        + weights.entropy
        + weights.uniqueness
        + weights.signal
        + weights.distribution;
    let python = profile
        .baselines
        .iter()
        .find(|baseline| baseline.language == "Python")
        .context("Python baseline missing")?;

    assert!((total - 1.0).abs() < f64::EPSILON);
    assert!(
        [
            weights.compression,
            weights.entropy,
            weights.uniqueness,
            weights.signal,
            weights.distribution,
        ]
        .into_iter()
        .all(|weight| weight <= 0.30)
    );
    assert_eq!(python.sample_count, 3);
    assert!(python.percentile.is_some());
    assert!(matches!(
        profile.score.confidence.as_str(),
        "low" | "medium" | "high"
    ));
    Ok(())
}

fn write_one_project_manifest(manifest_path: &Path, name: &str, path: &Path) -> Result<()> {
    let manifest = json!({
        "schema_version": 1,
        "projects": [{
            "name": name,
            "version": "1",
            "source_url": "https://example.com/source",
            "path": path
        }]
    });
    fs::write(manifest_path, serde_json::to_vec(&manifest)?)?;
    Ok(())
}

fn assert_nonmanaged_internal_output_is_rejected(filename: &str) -> Result<()> {
    let fixture = Fixture::new("database-feedback-rejection")?;
    fixture.write("project/unknown.bin", b"no recognized sources")?;
    let manifest_path = fixture.path.join("manifest.json");
    let project_path = fixture.path.join("project");
    let output_path = project_path.join(filename);
    write_one_project_manifest(&manifest_path, "project", &project_path)?;

    let error = build_database(&manifest_path, &output_path)
        .context("expected project-internal output error");

    assert!(
        matches!(
            error,
            Err(source) if matches!(
                source.downcast_ref::<CodensityError>(),
                Some(CodensityError::OutputInsideProject { .. })
            )
        ) && !output_path.exists()
    );
    Ok(())
}

fn decoded_json_contains(value: &Value, needle: &str) -> bool {
    match value {
        Value::String(value) => value.contains(needle),
        Value::Array(values) => values
            .iter()
            .any(|value| decoded_json_contains(value, needle)),
        Value::Object(values) => values
            .values()
            .any(|value| decoded_json_contains(value, needle)),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}
