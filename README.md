# codensity

[![crates.io](https://img.shields.io/crates/v/codensity.svg?style=flat-square)](https://crates.io/crates/codensity)
[![MIT](https://img.shields.io/crates/l/codensity.svg?style=flat-square)](LICENSE)
[![CI](https://github.com/LIghtJUNction/codensity/actions/workflows/ci.yml/badge.svg)](https://github.com/LIghtJUNction/codensity/actions/workflows/ci.yml)
[![database refresh](https://github.com/LIghtJUNction/codensity/actions/workflows/refresh-database.yml/badge.svg)](https://github.com/LIghtJUNction/codensity/actions/workflows/refresh-database.yml)
[![protocol](https://img.shields.io/badge/protocol-codensity--zstd19--concat--v1-315f54?style=flat-square)](#测量协议)

`codensity` 是一个可复现的源码测量工具。它记录固定源码字节流的压缩表现，并补充重复、熵、噪声、文件分布、语言上下文以及文件/函数视图。它为审查和优化提供证据，不给仓库或作者打分。

当前公开基准含 **23 个固定快照的成熟开源项目**，以及单独维护的 **5 个作者明确披露 AI 主导创作过程的项目**。每个样本均可追溯到仓库 URL、完整 commit、归档 SHA-256、协议与生成数据库。它们是描述性 cohort，不是行业平均值，更不是质量标签。

## 安装与开始

```bash
cargo install codensity --locked

# 创建或刷新项目本地快照。
codensity init .

# 仅删除 Codensity 自己的受管理状态。
codensity clean .

# 默认输出供快速阅读；JSON 适合仪表盘或笔记本。
codensity analyze .
codensity analyze . --format json
```

`init` 写入 `.codensity/analysis.json` 和很小的 `cache-v1.json`。该受管理目录被协议固定排除，记录快照不会反过来改变被测源码流。它可以安全刷新，但不能替代版本控制。

`init` 会先计算由规范路径和每个已识别文件内容 SHA-256 构成的源码清单。清单、协议、工具版本、zstd 版本和快照 digest 都匹配时，直接复用上次完整快照并显示 `cache: hit`；否则重新分析并显示 `cache: miss`。因此未变更的仓库不会反复进行完整多压缩器分析，而内容、路径、协议或运行时版本变化一定会失效缓存。缓存只有一个受校验的快照记录，不会随运行次数累积。

`codensity clean .` 只会删除完整、可验证的 `.codensity/` 状态；陌生文件、符号链接或不匹配的受管理内容会使它拒绝删除。对文件系统根目录或用户主目录执行 `init` 或 `clean` 必须显式添加 `--force`。

## 测量协议

冻结账本协议为 `codensity-zstd19-concat-v1`：

1. 扫描可识别的普通源码文件，遵循 `.gitignore`，并排除 `.git`、`target`、`node_modules`、`.codensity` 等固定目录和构建产物。
2. 按 POSIX 相对路径排序。
3. 原样串接文件字节：不加入路径、分隔符，也不做文本规范化。
4. 对这一个字节流以 zstd level 19 压缩，记录 `compressed_bytes / original_bytes`、源码 SHA-256、版本和文件数。

比率描述的是字节层面的规则性。比率较低，只表示固定压缩器在这段字节流中找到了更多可复用模式；它不表示设计更好、代码更安全，也不表示代码由人或模型编写。

### 重复实验，而不是“二次压缩”

同一份固定源码、同一个固定二进制应重复分析，然后比较完整 JSON：

```bash
codensity analyze . --format json > /tmp/codensity-a.json
codensity analyze . --format json > /tmp/codensity-b.json
cmp /tmp/codensity-a.json /tmp/codensity-b.json
```

应当一致的是协议、工具与 zstd 版本、源码字节数、源码 SHA-256 和最终指标。把已压缩输出再压缩一次不是代码质量测试，工具不会把它包装成这样的结论。

完整信息画像还使用 gzip、zstd、Brotli、XZ 四种压缩器，观察 zstd 级别曲线、字节熵、重复窗口覆盖、噪声风险和文件大小集中度。它们有不同的盲点；没有任何一项可以单独变成质量结论。

## 可执行的分析

```bash
# 定位大文件或异常区域；函数模式为 Rust 的解析器支持模式。
codensity analyze . --granularity file --format json
codensity analyze . --granularity function --format json

# 分析公开 GitHub 仓库的不可变快照；输出包含 commit 与 codeload 归档 digest。
codensity analyze https://github.com/BurntSushi/ripgrep --granularity function

# 检查一个工作区内两个源码文件的共同字节模式。
codensity relation --root . src/a.rs src/b.rs --format json

# 比较两个仓库的整体流 C(A)、C(B) 和 C(A+B)。
codensity compare https://github.com/BurntSushi/ripgrep https://github.com/serde-rs/serde

# 下载并校验官方发布的数据库，原子替换目标文件。
codensity database update --output database-v1.json
```

`relation` 和 `compare` 只测量跨流字节复用：共享模板、命名习惯或复制片段都可能产生信号。它们不测语义相似、抄袭、import/call 耦合、依赖方向或因果关系。若要提出架构结论，必须再检查真实 import/call graph（例如 CodeGraph）、边界所有权和运行时证据。

函数视图也刻意收窄：Rust `syn` 解析器识别自由函数、方法、trait 方法和闭包；其他语言会如实标为不支持，而不是用正则伪造函数。小于 512 字节的函数会标为小样本，因为压缩率方差很大。

## 固定数据中的观察

下表直接由已跟踪的生成数据库计算。项目总量按源码字节加权，不是把项目比率简单平均；清单、完整 commit、归档 SHA-256 和原始结果都在 [`benchmarks/`](benchmarks/)。

| Cohort | 入选方式 | 快照数 | 源码字节 | zstd-19 字节 | 加权压缩比 |
|---|---|---:|---:|---:|---:|
| 成熟 OSS | 公开项目的固定快照 | 23 | 267,962,913 | 23,089,904 | 0.086168 |
| AI 主导创作自披露 | 作者 README 直接披露 AI 主导/vibe-coded；每项至少 64 KiB 可识别源码 | 5 | 10,892,767 | 2,019,594 | 0.185407 |

第二组不应叫“低质量 cohort”。它的入选依据只是作者自己的披露；有些作者称项目为实验性或有 bug，另一些并没有。样本小、刻意选择，且语言和规模构成不同。其较高的聚合比率只是这些固定字节流的一个观察，不能证明低质量、一般意义的 AI 作者身份，也不能构成筛选仓库的阈值。

第一组同样不是“高质量真值集”。成熟度、测试、漏洞响应、维护、真实负载下的性能和运维历史，都是另外的证据。没有这些证据就把项目称为高质量或低质量，既不严谨，也无助于工程决策。

项目级记录进一步说明为何必须看原始数据：OSS 数据库中，固定 TypeScript 快照为 **0.047328**（143,059,923 源码字节），Catch2 为 **0.094877**（2,676,675 字节）；自披露 cohort 中，CodePrism 为 **0.132440**（4,164,594 字节），tauridraw 为 **0.228131**（5,303,367 字节）。这些不是排名，而是要求进一步检查语言、生成内容风险、重复模式与架构的理由。

## 把测量变成工程决策

把 Codensity 当作评审的第一页，而不是结论：

1. 记录不可变输入与账本字段：revision、协议、版本、字节数、比率、SHA-256、语言组成和排除项。
2. 分开检查画像信号；只审查具有足够规模的文件，并明确标出小函数，不给微小样本排序。
3. 用 `relation` 或 `compare` 找候选，再通过 imports、调用、共享类型、I/O 所有权和错误传播确认真实耦合。
4. 用测试、评审、安全检查和真实工作负载验证改动；前后重新测量只作为补充证据。

一份可靠的优化报告应明确区分 **事实**（记录的指标）、**推断**（有边界的假设）、**候选**（值得检查的改动）和 **不主张的内容**。仅凭压缩不能证明正确性、安全性、可维护性、性能、质量、AI 作者身份或耦合。

## 重建与更新数据库

Release 数据库是冻结账本的 schema-v1 产物。`database update` 只下载 `database-v1.json`，校验 GitHub Release 给出的 SHA-256 digest，再验证 schema 与协议，最后原子替换目标文件。若已在本地准备了固定源码快照，可这样重建：

```bash
cargo build --release --locked
workdir=/path/to/pinned-corpus # contains manifest.json and sources/<project>
(cd "$workdir" && /path/to/codensity database build manifest.json --output database.json)
```

采集规则与局限见 [`benchmarks/README.md`](benchmarks/README.md)；五个自披露记录见 [`benchmarks/real-ai-projects/README.md`](benchmarks/real-ai-projects/README.md)；每周的发布过程见 [数据库刷新工作流](.github/workflows/refresh-database.yml)。

## 范围

Codensity 使用 MIT 许可证。账本协议刻意保守：改变源码选择、排序、串接方式、zstd 参数或语言映射都会改变可比性，必须产生新协议，不能静默改写旧结果。
