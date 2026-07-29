# Code Quality Evidence Matrix

## Core Rule

Treat code quality as fitness for explicit requirements under realistic change
and failure conditions. Do not turn source regularity, compression ratio,
information-profile score, low entropy, or lack of duplication into a quality
verdict. Those measurements can prioritize inspection; they cannot establish
correctness, security, maintainability, or performance.

## Review in This Order

| Dimension | Evidence for stronger quality | Evidence for weaker quality | Codensity role |
|---|---|---|---|
| Correctness | Explicit invariants, deterministic tests, boundary/error tests, reproducible failures fixed by tests | Missing behavior tests, flaky tests, silent failure, untested boundary changes | None; use file/function data only to find where tests and implementation are concentrated |
| Security | Threat model, least privilege, input validation, safe resource handling, dependency review, negative tests | Unbounded input/resource use, unsafe parsing, secret exposure, broad privileges, unauthenticated trust boundaries | Flag unusual high-entropy or generated content for review; never infer security from ratio |
| Reliability | Typed failures, atomic writes, cleanup on failure, retry/timeouts where required, observable errors | Partial state after failure, lost errors, nondeterminism, hidden side effects | Use source concentration to identify failure-prone central modules |
| Maintainability | Cohesive responsibilities, explicit boundaries, stable APIs, readable names, low surprise, focused tests | Mixed unrelated ownership, implicit contracts, cycles, duplicated policy, difficult local change | Combine CodeGraph dependency facts with duplicate-pattern candidates |
| Performance | Representative benchmark, defined workload, latency/throughput/memory limits, regression threshold | Anecdotal speed claims, microbenchmark-only conclusions, uncontrolled inputs | Compression measures source regularity, not runtime cost |
| Operability | Reproducible build, safe upgrade/rollback, logs/metrics, clear configuration, documented failure modes | Manual-only deployment, opaque state, unrecoverable migration, no diagnostics | Use database/release provenance as reproducibility evidence only |

## Use Codensity as an Optimization Funnel

1. Measure the repository and record immutable protocol evidence.
2. Identify large files, concentrated ownership, duplicate windows, and
   non-small function candidates.
3. Trace real imports and calls with CodeGraph.
4. Form one falsifiable hypothesis, such as “two serializers duplicate the same
   validation policy” or “a large dispatcher owns unrelated transport and
   presentation concerns.”
5. Validate the hypothesis with tests, benchmarks, security review, or runtime
   traces before changing code.
6. Re-measure after a change, but accept the change only when the requested
   behavioral, safety, maintainability, or performance criterion improves.

## Interpret Common Signals Carefully

| Signal | Permitted inference | Forbidden inference |
|---|---|---|
| Low repository ratio | The source has more reusable byte patterns under the pinned protocol | The code is better, simpler, safer, human-written, or AI-written |
| High repository ratio | The source has fewer reusable byte patterns under the pinned protocol | The code is novel, higher quality, more complex, or worse |
| High file-pair gain | Inspect shared text, templates, naming, or duplicated logic | The files are structurally coupled or must be merged |
| High function-pair gain | Inspect exact parser-backed spans for shared implementation patterns | The functions are semantically equivalent or plagiarized |
| High score / low template risk | The profile's bounded byte-level components are favorable | The repository passes a quality gate |
| Small function ratio | The result is dominated by frame overhead and has high variance | The function is especially dense or inefficient |

## Report Template

1. **Facts:** command, commit or working-tree state, schema/protocol/version,
   corpus size, language mix, and exact measurements.
2. **Graph facts:** imports, callers, public boundaries, shared state, I/O,
   error paths, and cycles if observed.
3. **Bounded inferences:** repeated byte patterns or concentrated code that
   warrant inspection, with sample-size and variance caveats.
4. **Quality evidence:** correctness, security, reliability, maintainability,
   performance, and operability evidence that is present or missing.
5. **Optimization candidates:** small, falsifiable changes with acceptance
   tests or benchmarks. State “no change justified” when evidence is absent.
