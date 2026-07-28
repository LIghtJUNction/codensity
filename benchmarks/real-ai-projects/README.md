# 作者自披露的 AI 主导样本

这是一个可复现、描述性的三项目 cohort。它回应“需要真实反面案例”的
需求，但公开名称保持中性、可审计：入选仅依据项目作者在固定快照的 README
中对 AI 主导创作过程的直接披露，不把压缩结果当作质量、安全性或 AI 来源的
检测器。

| 项目 | 固定快照 | 作者自披露的入选证据 |
|---|---|---|
| [CodePrism](https://github.com/rustic-ai/codeprism) | `2025-08-12-8115b775` | [README](https://github.com/rustic-ai/codeprism/blob/8115b77568c16d1eb0710396da39232b33663fc0/README.md) 称项目为 “entirely AI-generated”。 |
| [rust-docs-mcp](https://github.com/snowmead/rust-docs-mcp) | `2026-07-09-27b26e6b` | [README](https://github.com/snowmead/rust-docs-mcp/blob/27b26e6b0cf2428cd16f628e86a83fdd01d78154/README.md) 称其为 “entirely vibe coded”。 |
| [ThePantry](https://github.com/tjacoby2006/ThePantry) | `2026-03-09-b0ef7930` | [README](https://github.com/tjacoby2006/ThePantry/blob/b0ef793069524dc0a4b9df37a727b7954f2fc51c/README.md) 自称 “vibe coded/AI slopped”。 |

这些是原作者自己的表述，不是本仓库对作者、项目或第三方的指控。此处不声称
任何项目是 fork、恶意软件或“垃圾项目”，也不依据压缩率给项目贴质量标签。
三条披露只用于说明为何这三个公开项目被选入样本；不能据此推断其他项目的来源、
安全性或质量。

[`manifest.json`](manifest.json) 保存实际 GitHub 仓库 URL、可读快照版本、完整
revision、精确 codeload archive 的 SHA-256 与相对本地路径。
[`database.json`](database.json) 是 release CLI 对这些固定快照的原始输出。

## 获取与重建

从仓库根目录执行以下命令。每个归档只从带完整 commit SHA 的 GitHub codeload
URL 下载；下载后先以清单的 SHA-256 核验，再提取。数据库命令在复制清单所在的
临时目录中运行，所以 `sources/<name>` 相对路径会正确解析。结束后的 `cmp` 无
输出即表示重建结果与跟踪数据库完全相同。

```bash
cargo build --release
codensity_bin="$PWD/target/release/codensity"
workdir="$(mktemp -d)"
cp benchmarks/real-ai-projects/manifest.json "$workdir/manifest.json"
mkdir "$workdir/sources"

while read -r name repo revision; do
  archive="$workdir/$name.tar.gz"
  curl --fail --location --output "$archive" "https://codeload.github.com/$repo/tar.gz/$revision"
  expected="$(jq -r --arg name "$name" '.projects[] | select(.name == $name).archive_sha256' "$workdir/manifest.json")"
  printf '%s  %s\n' "$expected" "$archive" | sha256sum --check --status
  mkdir "$workdir/sources/$name"
  tar -xzf "$archive" -C "$workdir/sources/$name" --strip-components=1
done <<'SOURCES'
codeprism rustic-ai/codeprism 8115b77568c16d1eb0710396da39232b33663fc0
rust-docs-mcp snowmead/rust-docs-mcp 27b26e6b0cf2428cd16f628e86a83fdd01d78154
the-pantry tjacoby2006/ThePantry b0ef793069524dc0a4b9df37a727b7954f2fc51c
SOURCES

(cd "$workdir" && "$codensity_bin" database build manifest.json --output database.json)
cmp "$workdir/database.json" benchmarks/real-ai-projects/database.json
rm -rf "$workdir"
```

The cohort is intentionally small and selected by explicit self-disclosure, so
it is not representative of AI-assisted development generally. Compression can
also reflect language, framework conventions, generated files, formatting,
project size, and repetition. A low or high ratio therefore establishes none
of quality, safety, maintainability, or authorship.
