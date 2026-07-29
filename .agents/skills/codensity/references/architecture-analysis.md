# Codensity architecture analysis reference

## What the metric does and does not establish

For a source stream, define:

```text
ratio = compressed_bytes / original_bytes
savings = 1 - ratio
```

A lower ratio indicates more repetition or regularity under the fixed Codensity
protocol. It may reflect vocabulary, formatting, generated code, boilerplate,
test fixtures, framework conventions, file size, or language design. It does
not establish code quality, maintainability, safety, defect rate, authorship,
or AI use.

Never compare different language mixes or substantially different corpus sizes
as though they were a controlled experiment. Prefer language-specific,
size-aware medians and quartiles; report byte-weighted aggregates separately.

## Cross-module regularity

Measure each selected module as its own stream, then measure their union:

```text
independent_sum = sum(module.compressed_bytes)
whole = union.compressed_bytes
cross_module_gain = independent_sum - whole
cross_module_gain_percent = 1 - whole / independent_sum
```

The gain indicates that one shared compression stream reused patterns across
module boundaries. It is not a coupling score: independent modules can share
idioms, generated headers, error text, identifiers, or test structure. It also
includes a small reduction in repeated zstd frame overhead, so interpret small
gains cautiously.

Use the metric to decide where to inspect duplicated vocabulary or templates,
not to justify a refactor by itself.

## Coupling review

Build a directed module graph from current imports and calls. Record for each
module:

- outgoing dependencies and incoming dependents;
- public API boundary versus private helper use;
- ownership of filesystem, network, serialization, or mutable state;
- error and data types shared with other modules;
- cycles, layer violations, and dependency direction.

Treat a module as tightly coupled when it needs another module's private
implementation detail or simultaneously owns multiple unrelated infrastructure
concerns. Treat a facade that intentionally re-exports stable public API as a
boundary, not automatically as harmful coupling.

## Cohesion review

State one responsibility for each module. Flag a module when it combines
unrelated responsibilities, for example network transport, release metadata
parsing, persistence, and user-facing presentation. Prefer extracting a shared
infrastructure boundary only when two or more modules genuinely need the same
semantics; avoid splitting merely to reduce line count.

## Report template

1. **Evidence:** protocol/version, commands, corpus sizes, ratios, and graph
   facts.
2. **Interpretation:** clearly labeled inferences about repeated structure,
   dependency direction, and responsibilities.
3. **Risks:** sample bias, small-corpus variance, stale indexes, or unavailable
   pinned inputs.
4. **Priorities:** one to three changes ranked by leverage, each tied to a
   concrete graph or ownership finding.
