# M2 Architecture

## Goal

M2 is the MIR panic-site scanner. Its public responsibility is still narrow:

- read `-Z unpretty=mir` text
- classify panic-oriented basic blocks
- emit stable `panic_sites.json`

M2 does **not** perform route association, interprocedural stitching, external input mapping, or final fuzz-target construction.

## Layering

The implementation now has three layers:

1. `src/mir/parser.rs`
   - structural MIR scanner
   - finds functions and `bbN` blocks
   - preserves line ranges and cleanup metadata
2. `src/mir/semantics.rs`
   - shared MIR semantic layer used by both M2 and M3
   - builds function/block context, CFG edges, def-use hints, inline hints, and block evidence
   - exposes panic-seed-style local evidence without imposing M2 or M3 policy
3. `src/mir/classifier.rs`
   - M2-only taxonomy layer
   - consumes the shared semantic graph and emits `PanicSite`

`src/mir/mod.rs` is the façade that composes the shared semantic graph and the classifier into the public `scan_mir_text()` / `scan_mir_file()` API.

## Shared MIR Inputs Consumed By M2

M2 now relies on the shared layer for:

- continuation-line joining
- parsed `assert(...)`, `call(...)`, `switchInt(...)`, `goto`
- block metadata: `is_cleanup`, `start_line`, `end_line`
- function metadata: params, debug bindings, inline hints
- block evidence:
  - parsed assert condition
  - panic-like snippet
  - local panic seed candidate

These richer contexts stay internal to the crate in this round. They improve classification quality, but they do not change the external `panic_sites.json` schema.

## Public Contract Boundary

The M2 public JSON contract remains unchanged:

- no new fields in `PanicSite`
- no schema version bump
- no embedding of seed locals or assert conditions in `panic_sites.json`

This keeps M2 stable for downstream consumers while still allowing M3 to reuse the same semantic inputs.

That downstream reuse now feeds both M3 artifacts:

- stitched `fuzz_targets.json`
- auxiliary `local_constraints.json`

## Relationship To M3

M2 and M3 now share the same MIR semantic substrate, but they diverge immediately after that:

- M2 asks: “what panic site is this block?”
- M3 asks: “what local constraint does this panic imply, and can it be stitched to a dispatch route?”

This is the key boundary:

- shared layer owns MIR syntax and generic block/function semantics
- M2 owns panic taxonomy
- M3 owns route stitching and constraint assembly

## Why Seed Context Stays Internal

The shared layer now knows concepts like:

- assert condition
- seed locals
- local panic seed candidate

They are intentionally **not** serialized into `panic_sites.json` yet because:

- M2’s public purpose is enumeration and classification, not slicing
- exposing them would require a versioned schema expansion
- the current round prioritizes shared implementation reuse over public contract churn

## Current Data Flow

`panic_scanner` runs this pipeline:

1. read MIR text
2. build `MirSemanticGraph`
3. classify semantic blocks into `PanicSite`
4. aggregate `PanicSummary`
5. write `panic_sites.json`

Warnings for malformed MIR, unknown assert families, and unknown panic-like calls are still emitted to stderr and are not serialized into the JSON report.
