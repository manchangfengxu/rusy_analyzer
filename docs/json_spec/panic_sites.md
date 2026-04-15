# `panic_sites.json` Output Specification

This directory records the output contract for the `panic_scanner` binary in [`src/bin/panic_scanner.rs`](/home/ssdns/code/rusy_analyzer/src/bin/panic_scanner.rs).

The scanner reads a MIR text file, by default `sbi.mir`, and writes:

- `panic_sites.json`

## Purpose

This file describes panic-oriented MIR targets extracted from a Rust project's `unpretty MIR` text.
It is intended for downstream tooling that needs to:

- enumerate panic-relevant basic blocks
- know which Rust function and MIR basic block contain the panic site
- distinguish high-level categories `EA`, `BV`, and `AE`
- retain the concrete textual evidence that caused the site to be classified
- consume one schema that works for ordinary Rust projects, not only RustSBI

## Category Contract

The JSON contract always uses the following three macro categories:

- `EA`: explicit panic path or unwrap/expect failure
- `BV`: bounds violation
- `AE`: arithmetic error, including overflow and division-style failures

## Top-Level Structure

The file is a JSON object with these required fields:

| Field | Type | Meaning |
| --- | --- | --- |
| `schema_version` | `string` | Schema version currently emitted by the scanner. |
| `mir_source` | `string` | Input MIR file path used for this scan. |
| `summary` | `PanicSummary` | Aggregate counts over all extracted sites. |
| `sites` | `PanicSite[]` | Extracted MIR panic targets. |

## `PanicSummary`

| Field | Type | Meaning |
| --- | --- | --- |
| `total_sites` | `integer` | Total number of extracted sites. |
| `categories.EA` | `integer` | Number of `EA` sites. |
| `categories.BV` | `integer` | Number of `BV` sites. |
| `categories.AE` | `integer` | Number of `AE` sites. |

## `PanicSite`

Each element in `sites` is an object with these required fields:

| Field | Type | Meaning |
| --- | --- | --- |
| `function` | `string` | MIR function name, parsed from the `fn ...` header. |
| `basic_block` | `string` | MIR basic block label, for example `bb7`. |
| `is_cleanup` | `boolean` | Whether the basic block was emitted as `bbN (cleanup): {`. |
| `category` | `string` | One of `EA`, `BV`, `AE`. |
| `panic_kind` | `string` | Concrete subtype of panic classification. The same semantic kind can appear in either `assert` or `call` MIR shape. |
| `panic_origin` | `string` | Whether the site came from an implicit MIR `Assert` or an explicit `Call` path. |
| `evidence_type` | `string` | How the scanner found the site. Current values: `assert`, `call`. |
| `evidence` | `string` | Short normalized evidence string matched from the block text. |
| `line_start` | `integer` | 1-based line number where the basic block started. |
| `line_end` | `integer` | 1-based line number where the basic block ended. |

## Current `panic_kind` Values

| `category` | `panic_kind` | Meaning |
| --- | --- | --- |
| `EA` | `explicit_panic` | Block contains a call path such as `core::panicking::panic`, `panic_fmt`, or similar explicit panic helpers. |
| `EA` | `assert_failed` | Block contains an `assert_failed(...)` helper call, typically lowered from `assert!()`. |
| `EA` | `assert_failed_eq` | Block contains an `assert_failed_eq(...)` helper call, typically lowered from `assert_eq!()`. |
| `EA` | `assert_failed_ne` | Block contains an `assert_failed_ne(...)` helper call, typically lowered from `assert_ne!()`. |
| `EA` | `unwrap_failed` | Block contains unwrap/expect-style panic evidence. |
| `BV` | `index_out_of_bounds` | Block contains bounds-check panic text. |
| `BV` | `not_a_char_boundary` | Block contains UTF-8 char-boundary panic text. |
| `AE` | `add_overflow` | Block contains add overflow panic text. |
| `AE` | `subtract_overflow` | Block contains subtract overflow panic text. |
| `AE` | `multiply_overflow` | Block contains multiply overflow panic text. |
| `AE` | `neg_overflow` | Block contains unary negation overflow panic text. |
| `AE` | `shift_left_overflow` | Block contains left-shift overflow panic text. |
| `AE` | `shift_right_overflow` | Block contains right-shift overflow panic text. |
| `AE` | `divide_by_zero` | Block contains divide-by-zero panic text. |
| `AE` | `remainder_by_zero` | Block contains remainder-by-zero panic text. |
| `EA` | `resumed_after_return` | Block contains a resumed-after-completion coroutine or async-fn panic. |
| `EA` | `resumed_after_panic` | Block contains a resumed-after-panic coroutine panic. |
| `EA` | `misaligned_pointer_dereference` | Block contains a misaligned pointer dereference panic. |
| `EA` | `null_pointer_dereference` | Block contains a null pointer dereference panic. |
| `EA` | `invalid_enum_construction` | Block contains invalid enum construction panic text. |
| `EA` | `unknown_assert` | Block contains an implicit MIR `Assert` whose message is not yet mapped to a known family. |
| `EA` | `unknown_explicit_call` | Block contains a panic-like MIR `Call` whose helper path is not yet mapped to a known family. |

## Support Matrix

| Family | Current strategy |
| --- | --- |
| Known implicit `Assert` text families | Emit a fully classified site with `panic_origin = implicit_assert`. |
| Unknown implicit `Assert` text | Emit a site as `EA / unknown_assert`, preserve evidence, and also print a warning. |
| Known explicit panic helpers | Emit a fully classified site with `panic_origin = explicit_call`. |
| Unknown panic-like `Call` helpers | Emit a site as `EA / unknown_explicit_call`, preserve the helper evidence, and also print a warning. |

## Supported Explicit Panic Helpers

The current explicit helper matcher is intentionally split by helper family instead of only checking whether a path contains the word `panic`. It includes patterns such as:

- `core::panicking::panic*`
- `std::panicking::panic*`
- `panic_fmt`
- `panic_display`
- `panic_nounwind`
- `panic_bounds_check`
- `assert_failed`
- `assert_failed_eq`
- `assert_failed_ne`
- `begin_panic`
- `core::result::unwrap_failed`
- `core::option::unwrap_failed`
- `core::result::expect_failed`
- `core::option::expect_failed`

`panic_bounds_check` is a key cross-shape case: even though it is a MIR `Call`, it still classifies as `BV / index_out_of_bounds`.
Likewise, `unwrap_failed` may appear as either an MIR `Assert` message or an MIR `Call`, so consumers must read `panic_origin` and `evidence_type` instead of assuming the shape from `panic_kind` alone.

## Parsing Rules For Consumers

- Treat every field above as required.
- Treat `evidence` as a normalized text fragment, not as a stable Rust AST.
- Treat `function` and `basic_block` together as the primary logical identity for one site.
- Treat `panic_origin` as the scanner's best statement about whether the edge came from an MIR `Assert` terminator or a panic helper `Call`.
- Do not assume every MIR `assert(...)` block becomes a known family.
  Unrecognized `assert` forms now become `unknown_assert` sites and also produce stderr warnings.
- Do not assume every panic-like MIR `Call` is already mapped to a concrete helper family.
  Unknown panic-like calls now become `unknown_explicit_call` sites and also produce stderr warnings.
- Do not assume `sites` is deduplicated across multiple MIR files.
  The scanner currently processes one MIR file per run.
- Do not assume top-level `const ... = {}` MIR is scanned.
  The current scanner only walks function MIR bodies that start with `fn ... {`.
  This means some divide-by-zero or remainder-by-zero candidates that only appear in constant MIR
  will not be emitted, which is one real reason the `AE` subkind counts for divide/remainder cases
  can remain `0` on current RustSBI
  inputs.

## Warning Behavior

The scanner prints warnings to stderr when it sees malformed MIR structure, an `assert` block whose panic message is not recognized, or a panic-like `Call` helper that is not yet mapped.
Those warnings are not serialized into `panic_sites.json`.

## Example

```json
{
  "schema_version": "2.0.0",
  "mir_source": "sbi.unpretty.mir",
  "summary": {
    "total_sites": 2,
    "categories": {
      "EA": 1,
      "BV": 1,
      "AE": 0
    }
  },
  "sites": [
    {
      "function": "demo::bounds",
      "basic_block": "bb0",
      "is_cleanup": false,
      "category": "BV",
      "panic_kind": "index_out_of_bounds",
      "panic_origin": "implicit_assert",
      "evidence_type": "assert",
      "evidence": "index out of bounds",
      "line_start": 2,
      "line_end": 4
    }
  ]
}
```

## Machine-Readable Schema

For validation in scripts or CI, use:

- [`panic_sites.schema.json`](/home/ssdns/code/rusy_analyzer/docs/json_spec/panic_sites.schema.json)
