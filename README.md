# rusy_analyzer

`rusy_analyzer` is a Rust-based static analysis toolkit for extracting fuzzing-relevant attack surfaces from Rust projects.

The current implementation completes the practical core of Module 1 by delivering:

- an AST-based interface extractor
- a MIR-based panic-site scanner

These two analyzers produce schema-validated JSON outputs that can be consumed by downstream tooling.

## Module 1 Status

Module 1 is the static analysis stage of the broader project roadmap.

Its purpose is to extract structured facts from Rust source code and MIR so later stages can reason about:

- externally reachable interfaces
- panic-oriented target sites
- future fuzzing-oriented routing and constraint information

At the current stage, Module 1 is partially completed and includes:

1. AST interface extraction
2. MIR panic-site extraction

Constraint slicing and final target stitching are planned next, but are not implemented in this repository yet.

## Implemented Components

### AST Interface Extractor

The AST pipeline scans Rust source code and reconstructs interface-routing facts from real syntax structures.

It currently focuses on RustSBI-style repositories and derives information from:

- specification modules and extension constants
- trait helper dispatch logic
- derive macro routing templates

The AST analyzer writes:

- `sbi_interfaces.json`

And validates the output contract against:

- `docs/json_spec/sbi_interfaces.schema.json`

### MIR Panic-Site Scanner

The MIR pipeline scans textual `-Z unpretty=mir` output and extracts panic-oriented basic blocks from function MIR.

This scanner is intentionally implemented with:

- pure text parsing
- explicit state machines
- regex-based semantic classification

It does not depend on unstable compiler internals such as `rustc_middle`.

The MIR analyzer currently classifies sites into three top-level families:

- `EA`: explicit assertion or explicit panic family
- `BV`: bounds violation family
- `AE`: arithmetic error family

It supports both:

- implicit panic sites represented by MIR `Assert`
- explicit panic-like sites represented by MIR `Call`

The MIR analyzer writes:

- `panic_sites.json`

And validates the output contract against:

- `docs/json_spec/panic_sites.schema.json`

## Repository Layout

```text
src/
  ast/          AST parsing and routing extraction
  mir/          MIR parsing, block classification, and matching rules
  models/       Public JSON-facing data models
  utils/        Shared filesystem and error helpers
  bin/          CLI entrypoints

docs/
  json_spec/    JSON schemas and output format specifications
```

## CLI Entrypoints

The repository currently provides two main binaries:

- `ast_parser`
- `panic_scanner`

### Run the AST analyzer

```bash
cargo run --bin ast_parser -- <path-to-target-repo> <output-json>
```

Typical output:

- `sbi_interfaces.json`

### Run the MIR analyzer

First generate textual MIR from the target project:

```bash
cargo rustc -- -Z unpretty=mir > sbi.unpretty.mir
```

Then run the scanner:

```bash
cargo run --bin panic_scanner sbi.unpretty.mir panic_sites.json
```

Typical output:

- `panic_sites.json`




