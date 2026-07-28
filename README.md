# codensity

`codensity` 是一个可复现的源代码压缩密度分析工具。它先以确定规则选择并排序源码，再把原始字节串接为一个流，用固定参数压缩：

```text
compression_ratio = compressed_source_bytes / original_source_bytes
```

比率越低，表示在同一协议下源码字节具有更高的可压缩性；`savings = 1 - compression_ratio`。这个指标衡量的是固定语言映射、规模和过滤规则下的规律性与冗余度，**不能单独视为代码质量、可读性、可维护性或抽象水平的评分**。

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

## 基准数据库

当前工作目录中的 `.codensity/database-v1.json` 由正式 release CLI 连续生成两次，两次结果均为 24,053 字节，SHA-256 均为 `1e16035c9508d1c6cb4c23bf17747855cc757e9ce093d0c7d73ffedd0be59934`。数据来自 14 个直接克隆的 GitHub 仓库：Linux、FFmpeg，以及 anyhow、clap、rand、rayon、regex、reqwest、serde、serde_json、syn、thiserror、tokio 和 tracing。每个来源都固定到完整 commit SHA，分析输入是该提交的 tracked snapshot，不是发布压缩包或 crates.io 包内容。

主要描述性统计如下，四分位数使用 R-7 线性插值：

| 样本组 | n | Q1 | 中位数 | Q3 | 按字节加权 |
|---|---:|---:|---:|---:|---:|
| 全部项目 | 14 | 0.114 | 0.130 | 0.156 | 0.101 |
| Rust 仓库 | 12 | 0.123 | 0.133 | 0.158 | 0.127 |
| Rust `<512 KiB` | 3 | 0.167 | 0.178 | 0.182 | 0.176 |
| Rust `512 KiB–<2 MiB` | 4 | 0.123 | 0.142 | 0.156 | 0.135 |
| Rust `>=2 MiB` | 5 | 0.113 | 0.127 | 0.129 | 0.123 |
| C | 3 | 0.136 | 0.145 | 0.156 | 0.143 |
| C Header | 3 | 0.109 | 0.167 | 0.185 | 0.054 |
| Assembly | 2 | 0.117 | 0.130 | 0.143 | 0.136 |

这些结果只是当前样本与协议下的描述性范围。Linux 会主导“全部项目”和 C Header 的按字节加权统计；C、C Header 和 Assembly 分别只有 3、3、2 个观测，不能据此定义通用语言范围。小语料也会明显受到 zstd 帧头和有限样本效应影响。

完整 schema-v1 数据库作为 [`v0.1.0` Release 资产](https://github.com/LIghtJUNction/codensity/releases/download/v0.1.0/database-v1.json) 发布。下载后可用下面的命令核验：

```bash
sha256sum database-v1.json
# 1e16035c9508d1c6cb4c23bf17747855cc757e9ce093d0c7d73ffedd0be59934
```
