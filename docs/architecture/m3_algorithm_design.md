# M3 Algorithm Design

## Purpose

This document records the **current implemented M3 algorithm**, not an ideal future design.
It is meant to help the next optimization phase work from the real code path rather than from a high-level architecture summary.

The current M3 implementation lives mainly in:

- [src/stitch/mod.rs](/home/ssdns/code/rusy_analyzer/src/stitch/mod.rs)
- [src/stitch/loader.rs](/home/ssdns/code/rusy_analyzer/src/stitch/loader.rs)
- [src/stitch/router.rs](/home/ssdns/code/rusy_analyzer/src/stitch/router.rs)
- [src/stitch/slicer.rs](/home/ssdns/code/rusy_analyzer/src/stitch/slicer.rs)
- [src/stitch/trace.rs](/home/ssdns/code/rusy_analyzer/src/stitch/trace.rs)
- [src/mir/semantics.rs](/home/ssdns/code/rusy_analyzer/src/mir/semantics.rs)

## What The Current Algorithm Actually Does

M3 is currently a **degraded static stitcher** with two public outputs:

- `fuzz_targets.json`
- `local_constraints.json`

The top-level orchestration is in `stitch_reports()`:
[src/stitch/mod.rs](/home/ssdns/code/rusy_analyzer/src/stitch/mod.rs:42)

Runtime order:

1. load M1, M2, and MIR text
2. build shared MIR semantic graph
3. harvest route-independent local constraints for all panic sites
4. try to associate each panic site with an M1 dispatch route
5. for stitched sites, rerun slice enrichment with route context
6. emit `local_constraints.json`
7. emit `fuzz_targets.json`

This means the algorithm is already structurally split into:

- **Stage A**: local constraint harvest
- **Stage B**: route stitching and route-aware enrichment

## Phase 0: Input Loading And Dispatch Normalization

Code:
[src/stitch/loader.rs](/home/ssdns/code/rusy_analyzer/src/stitch/loader.rs:39)

The loader does three things:

1. deserialize `sbi_interfaces.json`
2. deserialize `panic_sites.json`
3. read raw MIR text

Then it derives a neutral `DispatchModel` from M1:
[src/stitch/loader.rs](/home/ssdns/code/rusy_analyzer/src/stitch/loader.rs:93)

Current normalization assumptions:

- selector layer 0 = `extension_id`
- selector layer 1 = `function_id`
- input slots = `parameter_registers`

This is still SBI-shaped in naming, although the data model itself is neutralized enough for future adapters.

## Shared MIR Semantic Graph

Code:
[src/mir/semantics.rs](/home/ssdns/code/rusy_analyzer/src/mir/semantics.rs:191)

M3 does **not** slice directly on raw MIR text.
It consumes the shared MIR semantic graph built by `build_semantic_graph()`.

Important graph content:

- functions
- blocks
- forward call graph
- reverse call graph
- per-block successors and predecessors
- parsed `DefSite`
- inline scope hints
- panic-oriented `BlockEvidence`
- `PanicSeed`

Important shared types:

- `DefSite`
- `BlockGraph`
- `FunctionGraph`
- `MirGraph`
- `PanicSeed`

The stitch layer imports these through:
[src/stitch/mir_graph.rs](/home/ssdns/code/rusy_analyzer/src/stitch/mir_graph.rs:1)

## Stage A: Local Constraint Harvest

Entry point:
[src/stitch/slicer.rs](/home/ssdns/code/rusy_analyzer/src/stitch/slicer.rs:68)

For each `PanicSite`, M3 does:

1. derive a stable key: `function:bb`
2. ask shared MIR for a `PanicSeed`
3. if a seed exists, run `slice_from_seed(..., None, None)`
4. otherwise emit an `unresolved` bundle

Internal output type:
[LocalConstraintCandidate](/home/ssdns/code/rusy_analyzer/src/stitch/slicer.rs:41)

The harvest stage is route-independent by construction:

- no M1 route is required
- no external register mapping is required
- no top-level entry provenance is required

### A.1 Panic Seed Recovery

Seed lookup happens via:
`panic_seed_for_site(...)`

Call sites:

- [src/stitch/slicer.rs](/home/ssdns/code/rusy_analyzer/src/stitch/slicer.rs:83)
- [src/stitch/slicer.rs](/home/ssdns/code/rusy_analyzer/src/stitch/slicer.rs:158)

Recovered seed contains:

- `expression`
- `locals`
- `evidence`

Current role of the seed:

- it defines the slicing start expression
- it seeds the work queue with relevant MIR locals
- it contributes the initial `panic_guard` clause

### A.2 Backward Slice Core Loop

Core function:
[src/stitch/slicer.rs](/home/ssdns/code/rusy_analyzer/src/stitch/slicer.rs:183)

The algorithm is a queue-based degraded backward slice:

1. initialize queue from `seed.locals`
2. pop one `(function, bb, local, depth)` item
3. stop if depth reaches `MAX_SLICE_DEPTH = 50`
4. deduplicate visits by `function:bb:local`
5. resolve the local definition in current block or predecessor blocks
6. turn the resolved def into:
   - substitutions
   - external input refs
   - new queue items
   - path guard clauses
   - stop reasons / warnings

Important limits:

- `MAX_SLICE_DEPTH = 50`
- `MAX_PREDECESSOR_SEARCH_DEPTH = 20`
- `EXPR_BLOAT_THRESHOLD = 500`

These are in:
[src/stitch/slicer.rs](/home/ssdns/code/rusy_analyzer/src/stitch/slicer.rs:21)

### A.3 Definition Resolution Strategy

Definition lookup is in:
[src/stitch/slicer.rs](/home/ssdns/code/rusy_analyzer/src/stitch/slicer.rs:557)

Current strategy:

1. look for a block-local def first
2. if not found, BFS predecessors
3. accumulate predecessor edge conditions while walking
4. return the first matching def found

This is intentionally simple.
It does not try to compute full SSA form, join semantics, or alias-aware memory state.

### A.4 Supported DefSite Semantics

The main `match` over `DefSite` starts here:
[src/stitch/slicer.rs](/home/ssdns/code/rusy_analyzer/src/stitch/slicer.rs:316)

Current handled families:

- `Param`
- `CopyMove`
- `ArrayAccess`
- `BinOp`
- `UnaryOp`
- `Const`
- `Cast`
- `Discriminant`
- `FieldAccess`
- `Call`
- `Unsupported`

Semantic behavior today:

- `Param`: try to map to external inputs, or mark unknown
- `CopyMove`: substitute source local and continue slicing
- `ArrayAccess`: special-case array-like parameter mapping, else keep slicing source
- `BinOp` / `UnaryOp`: render readable algebraic expression and enqueue operands
- `Const`: record as constant substitution
- `Cast` / `FieldAccess` / `Discriminant`: degrade to a readable derived expression and continue
- `Call`: treat as opaque boundary, add `opaque_boundary` clause, stop precise recovery
- `Unsupported`: mark unknown and degrade

### A.5 Path Guards And Clause Assembly

Two sources of `path_guard` clauses exist:

1. predecessor search conditions during local slice
2. route-level conditions after a route is found

The local path-guard insertion happens here:
[src/stitch/slicer.rs](/home/ssdns/code/rusy_analyzer/src/stitch/slicer.rs:296)

The route-aware path-guard insertion happens here:
[src/stitch/slicer.rs](/home/ssdns/code/rusy_analyzer/src/stitch/slicer.rs:203)

The panic guard clause is inserted here:
[src/stitch/slicer.rs](/home/ssdns/code/rusy_analyzer/src/stitch/slicer.rs:231)

### A.6 Expression Rendering And Degradation

After substitutions are collected, M3 renders a canonical expression by:

1. taking path guards + panic guard clauses
2. rendering the seed expression through substitutions
3. deduplicating and joining with `&&`
4. normalizing text
5. filtering unsupported MIR noise
6. downgrading on expression bloat

This assembly starts here:
[src/stitch/slicer.rs](/home/ssdns/code/rusy_analyzer/src/stitch/slicer.rs:483)

Current degradation policy:

- expression empty => `unresolved`
- depth limit => `truncated`
- no route context => usually `partial`
- route present but imperfect evidence => `partial`
- exact route + no warnings + no stop reasons => `complete`

Status selection happens here:
[src/stitch/slicer.rs](/home/ssdns/code/rusy_analyzer/src/stitch/slicer.rs:523)

Important current property:

- the top-level expression tries to stay algebraically readable
- unsupported MIR noise triggers downgrade instead of pretending to be precise

## Stage B: Route Stitching

Entry point:
[src/stitch/router.rs](/home/ssdns/code/rusy_analyzer/src/stitch/router.rs:60)

The router only answers:

- can this panic site be credibly associated with an M1 dispatch route?

It does **not** build the main constraint body.

### B.1 Helper Lookup

M1 is converted into:

- `dispatcher_helper -> [(extension, function), ...]`

Code:
[src/stitch/router.rs](/home/ssdns/code/rusy_analyzer/src/stitch/router.rs:153)

This is important because routing is helper-driven, not hardcoded on RustSBI names inside M3 logic.

### B.2 Reverse BFS From Panic Owner

Core search:
[src/stitch/router.rs](/home/ssdns/code/rusy_analyzer/src/stitch/router.rs:168)

Current route discovery algorithm:

1. start from panic owner function
2. create initial trace step `(panic_owner, bb)`
3. BFS upward along `reverse_call_graph`
4. at each function:
   - check whether function name itself is a known dispatcher helper
   - check inline scope hints for known helpers
   - collect caller edges and continue BFS
5. accumulate route candidates
6. score candidates
7. pick best non-ambiguous candidate

Depth bound:

- `MAX_ROUTE_DEPTH = 50`

### B.3 Candidate Scoring

Scoring logic:
[src/stitch/router.rs](/home/ssdns/code/rusy_analyzer/src/stitch/router.rs:260)

For each candidate binding:

- check whether nearby path conditions contain matching `eid`
- check whether nearby path conditions contain matching `fid`
- reward unique helper-to-binding mapping
- allow degraded helper-only hit when helper is unique
- reject ambiguous equal-score different routes

Scoring intent today:

- prefer exact selector evidence
- allow degraded but still credible helper-level association
- avoid `first()`-style accidental routing

### B.4 Selector Evidence Source

Selector evidence comes from path conditions collected around the context block:
[src/stitch/router.rs](/home/ssdns/code/rusy_analyzer/src/stitch/router.rs:244)

The actual equality matching is string-driven:

- split condition on `==`
- ignore `otherwise`
- parse values as hex or decimal

Relevant code:

- [src/stitch/router.rs](/home/ssdns/code/rusy_analyzer/src/stitch/router.rs:325)
- [src/stitch/router.rs](/home/ssdns/code/rusy_analyzer/src/stitch/router.rs:348)

This is one of the current weak points and a likely optimization target.

## Route-Aware Enrichment

Once a route exists, M3 does not reuse the local constraint bundle blindly.
It reruns `slice_from_seed()` with:

- `Some(route_match)`
- `Some(interface_report)`

Call site:
[src/stitch/slicer.rs](/home/ssdns/code/rusy_analyzer/src/stitch/slicer.rs:136)

This adds two route-aware capabilities:

1. inject route path guards into clauses
2. try to map parameters back to external registers through caller arguments

### Parameter Mapping

Current parameter mapping helpers:

- `map_direct_param()`
- `map_array_access()`
- `map_param_through_route()`

Code:

- [src/stitch/slicer.rs](/home/ssdns/code/rusy_analyzer/src/stitch/slicer.rs:648)
- [src/stitch/slicer.rs](/home/ssdns/code/rusy_analyzer/src/stitch/slicer.rs:696)
- [src/stitch/slicer.rs](/home/ssdns/code/rusy_analyzer/src/stitch/slicer.rs:746)

Current mapping policy:

- if parameter index directly matches M1 input slots, map to register
- if route trace exposes a caller callsite, parse actual arguments and continue slicing upward
- if inline frame blocks precise mapping, degrade instead of inventing one

## Weak Route Hints For Local-Only Candidates

Even when no formal route is found, M3 still projects weak hints into `local_constraints.json`.

Entry point:
[src/stitch/router.rs](/home/ssdns/code/rusy_analyzer/src/stitch/router.rs:79)

Current hint families:

- `inline_helper`
- `selector_guard`
- `caller_tail`

These hints are intentionally non-authoritative.
They are a side channel for later optimization and dictionary projection, not a substitute for `RouteBinding`.

## Call Trace

Trace rendering is intentionally lightweight.

The router builds `RouteTraceStep`.
The trace module converts that into public `CallTraceFrame`.

Current integration point:
[src/stitch/slicer.rs](/home/ssdns/code/rusy_analyzer/src/stitch/slicer.rs:129)

Important rule:

- trace is explanation-only
- trace does not prove route correctness
- route does not depend on trace rendering success

## Public Output Projection

Top-level projection is in:
[src/stitch/mod.rs](/home/ssdns/code/rusy_analyzer/src/stitch/mod.rs:42)

Current projection policy:

- every panic site gets one local candidate record in `local_constraints.json`
- only routed sites with both `RouteMatch` and harvested local candidate become stitched targets
- missing harvest data degrades to unresolved local candidate instead of disappearing

The local-candidate projection helper is:
[src/stitch/mod.rs](/home/ssdns/code/rusy_analyzer/src/stitch/mod.rs:167)

## Current Defensive Design Choices

The present algorithm intentionally prioritizes:

- zero silent failure
- bounded traversal
- human-readable expressions
- structured degradation over false precision

So when M3 encounters:

- missing defs
- opaque calls
- unsupported statements
- ambiguous routes
- missing entry mapping
- cycle-like revisits

it emits:

- `partial`
- `truncated`
- `unresolved`
- warnings
- stop reasons

instead of pretending to have complete semantics.

## Known Current Limitations

These are implementation realities, not theoretical goals:

1. route matching is still selector-string-driven, not a normalized symbolic predicate engine
2. predecessor search returns first workable def, not a full join-aware data-flow lattice
3. memory and alias semantics are mostly degraded away
4. opaque calls are not summarized interprocedurally
5. route-aware parameter recovery depends on visible caller callsites and simple argument parsing
6. dispatch normalization is still effectively SBI-first, even though the surface schema is neutralized
7. local candidate count and stitched target count are intentionally overlapping sets, not disjoint sets

## Best Optimization Targets For Next Phase

If the next phase is optimization rather than redesign-from-scratch, the highest-leverage areas are:

1. normalize selector/path condition reasoning so routing stops depending on raw string equality patterns
2. improve `panic_seed_for_site()` quality so harvest starts from cleaner seed expressions
3. upgrade predecessor def resolution from first-hit BFS to path-aware merge logic
4. add lightweight summaries for known helper calls instead of treating all calls as opaque
5. improve caller-argument lifting across more inline and wrapper patterns
6. split “candidate count” and “panic-site coverage” statistics more explicitly in summaries

## Relationship To The High-Level Architecture Doc

Use:

- [m3_architecture.md](/home/ssdns/code/rusy_analyzer/docs/architecture/m3_architecture.md)
  for ownership, boundaries, public contract, and stage semantics

Use this file:

- for the **actual implemented algorithm**
- for optimization planning
- for locating the exact code entry points that would need to change
