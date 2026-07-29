---
name: Codensity
description: This skill should be used when the user asks to "analyze code compression", "分析压缩率", "measure codensity", "update the Codensity database", "initialize Codensity", "分析模块耦合", "分析内聚", "compare repository density", or assess source compression alongside module coupling or cohesion.
version: 0.1.0
---

# Codensity

Use Codensity as a deterministic source-compression ledger. Treat
`compressed_bytes / original_bytes` as a measure of byte-level regularity under
the pinned protocol, never as a score for quality, maintainability, security,
or AI authorship.

## Select the Correct Workflow

Choose one workflow before executing commands:

- Initialize a project-local snapshot with `codensity init [PATH]`.
- Measure a source tree with `codensity analyze [PATH] --format json`.
- Consume the published benchmark with `codensity database update`.
- Rebuild an auditable benchmark from pinned local snapshots with
  `codensity database build MANIFEST --output OUTPUT`.
- Assess architecture by combining compression measurements with a real import
  and call graph; do not infer coupling from compression alone.

Build the checked-out binary before any measurement that must be current:

```bash
cargo build --release --locked
codensity_bin="$PWD/target/release/codensity"
```

Use `$codensity_bin`, rather than a globally installed binary, for a source-tree
or benchmark claim. Record the CLI's `schema_version`, `protocol`, Codensity
version, zstd version, raw bytes, compressed bytes, ratio, and source-stream
SHA-256 with the result.

## Produce Evidence for Optimization, Not a Quality Label

For a repository-level optimization review, build the checked-out binary and
measure the repository root at file and function granularity:

```bash
"$codensity_bin" analyze . --granularity function --format json > /tmp/codensity-analysis.json
```

Extract and report all of the following before suggesting an optimization:

1. Record whole-stream provenance and metrics: schema, protocol, versions,
   source bytes, compressed bytes, ratio, SHA-256, language mix, and excluded
   source count.
2. Inspect profile signals separately: cross-compressor agreement, entropy,
   duplicate-window coverage, noise flags, file-size concentration, and the
   language baseline sample count. Treat baselines with fewer than five
   projects as directional only.
3. Rank files by source bytes and inspect only sufficiently large files before
   discussing compression ratios. Do not rank tiny files or functions by ratio.
4. Group parser-backed functions by kind and count `small_sample` records.
   Exclude or clearly flag functions below the fixed variance threshold when
   comparing function ratios.
5. Use `relation` or `compare` only to find byte-pattern candidates. Pair each
   candidate with CodeGraph import/call paths, API ownership, shared types,
   I/O ownership, and error propagation before calling it coupling or proposing
   a refactor.
6. Separate the final report into facts, bounded inferences, optimization
   candidates, and non-claims. Include a negative result when there is no
   evidence for a change.

Never classify a repository as high- or low-quality from a compression ratio,
an information-profile score, a similarity gain, or a function metric. Quality
requires evidence about required behavior, security, reliability,
maintainability, performance, and operational fitness. Consult
`references/code-quality-evidence.md` for the quality evidence matrix and
review order.

## Initialize Safely

Run:

```bash
"$codensity_bin" init path/to/project
```

Expect `.codensity/analysis.json` and its managed `.gitignore`. The protocol
excludes `.codensity`, so refreshing the snapshot cannot feed back into the
measured source stream. Treat a pre-existing managed directory with unexpected
contents as a hard error; do not overwrite it manually.

Do not initialize filesystem root or a user home directory without an explicit,
informed request to pass `--force`.

## Use Release Data or Rebuild It

For a consumer who needs the official published database, run:

```bash
"$codensity_bin" database update --output database-v1.json
"$codensity_bin" database update --tag v0.1.0 --output database-v1.json
```

`database update` accepts only the official `database-v1.json` release asset,
checks GitHub's `sha256:` digest, then validates database schema and protocol
before atomically replacing the destination. Report a failed update as failed;
do not reuse a partial download or claim that an old output is current.

For a reproducible producer workflow, acquire exact pinned snapshots, verify
each archive SHA-256, and run:

```bash
"$codensity_bin" database build manifest.json --output database.json
```

Never substitute a later repository revision when a pinned archive is
temporarily unavailable. Do not begin corpus collection from ad-hoc scripts;
the canonical CLI and protocol must define every published result.

## Analyze Architecture Without Overclaiming

Read `references/architecture-analysis.md` before reporting module coupling or
cohesion. Follow this order:

1. Establish the current source graph with CodeGraph when `.codegraph/` exists;
   otherwise inspect imports and call paths directly.
2. Measure whole source sets, directories, and sufficiently large individual
   modules with JSON output.
3. Compare whole-stream compression against the sum of independent module
   frames only as a cross-module regularity signal.
4. Identify real coupling from imports, public APIs, shared mutable resources,
   error propagation, I/O ownership, and cycle direction.
5. Judge cohesion from each module's responsibility, not from its ratio.

Separate facts, inferences, and recommendations in the final report. State the
sample size and language mix for every benchmark comparison. Treat small files
and small corpora as high-variance because frame overhead and sparse repetition
materially affect their ratios.

## Validate Changes

For Codensity implementation changes, run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --locked
cargo build --release --locked
git diff --check
```

For database-update changes, also run a real update into a temporary output and
verify its SHA-256, schema, protocol, and project count. Keep temporary
downloads outside the repository and remove them after validation.
