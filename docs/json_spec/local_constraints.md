# `local_constraints.json` Output Specification

This file documents the auxiliary M3 artifact emitted by
[`src/bin/fuzz_stitcher.rs`](/home/ssdns/code/rusy_analyzer/src/bin/fuzz_stitcher.rs).

The current M3 pipeline consumes:

- `sbi_interfaces.json`
- `panic_sites.json`
- `sbi.mir`

and produces:

- `fuzz_targets.json`
- `local_constraints.json`

`local_constraints.json` is the harvest-stage artifact. It contains route-independent
local constraint candidates for every panic site M3 analyzed, including degraded
`partial`, `truncated`, and `unresolved` cases.

It is intentionally separate from `fuzz_targets.json`:

- `fuzz_targets.json` is for stitched directed-fuzz targets
- `local_constraints.json` is for dictionary projection, seed enrichment, and future solver preprocessing

## Top-Level Structure

The file is a JSON object with these required fields:

| Field | Type | Meaning |
| --- | --- | --- |
| `schema_version` | `string` | Schema version currently emitted by the local-constraint artifact. Initial value: `1.0.0`. |
| `target_repository` | `string` | Path to the analyzed repository root. |
| `dispatch_model` | `DispatchModel` | Same normalized dispatch model used by `fuzz_targets.json`. |
| `input_artifacts` | `InputArtifacts` | Provenance of the M1 report, M2 report, and MIR text. |
| `summary` | `LocalConstraintSummary` | Aggregate counts for all harvested candidates. |
| `candidates` | `LocalConstraintCandidate[]` | Route-independent local constraint candidates. |

`DispatchModel` and `InputArtifacts` are intentionally identical to the shapes used by `fuzz_targets.json`.

## `LocalConstraintSummary`

| Field | Type | Meaning |
| --- | --- | --- |
| `total_candidates` | `integer` | Total number of candidate records emitted into `candidates`. |
| `statuses.partial` | `integer` | Candidates with a useful but incomplete local expression. |
| `statuses.truncated` | `integer` | Candidates cut short by depth or defensive limits. |
| `statuses.unresolved` | `integer` | Candidates whose local constraint could not be reconstructed. |
| `candidates_with_seed_context` | `integer` | Number of candidates with a non-null `seed_context`. |
| `candidates_with_nonempty_expression` | `integer` | Number of candidates whose `constraint.expression` is non-empty. |

## `LocalConstraintCandidate`

Each element in `candidates` is an object with these fields:

| Field | Type | Meaning |
| --- | --- | --- |
| `candidate_id` | `string` | Stable logical identifier such as `LC_0001`. |
| `panic_site` | `PanicSite` | Full embedded M2 panic site object. |
| `seed_context` | `SeedContext \| null` | Recovered local seed that started the harvest, or `null` if no seed was found. |
| `constraint` | `ConstraintBundle` | Route-independent local constraint bundle. Same data model as stitched targets use. |
| `route_hints` | `RouteHint[]?` | Optional weak hints that may help later stitching or dictionary projection. |
| `warnings` | `string[]` | Candidate-level warnings that must not be silently dropped. |

## `SeedContext`

| Field | Type | Meaning |
| --- | --- | --- |
| `seed_expression` | `string` | Human-readable local expression recovered from panic evidence. |
| `seed_locals` | `string[]` | MIR locals directly referenced by `seed_expression`. |
| `evidence` | `string` | MIR evidence snippet that yielded the seed. |
| `source_function` | `string` | Function that owns the seed. |
| `source_basic_block` | `string` | Basic block that owns the seed. |

## `RouteHint`

`route_hints` are intentionally weak and non-authoritative. They are allowed to help
downstream tooling prioritize dictionary projection or later stitching work, but they
must never be confused with a real `route` binding.

| Field | Type | Meaning |
| --- | --- | --- |
| `hint_kind` | `string` | One of `inline_helper`, `selector_guard`, `caller_tail`. |
| `value` | `string` | Human-readable hint payload. |
| `source_function` | `string?` | Function context where the hint was observed. |
| `source_basic_block` | `string?` | Basic block where the hint was observed. |
| `evidence` | `string?` | Short explanation for why this hint exists. |

## `ConstraintBundle`

`constraint` reuses the exact same shape and formatting rules as the stitched
`ConstraintBundle` in `fuzz_targets.json`:

- `status`
- `format`
- `expression`
- `clauses`
- `external_inputs`
- `substitutions`
- `slice_depth`
- `stop_reasons`
- `warnings`

The same algebraic-purity rules apply:

- `expression` should remain readable C/math-like text
- raw MIR noise such as `otherwise`, allocator internals, or opaque call dumps must not become the main expression
- unresolved or unsupported cases must be downgraded structurally instead of being silently dropped

## Why This Artifact Exists

Local constraints are useful even when M3 cannot prove a full dispatch route.
They can still surface:

- magic constants
- local inequalities
- panic-adjacent value relations

Those fragments are valuable for later modules, but they are not formal stitched targets
because they do not yet guarantee:

- top-level dispatch provenance
- selector identity
- reliable external input binding

That is why this artifact is separate from `fuzz_targets.json` rather than mixed into its `targets` array.
