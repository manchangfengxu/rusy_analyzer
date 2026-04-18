# `fuzz_targets.json` Output Specification

This directory records the current output contract for the Milestone 3 stitcher in
[`src/bin/fuzz_stitcher.rs`](/home/ssdns/code/rusy_analyzer/src/bin/fuzz_stitcher.rs).

The current M3 pipeline consumes:

- `sbi_interfaces.json`
- `panic_sites.json`
- `sbi.mir`

and produces:

- `fuzz_targets.json`
- `local_constraints.json`

## Design Goal

This schema is intentionally split into two layers:

- a **stable normalized dispatch layer** (`dispatch_model` + `route.selector_values` + `constraint`) that downstream tooling should prefer
- a **compatibility / provenance layer** (`route.source_binding`) that embeds the current Milestone 1 SBI objects without modifying their original schema

That split is the main decoupling strategy for future projects with a centralized dispatch layer, such as hypercall routers, OS syscall tables, kernel service multiplexers, or ioctl-style command gateways.
Downstream consumers should avoid hard-coding `extension` / `function` semantics and instead read the normalized selector chain.

Internally, M3 now runs in two stages:

- local constraint harvest
- route stitching

Only successfully stitched targets are serialized into `targets`.
The harvest-stage results are emitted separately into `local_constraints.json` so they do not pollute the stitched target array.

## Top-Level Structure

The file is a JSON object with these required fields:

| Field | Type | Meaning |
| --- | --- | --- |
| `schema_version` | `string` | Schema version currently emitted by the M3 stitcher. Initial value: `1.0.0`. |
| `target_repository` | `string` | Path to the analyzed repository root. |
| `dispatch_model` | `DispatchModel` | Normalized description of the centralized dispatch contract used by the analyzed target. |
| `input_artifacts` | `InputArtifacts` | Provenance of the M1 report, M2 report, and MIR text consumed by M3. |
| `summary` | `FuzzTargetSummary` | Aggregate counts over all stitched targets. |
| `targets` | `FuzzTarget[]` | Final stitched targets that connect one entry route to one panic site with one backward-sliced constraint bundle. |

## `DispatchModel`

This section is the schema's long-term stable abstraction layer.
It describes the dispatch family once at the document level, so each target only needs to provide concrete selector values.

| Field | Type | Meaning |
| --- | --- | --- |
| `interface_family` | `string` | Current broad interface family. Initial enum: `sbi`, `hypercall`, `syscall`, `ioctl`, `kernel_dispatch`, `custom`. |
| `dispatch_style` | `string` | Whether the target uses a selector-driven centralized dispatcher. Current values: `selector_dispatch`, `mixed_dispatch`, `unknown`. |
| `selector_layers` | `SelectorLayer[]` | Ordered selector channels, for example `extension_id -> function_id` in SBI or `service -> opcode` in a hypercall ABI. |
| `input_slots` | `InputSlot[]` | Ordered externally controllable inputs that may later appear in constraints. |

### `SelectorLayer`

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | `string` | Logical selector name, for example `extension_id`, `function_id`, `service`, `opcode`. |
| `role` | `string` | Semantic role of this selector layer. |
| `carrier_type` | `string` | How the selector is carried, for example register, argument, memory field, immediate, or table key. |
| `carrier_name` | `string` | Concrete carrier identifier, for example `a7`, `x0`, `rax`, `arg0`. |
| `selector_position` | `integer` | Stable order in the selector chain. |

### `InputSlot`

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | `string` | Stable input slot name such as `a0`, `arg2`, `req.len`. |
| `carrier_type` | `string` | Where the input comes from, for example register, argument, memory, or context. |
| `slot_position` | `integer` | Position within the carrier family. |
| `width_bits` | `integer?` | Optional bit width when known. |
| `direction` | `string?` | Optional direction metadata, currently `in` or `inout`. |

## `InputArtifacts`

This section gives the M3 output enough provenance to explain which upstream reports were stitched together.

| Field | Type | Meaning |
| --- | --- | --- |
| `interface_report.path` | `string` | Path to the consumed `sbi_interfaces.json`. |
| `interface_report.schema_id` | `string` | Schema identity for the consumed M1 report. |
| `panic_report.path` | `string` | Path to the consumed `panic_sites.json`. |
| `panic_report.schema_id` | `string` | Schema identity for the consumed M2 report. |
| `panic_report.schema_version` | `string` | M2 schema version consumed by this run. |
| `mir_text.path` | `string` | Path to the MIR text file used for slicing. |
| `mir_text.format` | `string` | Current required value: `unpretty_mir_text`. |

## `FuzzTargetSummary`

| Field | Type | Meaning |
| --- | --- | --- |
| `total_targets` | `integer` | Total stitched targets emitted into `targets`. |
| `categories` | `object` | Aggregate counts by M2 macro category `EA` / `BV` / `AE`. |
| `statuses.complete` | `integer` | Targets with fully resolved constraints. |
| `statuses.partial` | `integer` | Targets with useful but incomplete constraints. |
| `statuses.truncated` | `integer` | Targets cut short by the defensive truncation mechanism. |
| `statuses.unresolved` | `integer` | Targets whose route was found but no usable constraint could be reconstructed. |
| `targets_with_call_trace` | `integer` | Number of targets that include a non-empty `call_trace`. |

## `FuzzTarget`

Each element in `targets` is an object with these required fields:

| Field | Type | Meaning |
| --- | --- | --- |
| `target_id` | `string` | Stable logical identifier for one stitched route-to-panic target. |
| `route` | `RouteBinding` | Entry route information, including normalized selectors and a read-only M1 binding snapshot. |
| `panic_site` | `PanicSite` | Full verbatim M2 panic site object, embedded as a read-only node. |
| `constraint` | `ConstraintBundle` | Backward-sliced path constraint bundle. |
| `call_trace` | `CallTraceFrame[]?` | Optional interprocedural route from entry toward the panic owner. |

## `RouteBinding`

`route` is where the schema stops being SBI-specific for downstream consumers.
The consumer-facing contract is `route_id`, `entry_name`, and `selector_values`.
`source_binding` exists so the current M3 producer can still preserve the exact M1 objects it stitched from.

| Field | Type | Meaning |
| --- | --- | --- |
| `route_id` | `string` | Stable route identifier, for example `sbi:0x54494D45:0x0`. |
| `entry_name` | `string` | Human-readable entry label, for example `TIME::SET_TIMER`. |
| `selector_values` | `SelectorValue[]` | Concrete values for the dispatch model's selector layers. |
| `source_binding` | `SbiSourceBinding` | Read-only copy of the M1 extension + function objects. |

### `SelectorValue`

| Field | Type | Meaning |
| --- | --- | --- |
| `selector_name` | `string` | Must match one selector layer name from `dispatch_model.selector_layers`. |
| `selector_value` | `string` | Concrete value used by this route. |
| `value_format` | `string` | Serialization hint, for example `hex`, `decimal`, `string`, `symbolic`, `mixed`. |
| `selector_symbol` | `string?` | Optional symbolic alias such as `EID_TIME` or `SET_TIMER`. |

### `SbiSourceBinding`

`source_binding` is the compatibility layer for the current repository state.
It embeds the original M1 objects by reference instead of re-inventing them.

| Field | Type | Meaning |
| --- | --- | --- |
| `source_kind` | `string` | Current required value: `sbi_interfaces_v1`. |
| `extension` | `ExtensionInterface` | Verbatim M1 extension object. |
| `function` | `FunctionInterface` | Verbatim M1 function object. |

## `ConstraintBundle`

This object is deliberately richer than a single free-form string.
The canonical human-readable surface is still `expression`, but downstream tools also receive a clause list, the external inputs that matter, and the substitution trail from MIR locals to external inputs.

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `string` | Resolution status: `complete`, `partial`, `truncated`, `unresolved`. |
| `format` | `string` | Current required value: `clause_conjunction_v1`. |
| `expression` | `string` | Canonical readable conjunction after algebraic substitution, for example `(a0 + 5) * 2 >= 100 && a1 != 0`. It may be empty when the bundle is `unresolved`. |
| `clauses` | `ConstraintClause[]` | Clause-level decomposition of the full expression. |
| `external_inputs` | `ExternalInputRef[]` | Subset of dispatch inputs that actually influence this target. |
| `substitutions` | `SymbolSubstitution[]` | Audit trail showing how MIR locals were resolved upward. |
| `slice_depth` | `integer` | Maximum backward-slice depth consumed by this target. Hard cap: `50`. |
| `stop_reasons` | `string[]` | Structured explanation for truncation or partial resolution. Empty for fully resolved cases. |
| `warnings` | `string[]` | Non-fatal warnings that must not be silently dropped. |

### `ConstraintClause`

| Field | Type | Meaning |
| --- | --- | --- |
| `clause_id` | `string` | Stable per-target clause identifier. |
| `role` | `string` | Clause role: `path_guard`, `panic_guard`, `value_relation`, `assumption`, `opaque_boundary`. |
| `expression` | `string` | Human-readable clause text. |
| `source_function` | `string?` | Function where the clause was recovered. |
| `source_basic_block` | `string?` | Basic block where the clause originated. |
| `evidence` | `string?` | Short MIR evidence snippet or summarized statement. |

### `ExternalInputRef`

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | `string` | External input slot name, for example `a0`. |
| `carrier_type` | `string` | Input carrier type. |
| `slot_position` | `integer` | Position inside that carrier family. |
| `source_expression` | `string?` | Optional rendered expression showing how the slot entered the local computation. |

### `SymbolSubstitution`

| Field | Type | Meaning |
| --- | --- | --- |
| `mir_symbol` | `string` | MIR local such as `_7`. |
| `binding_kind` | `string` | How that local was resolved: `external_input`, `derived`, `constant`, `unknown`. |
| `resolved_expression` | `string` | Best-known replacement expression after substitution. |
| `depends_on` | `string[]?` | Other symbols or external inputs used by this substitution. |

## `CallTraceFrame`

`call_trace` stays optional because not every usable slice requires a full interprocedural chain.
When present, it is ordered from entry to panic owner.

| Field | Type | Meaning |
| --- | --- | --- |
| `depth` | `integer` | Zero-based frame depth from the entry route. |
| `function` | `string` | Function at this trace frame. |
| `basic_block` | `string?` | Relevant MIR basic block when known. |
| `role` | `string` | One of `entry`, `intermediate`, `panic_owner`. |
| `call_resolution` | `string` | Whether the edge was `resolved`, `opaque`, `indirect`, or `truncated`. |
| `call_kind` | `string?` | How the call edge was modeled, for example direct, trait_dispatch, indirect, foreign, or unknown. |
| `callsite` | `string?` | Optional rendered MIR callsite statement. |

## Decoupling Rules For Consumers

- Prefer `dispatch_model.selector_layers` + `route.selector_values` to identify an entry.
- Treat `route.source_binding` as provenance, not as the primary semantic contract.
- Treat `panic_site` as an immutable M2 snapshot. M3 must not rewrite or flatten its fields.
- Treat `constraint.expression` as the canonical readable output, but prefer `clauses` and `substitutions` for programmatic auditing.
- Treat `status`, `stop_reasons`, and `warnings` as first-class outputs. M3 must never silently drop truncation or opacity events.

## Why This Design Avoids Over-Coupling To Milestone 1

The current repository only has one upstream interface report: `sbi_interfaces.json`.
So this schema keeps a required `source_binding` that embeds those exact objects.
However, the fields that downstream tools are expected to rely on are no longer named after SBI concepts:

- selector semantics live in `dispatch_model`
- concrete route identity lives in `route.selector_values`
- externally controllable operands live in `constraint.external_inputs`

That means a future hypercall or kernel-dispatch adapter can preserve the same consumer-facing shape by changing:

- `dispatch_model.interface_family`
- `dispatch_model.selector_layers`
- `dispatch_model.input_slots`
- `route.selector_values`

while keeping `panic_site`, `constraint`, `call_trace`, `summary`, and most downstream logic unchanged.

## Example

```json
{
  "schema_version": "1.0.0",
  "target_repository": "target_repo/rustsbi",
  "dispatch_model": {
    "interface_family": "sbi",
    "dispatch_style": "selector_dispatch",
    "selector_layers": [
      {
        "name": "extension_id",
        "role": "namespace",
        "carrier_type": "register",
        "carrier_name": "a7",
        "selector_position": 0
      },
      {
        "name": "function_id",
        "role": "operation",
        "carrier_type": "register",
        "carrier_name": "a6",
        "selector_position": 1
      }
    ],
    "input_slots": [
      {
        "name": "a0",
        "carrier_type": "register",
        "slot_position": 0,
        "width_bits": 64,
        "direction": "in"
      },
      {
        "name": "a1",
        "carrier_type": "register",
        "slot_position": 1,
        "width_bits": 64,
        "direction": "in"
      }
    ]
  },
  "input_artifacts": {
    "interface_report": {
      "path": "sbi_interfaces.json",
      "schema_id": "https://rusy-analyzer.local/schemas/sbi_interfaces.schema.json"
    },
    "panic_report": {
      "path": "panic_sites.json",
      "schema_id": "https://rusy-analyzer.local/schemas/panic_sites.schema.json",
      "schema_version": "2.0.0"
    },
    "mir_text": {
      "path": "sbi.mir",
      "format": "unpretty_mir_text"
    }
  },
  "summary": {
    "total_targets": 1,
    "categories": {
      "EA": 0,
      "BV": 1,
      "AE": 0
    },
    "statuses": {
      "complete": 1,
      "partial": 0,
      "truncated": 0,
      "unresolved": 0
    },
    "targets_with_call_trace": 1
  },
  "targets": [
    {
      "target_id": "sbi:0x54494D45:0x0:demo::bounds:bb7",
      "route": {
        "route_id": "sbi:0x54494D45:0x0",
        "entry_name": "TIME::SET_TIMER",
        "selector_values": [
          {
            "selector_name": "extension_id",
            "selector_value": "0x54494D45",
            "value_format": "hex",
            "selector_symbol": "EID_TIME"
          },
          {
            "selector_name": "function_id",
            "selector_value": "0x0",
            "value_format": "hex",
            "selector_symbol": "SET_TIMER"
          }
        ],
        "source_binding": {
          "source_kind": "sbi_interfaces_v1",
          "extension": {
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
                "used_registers": [
                  "a0",
                  "a1"
                ],
                "argument_expressions": [
                  "a0 as _",
                  "concat_u32(a1, a0)"
                ]
              }
            ]
          },
          "function": {
            "function_name": "SET_TIMER",
            "function_id_hex": "0x0",
            "used_registers": [
              "a0",
              "a1"
            ],
            "argument_expressions": [
              "a0 as _",
              "concat_u32(a1, a0)"
            ]
          }
        }
      },
      "panic_site": {
        "function": "demo::bounds",
        "basic_block": "bb7",
        "is_cleanup": false,
        "category": "BV",
        "panic_kind": "index_out_of_bounds",
        "panic_origin": "implicit_assert",
        "evidence_type": "assert",
        "evidence": "index out of bounds",
        "line_start": 42,
        "line_end": 45
      },
      "constraint": {
        "status": "complete",
        "format": "clause_conjunction_v1",
        "expression": "a0 < a1 && a1 != 0",
        "clauses": [
          {
            "clause_id": "c0",
            "role": "panic_guard",
            "expression": "a0 < a1",
            "source_function": "demo::bounds",
            "source_basic_block": "bb7",
            "evidence": "assert(Lt(move _3, move _4), ...)"
          },
          {
            "clause_id": "c1",
            "role": "path_guard",
            "expression": "a1 != 0",
            "source_function": "demo::set_timer",
            "source_basic_block": "bb3",
            "evidence": "_4 = Len((*_2))"
          }
        ],
        "external_inputs": [
          {
            "name": "a0",
            "carrier_type": "register",
            "slot_position": 0,
            "source_expression": "param[0]"
          },
          {
            "name": "a1",
            "carrier_type": "register",
            "slot_position": 1,
            "source_expression": "param[1]"
          }
        ],
        "substitutions": [
          {
            "mir_symbol": "_3",
            "binding_kind": "external_input",
            "resolved_expression": "a0"
          },
          {
            "mir_symbol": "_4",
            "binding_kind": "derived",
            "resolved_expression": "a1",
            "depends_on": [
              "a1"
            ]
          }
        ],
        "slice_depth": 4,
        "stop_reasons": [],
        "warnings": []
      },
      "call_trace": [
        {
          "depth": 0,
          "function": "_rustsbi_timer",
          "role": "entry",
          "call_resolution": "resolved",
          "call_kind": "direct",
          "callsite": "match function { SET_TIMER => ... }"
        },
        {
          "depth": 1,
          "function": "demo::set_timer",
          "basic_block": "bb3",
          "role": "intermediate",
          "call_resolution": "resolved",
          "call_kind": "direct",
          "callsite": "_7 = demo::set_timer(move _1, move _2)"
        },
        {
          "depth": 2,
          "function": "demo::bounds",
          "basic_block": "bb7",
          "role": "panic_owner",
          "call_resolution": "resolved"
        }
      ]
    }
  ]
}
```

## Machine-Readable Schema

For validation in scripts or CI, use:

- [`fuzz_targets.schema.json`](/home/ssdns/code/rusy_analyzer/docs/json_spec/fuzz_targets.schema.json)
