<div align="center">

# `codensity`

#### SOURCE COMPRESSION LEDGER · 可复现的源码压缩密度记录

<sub>以固定协议保留可比较的字节事实，而不是把一个数字伪装成质量结论。</sub>

[![Rust 2024](https://img.shields.io/badge/Rust-2024-b55b36?style=flat-square&logo=rust&logoColor=white)](Cargo.toml)
[![Protocol v1](https://img.shields.io/badge/protocol-v1-315f54?style=flat-square)](#协议)
[![zstd level 19](https://img.shields.io/badge/zstd-level%2019-315f54?style=flat-square)](#协议)
[![25 language mappings](https://img.shields.io/badge/language%20mappings-25-315f54?style=flat-square)](#基准主流开源语言)
[![14 OSS snapshots](https://img.shields.io/badge/OSS%20fixed%20snapshots-14-315f54?style=flat-square)](#基准主流开源语言)
[![3 author self-disclosed AI samples](https://img.shields.io/badge/AI%20self--disclosed%20samples-3-315f54?style=flat-square)](#真实-ai-主导项目样本)

[`分析`](#分析源码) · [`协议`](#协议) · [`OSS 基准`](#基准主流开源语言) · [`真实 AI 样本`](#真实-ai-主导项目样本) · [`重建`](#从仓库根目录重建)

</div>

`codensity` 在固定、可复现的协议下统计源码的压缩密度：识别并排序源码文件，将其**原始字节**串接为一个规范流，以 zstd level 19 压缩，并报告 `compressed / original`。比率越低只表示同一协议下字节更容易压缩；它**不是**代码质量、可读性、可维护性、安全性或 AI 来源的评分。

![codensity 协议图：排序源码文件、无分隔串接原始字节、使用 zstd 19 独立帧压缩，最后计算压缩字节与原始字节之比](assets/codensity-protocol.svg)

<div align="center">
  <img src="assets/codensity-editorial.png" width="760" alt="暖色编辑插画：一台手动压印机将有序的源码纸带压成一条测量刻度，象征在固定协议下记录可复核的压缩结果。">
  <br>
  <sub>一份研究账册：先固定输入与程序，再阅读结果。</sub>
</div>

## 分析源码

```bash
codensity analyze
codensity analyze path/to/project --format text
codensity analyze path/to/project --format json
```

省略路径时，字面默认值是 `src`。文本输出适合阅读；JSON 使用数值比率，并提供 schema、工具、zstd 与协议版本、输入逻辑标签、总体结果、按语言结果和跳过文件数。

### 项目自分析

使用 release 构建分析 `codensity` 当前源码：

```bash
target/release/codensity analyze . --format json
```

| 指标 | 结果 |
|---|---:|
| Rust 文件 | 8 |
| 原始字节 | 75,404 |
| zstd 压缩字节 | 13,908 |
| 压缩比 | 0.184446 |
| 节省率 | 81.56% |
| 跳过文件 | 4 |
| 源码流 SHA-256 | `02577f73a282b1f94ebc8594bd9926b1b4a83ca5ab68ebadf18d1ef78b68c39f` |

这里的四个跳过文件是未被语言表识别的 `.gitignore`、`Cargo.toml`、`Cargo.lock` 和 `README.md`；工具不会读取未知扩展名的内容。`.git`、`.codensity` 和 `target` 等固定排除目录不计入跳过文件。

本项目的 `0.184446` 高于 12 个 Rust 仓库样本的中位数 `0.132867` 和四分位上界 `0.158087`，即在当前协议下比样本中的多数项目更难压缩。不过，本项目的 Rust 语料只有约 74 KiB，甚至小于基准中最小的项目；与小型 Rust 项目组的 `0.167389–0.181687` 相比则只略高于上沿。更合理的解释是小语料中的 zstd 帧固定开销占比更高、可供跨文件复用的重复模式更少，而不是代码质量较差。

## 协议

首版协议标识为 `codensity-zstd19-concat-v1`。它只扫描普通文件、不跟随符号链接、遵循 `.gitignore`、保留隐藏源码，并固定排除 `.git`、`.codensity`、`target`、`node_modules`、`vendor`、`dist`、`build`、`.next` 和 `.cache` 目录。受支持源码按 POSIX 风格相对路径逐字节排序，然后不添加路径、分隔符、长度、换行或转码地串接原始字节。

总体流及每种语言的流都分别使用单线程语义的 zstd 级别 19 压缩为一个独立帧，并对未压缩流计算 SHA-256。空的已识别文件计入文件数；只有空文件或没有可识别非空源码时返回错误。若某种语言只有空文件，它的比率和节省率是 `null`。总体压缩字节数通常不等于各语言压缩字节数之和，因为它们是不同的压缩流。

协议标识、schema 版本、codensity 版本和 zstd 版本都是结果的一部分。压缩参数、语言表、过滤规则、排序或串接方法变化都会影响可比较性，因此不能在相同协议标识下静默修改。

## 构建数据库

```bash
codensity database build manifest.json --output database.json
```

schema v1 清单示例：

```json
{
  "schema_version": 1,
  "projects": [
    {
      "name": "example",
      "version": "1.0.0",
      "revision": "0123456789abcdef0123456789abcdef01234567",
      "source_url": "https://github.com/example/example",
      "path": "/local/git-snapshots/example"
    }
  ]
}
```

`revision` 与 `archive_sha256` 可省略。Git 来源应把 `source_url` 设为 GitHub 仓库地址，并用完整 commit SHA 固定 `revision`；`path` 指向从该 commit 导出的 tracked snapshot。项目按 `(name, version)` 排序，重复项目、无效 schema、字段或本地目录会报错。输出可以位于所有项目之外；若规范化后的真实位置位于某个项目内，则只能放在该项目根目录直属的 `.codensity/` 子树中，工具会在分析前安全创建 `.codensity/` 及内容严格为 `*`、`!.gitignore` 两行规则的 `.gitignore`。已有规则不同会报错且不会覆盖；`.codensity` 目录本身和保留的 `.gitignore` 都不能作为数据库输出。这个托管目录是协议固定排除项，不会反馈进指标。其他项目内部输出会被拒绝；项目根重叠时，输出必须同时满足每个包含它的项目。

输出使用稳定的格式化 JSON，通过目标文件旁的临时文件完整写入后原子重命名；数据库只保留项目来源信息，不会序列化本地 `path`。在 Windows 上，标准库不能原子替换已有目标时，工具会安全报错并保留原目标，而不会先删除再重命名。

## 基准：主流开源语言

可复核的清单、采集规则和完整结果均在 [`benchmarks/`](benchmarks/)：
[`oss-manifest.json`](benchmarks/oss-manifest.json) 固定来源，
[`oss-database.json`](benchmarks/oss-database.json) 是 release CLI 的实际输出。
下表只列出整个 OSS cohort 中已识别、非空源码合计至少 **64 KiB** 的语言。
`n` 是该语言有非空源码的项目数；压缩比是各项目该语言压缩字节之和除以原始字节之和的**按字节加权**结果，不是项目比率的平均值。

| 语言 | n | 源码总量 | 按字节加权压缩比 | 节省率 |
|---|---:|---:|---:|---:|
| C | 1 | 9.56 MiB | 0.185095 | 81.49% |
| C Header | 4 | 1.01 MiB | 0.192790 | 80.72% |
| C# | 1 | 0.87 MiB | 0.105365 | 89.46% |
| C++ | 2 | 1.37 MiB | 0.111783 | 88.82% |
| C++ Header | 1 | 1.12 MiB | 0.077208 | 92.28% |
| Go | 1 | 0.59 MiB | 0.158183 | 84.18% |
| Java | 2 | 27.51 MiB | 0.106754 | 89.32% |
| JavaScript | 5 | 110.75 MiB | 0.038793 | 96.12% |
| Kotlin | 2 | 3.81 MiB | 0.132878 | 86.71% |
| Lua | 1 | 11.27 MiB | 0.132105 | 86.79% |
| Objective-C | 1 | 0.49 MiB | 0.106400 | 89.36% |
| PHP | 1 | 5.98 MiB | 0.107561 | 89.24% |
| Python | 5 | 0.59 MiB | 0.224487 | 77.55% |
| Ruby | 2 | 16.35 MiB | 0.133349 | 86.67% |
| Shell | 7 | 1.26 MiB | 0.213946 | 78.61% |
| Swift | 2 | 0.43 MiB | 0.129745 | 87.03% |
| TSX | 2 | 0.31 MiB | 0.191678 | 80.83% |
| TypeScript | 2 | 44.36 MiB | 0.098018 | 90.20% |

JSX 在这批固定快照中没有带 `.jsx` 扩展名的非空文件，因此没有把 JavaScript 的结果伪装成 JSX 结果；其它低于 64 KiB 的语言也同样省略。`n=1` 的行只描述那一个快照，不是语言总体结论；即使达到 64 KiB，框架惯例、代码生成、项目规模与文件布局都会影响结果。详细的零字节、低样本和逐项目结果保留在数据库中。

本批项目（链接均为公开 GitHub 仓库，完整 revision 和 archive SHA-256 见清单）：

| 项目 | 固定快照 |
|---|---|
| [AFNetworking](https://github.com/AFNetworking/AFNetworking) | `4.0.1` / `ffae2391` |
| [Catch2](https://github.com/catchorg/Catch2) | `v3.8.1` / `2b60af89` |
| [gin](https://github.com/gin-gonic/gin) | `v1.11.0` / `6ad6205e` |
| [Guava](https://github.com/google/guava) | `v33.4.8` / `f06690fa` |
| [kotlinx.coroutines](https://github.com/Kotlin/kotlinx.coroutines) | `1.10.2` / `5f890047` |
| [Laravel Framework](https://github.com/laravel/framework) | `v12.0.0` / `bd8aeb64` |
| [Neovim](https://github.com/neovim/neovim) | `v0.11.3` / `b2684d9f` |
| [Oh My Zsh](https://github.com/ohmyzsh/ohmyzsh) | `master-7ea697fd` / `7ea697fd` |
| [Rails](https://github.com/rails/rails) | `v8.0.2` / `32358275` |
| [React](https://github.com/facebook/react) | `v19.1.1` / `02ef4958` |
| [Requests](https://github.com/psf/requests) | `v2.32.5` / `b25c87d7` |
| [Serilog](https://github.com/serilog/serilog) | `v4.3.0` / `1b461379` |
| [Swift Algorithms](https://github.com/apple/swift-algorithms) | `1.2.1` / `87e50f48` |
| [TypeScript](https://github.com/microsoft/TypeScript) | `v5.9.3` / `c63de15a` |

`bminor/bash` 的 GitHub API snapshot 在采集时不可用，因此 Shell 语料采用了明显更小的、同样公开的 Oh My Zsh 固定快照；这不是对 Bash 或任何项目的比较性判断。

## 真实 AI 主导项目样本

[`benchmarks/real-ai-projects/`](benchmarks/real-ai-projects/) 记录三项公开 GitHub
项目的固定快照、作者自披露证据、archive SHA-256、清单与 release CLI 输出。它
响应“需要真实反面案例”的需求，但发布名称保持中性、可审计：这只是**作者自披露
的 AI 主导样本**，不是对任何项目的“垃圾”判定。

| 项目 | 作者自披露（固定 README） | 源码字节 | 压缩比 | 节省率 |
|---|---|---:|---:|---:|
| [CodePrism](https://github.com/rustic-ai/codeprism) | [entirely AI-generated](https://github.com/rustic-ai/codeprism/blob/8115b77568c16d1eb0710396da39232b33663fc0/README.md) | 4,164,594 | 0.132440 | 86.76% |
| [rust-docs-mcp](https://github.com/snowmead/rust-docs-mcp) | [entirely vibe coded](https://github.com/snowmead/rust-docs-mcp/blob/27b26e6b0cf2428cd16f628e86a83fdd01d78154/README.md) | 848,280 | 0.162071 | 83.79% |
| [ThePantry](https://github.com/tjacoby2006/ThePantry) | [vibe coded/AI slopped](https://github.com/tjacoby2006/ThePantry/blob/b0ef793069524dc0a4b9df37a727b7954f2fc51c/README.md) | 504,014 | 0.210002 | 79.00% |
| 合计（按字节加权） | 三个作者自披露样本 | 5,516,888 | 0.144082 | 85.59% |

这三条自披露只是入选依据，不能证明任何第三方项目的 AI 来源、质量或安全性；压缩
率也不是质量、安全性、可维护性或 AI 来源检测器。框架样板、生成代码、格式、语言、
规模与领域重复都可能改变比率。

## 从仓库根目录重建

下面的命令把 OSS 快照放进唯一的临时目录，不会把 archive 或 source snapshot 写入本仓库；每个下载都由清单中的完整 SHA 固定。命令结束后，两个 `cmp` 都应无输出。

```bash
cargo build --release
codensity_bin="$PWD/target/release/codensity"
workdir="$(mktemp -d)"
cp benchmarks/oss-manifest.json "$workdir/oss-manifest.json"
mkdir "$workdir/sources"

while read -r name repo revision; do
  archive="$workdir/$name.tar.gz"
  curl --fail --location --output "$archive" "https://codeload.github.com/$repo/tar.gz/$revision"
  expected="$(jq -r --arg name "$name" '.projects[] | select(.name == $name).archive_sha256' "$workdir/oss-manifest.json")"
  printf '%s  %s\n' "$expected" "$archive" | sha256sum --check --status
  mkdir "$workdir/sources/$name"
  tar -xzf "$archive" -C "$workdir/sources/$name" --strip-components=1
done <<'SOURCES'
afnetworking AFNetworking/AFNetworking ffae2391ab0c29dc88eb0a58d2f5b2c2c27cadbf
catch2 catchorg/Catch2 2b60af89e23d28eefc081bc930831ee9d45ea58b
gin gin-gonic/gin 6ad6205e9c94a4b8a320219e28c37c29d22a7a2c
guava google/guava f06690fa3e874f65515e8fd338a74d636e2c792f
kotlinx-coroutines Kotlin/kotlinx.coroutines 5f8900478a8e20c073145b1608fbc71fe3d7378b
laravel-framework laravel/framework bd8aeb64d3f9fa4b11690d702bdf289f5f32ae97
neovim neovim/neovim b2684d9f6658544d75e2431a06bcf21fe80673f8
oh-my-zsh ohmyzsh/ohmyzsh 7ea697fd8138550ddf7262456d412f0dcd1cbf84
rails rails/rails 3235827585d87661942c91bc81f64f56d710f0b2
react facebook/react 02ef49580922f87180f32618b9d1c70b75b968b7
requests psf/requests b25c87d7cb8d6a18a37fa12442b5f883f9e41741
serilog serilog/serilog 1b461379f4e218a939d5c94897df2a1dbbf90573
swift-algorithms apple/swift-algorithms 87e50f483c54e6efd60e885f7f5aa946cee68023
typescript microsoft/TypeScript c63de15a992d37f0d6cec03ac7631872838602cb
SOURCES

(cd "$workdir" && "$codensity_bin" database build oss-manifest.json --output oss-database.json)
cmp "$workdir/oss-database.json" benchmarks/oss-database.json
mkdir "$workdir/real-ai"
cp benchmarks/real-ai-projects/manifest.json "$workdir/real-ai/manifest.json"
mkdir "$workdir/real-ai/sources"

while read -r name repo revision; do
  archive="$workdir/real-ai/$name.tar.gz"
  curl --fail --location --output "$archive" "https://codeload.github.com/$repo/tar.gz/$revision"
  expected="$(jq -r --arg name "$name" '.projects[] | select(.name == $name).archive_sha256' "$workdir/real-ai/manifest.json")"
  printf '%s  %s\n' "$expected" "$archive" | sha256sum --check --status
  mkdir "$workdir/real-ai/sources/$name"
  tar -xzf "$archive" -C "$workdir/real-ai/sources/$name" --strip-components=1
done <<'SOURCES'
codeprism rustic-ai/codeprism 8115b77568c16d1eb0710396da39232b33663fc0
rust-docs-mcp snowmead/rust-docs-mcp 27b26e6b0cf2428cd16f628e86a83fdd01d78154
the-pantry tjacoby2006/ThePantry b0ef793069524dc0a4b9df37a727b7954f2fc51c
SOURCES

(cd "$workdir/real-ai" && "$codensity_bin" database build manifest.json --output database.json)
cmp "$workdir/real-ai/database.json" benchmarks/real-ai-projects/database.json
rm -rf "$workdir"
```
