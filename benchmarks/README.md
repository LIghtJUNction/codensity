# Benchmark corpus

This directory records reproducible, descriptive cohorts for the
`codensity-zstd19-concat-v1` protocol.  The tracked JSON databases are outputs
of the release CLI; source snapshots and downloaded archives are deliberately
not tracked.

## OSS cohort

[`oss-manifest.json`](oss-manifest.json) declares 20 established public GitHub
projects.  Every entry has a repository URL, readable snapshot label, full
immutable Git commit, and SHA-256 of the GitHub codeload archive used here.
The `path` values are deliberately local and relative to the directory from
which the manifest is run: put the manifest beside a `sources/` directory.
`oss-database.json` does not contain those local paths.

Source acquisition rules:

- Download only `https://codeload.github.com/<owner>/<repo>/tar.gz/<full-sha>`.
- Check the archive SHA-256 against the manifest, then extract with
  `--strip-components=1` into the matching `sources/<name>` directory.
- Do not use a branch, a moving tag, a release artifact, vendored dependency
  cache, or generated build output as a substitute for the pinned snapshot.
- Run the database command from the directory containing the copied manifest;
  this makes every local `path` unambiguous.  Keep that temporary directory
  outside the checkout and remove it afterwards.

The reported language table uses only languages with at least **64 KiB** of
non-empty recognized source across the cohort.  `n` is the number of projects
with non-empty source in that language.  The published ratio is byte-weighted:
the sum of each qualifying language's compressed bytes divided by the sum of
its original bytes, rather than an average of project ratios.  Empty recognised
files remain part of a project's file count but contribute no bytes.  The tool
also excludes its protocol-fixed directories and respects each project's
`.gitignore`; unknown extensions, archives, dependencies, and build products
are not manually added back.

The expansion adds Flask, Caddy, Vue Core, Sidekiq, fmt, and CocoaLumberjack.
This broadens fixed language and framework context; it does not make the cohort
a statistical sample of all open-source software. Within-language and size
context is the strongest supported comparison. Very low or high ratios, and
cross-cohort overlap, are observations rather than quality attribution.

Across all 20 snapshots, the byte-weighted overall ratio is `0.085356`
(`262,231,102` source bytes and `22,382,908` compressed bytes). This aggregate
is dominated by the largest projects and languages, so per-language results and
individual fixed snapshots remain necessary context.

## Author-self-disclosed AI-led cohort

[`real-ai-projects/`](real-ai-projects/) records five public GitHub projects
whose pinned READMEs directly disclose AI-led or vibe-coded creation. Its
manifest, archive hashes, reproducible acquisition procedure, source
attribution links, and release-generated database are all kept in that
directory.

Every included project has at least **64 KiB** of recognised, non-empty source
under this protocol and a pinned README blob URL containing its owner's direct
self-disclosure. `karpathy/llm-council` at
`92e1fccb1bdcf1bab7221aa9ed90f9dc72529131` was inspected but omitted: its
recognised non-empty source is `46,048` bytes, below that floor. It was not
padded or otherwise altered.

The five included snapshots have a byte-weighted overall ratio of `0.185407`
(`10,892,767` source bytes and `2,019,594` compressed bytes). Cross-cohort
overlap is expected: the project-level and language-level ranges intersect, so
the cohorts do not support a ratio-based attribution claim.

The cohort name is intentionally neutral and audit-friendly. It was selected
because cautionary examples were requested, but the owner statements are
selection evidence only: they do not establish a project is bad, unsafe, a
fork, malware, or representative of other projects. Compression ratio is not
a detector of quality, safety, maintainability, or AI origin; frameworks,
generated code, formatting, language, project size, and ordinary repetition
can all affect it.

## Future module relationship metric

The next product metric compares one whole-repository compression stream with
the sum of independently compressed submodule streams. Extra saving in the
whole stream can surface cross-module repetition for review, including shared
conventions and possible copied patterns. It is not yet emitted by the v1 CLI,
and it would not itself establish redundancy or quality.
