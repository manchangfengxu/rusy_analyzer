# `sbi_interfaces.json` Output Specification

This directory records the output contract for the `ast_parser` binary in [`src/bin/ast_parser.rs`](/home/ssdns/code/rusy_analyzer/src/bin/ast_parser.rs).

The current parser writes one JSON file at the repository root:

- `sbi_interfaces.json`

## Purpose

This file describes the SBI extension attack surface currently extracted from RustSBI source code.
It is intended for downstream tooling that needs to:

- enumerate implemented SBI extensions
- map extension IDs to function IDs
- understand which supervisor-call registers (`a0` to `a5`) are consumed by each function
- inspect the parser's best-effort argument expressions
- understand which spec modules were skipped or only partially implemented via stderr warnings

## Top-Level Structure

The file is a JSON object with the following required fields:

| Field | Type | Meaning |
| --- | --- | --- |
| `target_repository` | `string` | Path to the analyzed RustSBI checkout. |
| `extension_register` | `string` | Register that carries the SBI extension ID. Current value: `a7`. |
| `function_register` | `string` | Register that carries the SBI function ID. Current value: `a6`. |
| `parameter_registers` | `string[6]` | Ordered parameter registers used by SBI calls. Current order: `["a0","a1","a2","a3","a4","a5"]`. |
| `extensions` | `ExtensionInterface[]` | All extensions that were both recognized in `sbi-spec` and matched to a RustSBI dispatcher helper. |

## `ExtensionInterface`

Each element in `extensions` is an object with these required fields:

| Field | Type | Meaning |
| --- | --- | --- |
| `module` | `string` | RustSBI extension module name, usually matching `library/sbi-spec/src/<module>.rs`. |
| `extension_name` | `string` | Uppercase extension name derived from the EID constant, for example `TIME`. |
| `eid_constant` | `string` | Constant identifier from `sbi-spec`, for example `EID_TIME`. |
| `eid_value_hex` | `string` | Hexadecimal EID value string with `0x` prefix, for example `0x54494D45`. |
| `source_file` | `string` | Source file where the extension EID and function IDs were parsed. |
| `dispatcher_helper` | `string` | RustSBI helper function used to summarize dispatch behavior, for example `_rustsbi_timer`. |
| `functions` | `FunctionInterface[]` | Functions implemented by the matched dispatcher helper. |

## `FunctionInterface`

Each element in `functions` is an object with these required fields:

| Field | Type | Meaning |
| --- | --- | --- |
| `function_name` | `string` | Function ID constant name from the extension `fid` module. |
| `function_id_hex` | `string` | Hexadecimal function ID string with `0x` prefix. |
| `used_registers` | `string[]` | Deduplicated subset of `parameter_registers` that appear in the summarized argument flow. |
| `argument_expressions` | `string[]` | Best-effort argument expressions rendered from dispatcher helper code. |

## Parsing Rules For Consumers

- Treat all fields listed above as required.
- Do not assume `extensions` is sorted semantically; preserve parser order unless your application needs reordering.
- Do not assume every spec-defined function appears in `functions`.
  Only functions that overlap with the actual RustSBI dispatcher helper are emitted.
- Treat `used_registers` as a summary, not a complete semantic proof.
  It reflects register identifiers visible after the parser's local alias substitution.
- Treat `argument_expressions` as analysis hints.
  They are stringified Rust expressions, not a stable AST format.
- Treat `dispatcher_helper` as the helper actually recovered from `traits.rs`.
  For `base`, the derive macro may contain multiple real helper templates such as
  `_rustsbi_base_env_info` and `_rustsbi_base_bare`; the JSON records the helper that matches the
  parsed RustSBI dispatcher implementation.
- An empty `used_registers` array is valid.
  It means the current implementation did not observe any direct `a0` to `a5` register usage in the summarized path.
- An empty `argument_expressions` array is valid.
  It usually means the dispatcher helper returns a value directly or does not pass user-controlled arguments into a nested call.

## Producer Behavior

The current parser also emits warnings to stderr when:

- a `pub mod ...;` item in `sbi-spec/src/lib.rs` does not expose a standard `EID_* + mod fid { ... }`
  extension contract and is therefore skipped
- an extension exists in `sbi-spec`
- but one or more spec-declared function IDs are not implemented by the matched dispatcher helper
- an extension exists in `sbi-spec`
- but the current RustSBI checkout has no matching dispatcher helper for it

Those warnings are not part of `sbi_interfaces.json`, so downstream parsers should not expect a warning field in the JSON itself.

## Example

```json
{
  "target_repository": "target_repo/rustsbi",
  "extension_register": "a7",
  "function_register": "a6",
  "parameter_registers": ["a0", "a1", "a2", "a3", "a4", "a5"],
  "extensions": [
    {
      "module": "time",
      "extension_name": "TIME",
      "eid_constant": "EID_TIME",
      "eid_value_hex": "0x54494D45",
      "source_file": "target_repo/rustsbi/library/sbi-spec/src/time.rs",
      "dispatcher_helper": "_rustsbi_timer",
      "functions": [
        {
          "function_name": "SET_TIMER",
          "function_id_hex": "0x0",
          "used_registers": ["a0", "a1"],
          "argument_expressions": ["a0 as _", "concat_u32 (a1, a0)"]
        }
      ]
    }
  ]
}
```

## Machine-Readable Schema

For validation in scripts or CI, use:

- [`sbi_interfaces.schema.json`](/home/ssdns/code/rusy_analyzer/docs/json_spec/sbi_interfaces.schema.json)
