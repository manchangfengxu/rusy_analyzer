//! Data models for the `local_constraints.json` auxiliary M3 artifact.

use serde::{Deserialize, Serialize};

use crate::models::fuzz_target::{
    ConstraintBundle, ConstraintStatus, DispatchModel, InputArtifacts,
};
use crate::models::panic_site::PanicSite;

/// Schema version emitted for `local_constraints.json`.
pub const LOCAL_CONSTRAINT_SCHEMA_VERSION: &str = "1.0.0";

/// Top-level report for route-independent local constraint harvest results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalConstraintReport {
    /// Version of the `local_constraints.json` schema emitted by this run.
    pub schema_version: String,
    /// Repository root or target identifier copied from the upstream M1 report.
    pub target_repository: String,
    /// Neutral dispatch abstraction shared with `fuzz_targets.json`.
    pub dispatch_model: DispatchModel,
    /// Provenance of the M1/M2/MIR inputs consumed during harvest.
    pub input_artifacts: InputArtifacts,
    /// Aggregate counts over all harvested local-constraint candidates.
    pub summary: LocalConstraintSummary,
    /// Per-panic-site local constraint candidates, including unresolved cases.
    pub candidates: Vec<LocalConstraintCandidate>,
}

impl LocalConstraintReport {
    /// Builds a schema-aligned local-constraint report from candidate records.
    pub fn new(
        target_repository: String,
        dispatch_model: DispatchModel,
        input_artifacts: InputArtifacts,
        candidates: Vec<LocalConstraintCandidate>,
    ) -> Self {
        let summary = LocalConstraintSummary::from_candidates(&candidates);
        Self {
            schema_version: LOCAL_CONSTRAINT_SCHEMA_VERSION.to_string(),
            target_repository,
            dispatch_model,
            input_artifacts,
            summary,
            candidates,
        }
    }
}

/// Aggregate summary for the local harvest artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalConstraintSummary {
    /// Number of candidate records written to `candidates`.
    pub total_candidates: usize,
    /// Counts grouped by degraded local-constraint status.
    pub statuses: LocalConstraintStatusSummary,
    /// Number of candidates that recovered a non-null seed context.
    pub candidates_with_seed_context: usize,
    /// Number of candidates whose canonical expression is non-empty.
    pub candidates_with_nonempty_expression: usize,
}

impl LocalConstraintSummary {
    /// Derives the summary fields from a candidate slice.
    pub fn from_candidates(candidates: &[LocalConstraintCandidate]) -> Self {
        let mut statuses = LocalConstraintStatusSummary::default();
        let mut candidates_with_seed_context = 0_usize;
        let mut candidates_with_nonempty_expression = 0_usize;

        for candidate in candidates {
            match candidate.constraint.status {
                ConstraintStatus::Partial => statuses.partial += 1,
                ConstraintStatus::Truncated => statuses.truncated += 1,
                ConstraintStatus::Unresolved => statuses.unresolved += 1,
                ConstraintStatus::Complete => statuses.partial += 1,
            }
            if candidate.seed_context.is_some() {
                candidates_with_seed_context += 1;
            }
            if !candidate.constraint.expression.is_empty() {
                candidates_with_nonempty_expression += 1;
            }
        }

        Self {
            total_candidates: candidates.len(),
            statuses,
            candidates_with_seed_context,
            candidates_with_nonempty_expression,
        }
    }
}

/// Status buckets emitted in `local_constraints.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalConstraintStatusSummary {
    /// Number of candidates with a useful but incomplete local constraint.
    pub partial: usize,
    /// Number of candidates truncated by depth or defensive slicing limits.
    pub truncated: usize,
    /// Number of candidates whose local constraint could not be reconstructed.
    pub unresolved: usize,
}

/// One route-independent local constraint candidate rooted at one panic site.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalConstraintCandidate {
    /// Stable logical identifier for this local candidate.
    pub candidate_id: String,
    /// Read-only embedded M2 panic site object.
    pub panic_site: PanicSite,
    /// Seed context used to start the local slice, or `null` when unavailable.
    pub seed_context: Option<SeedContext>,
    /// Route-independent constraint bundle harvested from MIR.
    pub constraint: ConstraintBundle,
    /// Weak route hints that may help downstream stitching or dictionary projection.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub route_hints: Vec<RouteHint>,
    /// Candidate-level warnings that must not be silently dropped.
    pub warnings: Vec<String>,
}

/// Route-independent local seed context recovered from MIR evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedContext {
    /// Human-readable seed expression before later substitution or enrichment.
    pub seed_expression: String,
    /// MIR locals directly referenced by the seed expression.
    pub seed_locals: Vec<String>,
    /// Evidence line or snippet from which the seed was reconstructed.
    pub evidence: String,
    /// Function that owns the panic seed.
    pub source_function: String,
    /// Basic block that owns the panic seed.
    pub source_basic_block: String,
}

/// Weak route hint attached to a local-only candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteHint {
    /// Kind of weak hint carried by this record.
    pub hint_kind: RouteHintKind,
    /// Human-readable hint payload.
    pub value: String,
    /// Function context where the hint was observed when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_function: Option<String>,
    /// Basic block where the hint was observed when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_basic_block: Option<String>,
    /// Short evidence snippet explaining the hint source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

/// Enumerates the allowed weak route-hint families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RouteHintKind {
    /// Inline scope hint that resembles a dispatcher helper.
    #[serde(rename = "inline_helper")]
    InlineHelper,
    /// Selector-like guard observed on a nearby control-flow edge.
    #[serde(rename = "selector_guard")]
    SelectorGuard,
    /// Caller function name tail observed on the reverse call chain.
    #[serde(rename = "caller_tail")]
    CallerTail,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::fuzz_target::{
        CarrierType, ClauseRole, ConstraintBundle, ConstraintClause, ConstraintStatus,
        DispatchStyle, InputArtifacts, InterfaceArtifact, InterfaceFamily, MirArtifact,
        PanicArtifact, SelectorLayer, SelectorRole, StopReason,
    };
    use crate::models::panic_site::{EvidenceType, PanicCategory, PanicKind, PanicOrigin};

    fn sample_dispatch_model() -> DispatchModel {
        DispatchModel {
            interface_family: InterfaceFamily::Sbi,
            dispatch_style: DispatchStyle::SelectorDispatch,
            selector_layers: vec![SelectorLayer {
                name: "extension_id".to_string(),
                role: SelectorRole::Namespace,
                carrier_type: CarrierType::Register,
                carrier_name: "a7".to_string(),
                selector_position: 0,
            }],
            input_slots: Vec::new(),
        }
    }

    fn sample_input_artifacts() -> InputArtifacts {
        InputArtifacts {
            interface_report: InterfaceArtifact {
                path: "sbi_interfaces.json".to_string(),
                schema_id: "https://rusy-analyzer.local/schemas/sbi_interfaces.schema.json"
                    .to_string(),
            },
            panic_report: PanicArtifact {
                path: "panic_sites.json".to_string(),
                schema_id: "https://rusy-analyzer.local/schemas/panic_sites.schema.json"
                    .to_string(),
                schema_version: "2.0.0".to_string(),
            },
            mir_text: MirArtifact {
                path: "sbi.mir".to_string(),
                format: "unpretty_mir_text".to_string(),
            },
        }
    }

    fn sample_panic_site() -> PanicSite {
        PanicSite {
            function: "leaf".to_string(),
            basic_block: "bb3".to_string(),
            is_cleanup: false,
            category: PanicCategory::Bv,
            panic_kind: PanicKind::IndexOutOfBounds,
            panic_origin: PanicOrigin::ImplicitAssert,
            evidence_type: EvidenceType::Assert,
            evidence: "assert(!move _8, \"index out of bounds\")".to_string(),
            line_start: 10,
            line_end: 12,
        }
    }

    fn sample_constraint(status: ConstraintStatus, expression: &str) -> ConstraintBundle {
        ConstraintBundle {
            status,
            format: "clause_conjunction_v1".to_string(),
            expression: expression.to_string(),
            clauses: vec![ConstraintClause {
                clause_id: "c1".to_string(),
                role: ClauseRole::PanicGuard,
                expression: "a0 >= 4096".to_string(),
                source_function: Some("leaf".to_string()),
                source_basic_block: Some("bb3".to_string()),
                evidence: Some("assert".to_string()),
            }],
            external_inputs: Vec::new(),
            substitutions: Vec::new(),
            slice_depth: 1,
            stop_reasons: if status == ConstraintStatus::Truncated {
                vec![StopReason::DepthLimit]
            } else {
                Vec::new()
            },
            warnings: Vec::new(),
        }
    }

    #[test]
    fn constants_are_correct() {
        assert_eq!(LOCAL_CONSTRAINT_SCHEMA_VERSION, "1.0.0");
    }

    #[test]
    fn summary_counts_seed_contexts_and_statuses() {
        let candidates = vec![
            LocalConstraintCandidate {
                candidate_id: "LC_0001".to_string(),
                panic_site: sample_panic_site(),
                seed_context: Some(SeedContext {
                    seed_expression: "_8 < _3".to_string(),
                    seed_locals: vec!["_8".to_string(), "_3".to_string()],
                    evidence: "assert".to_string(),
                    source_function: "leaf".to_string(),
                    source_basic_block: "bb3".to_string(),
                }),
                constraint: sample_constraint(ConstraintStatus::Partial, "a0 < a1"),
                route_hints: Vec::new(),
                warnings: Vec::new(),
            },
            LocalConstraintCandidate {
                candidate_id: "LC_0002".to_string(),
                panic_site: sample_panic_site(),
                seed_context: None,
                constraint: sample_constraint(ConstraintStatus::Truncated, "a0 < a1"),
                route_hints: Vec::new(),
                warnings: vec!["depth limited".to_string()],
            },
            LocalConstraintCandidate {
                candidate_id: "LC_0003".to_string(),
                panic_site: sample_panic_site(),
                seed_context: None,
                constraint: sample_constraint(ConstraintStatus::Unresolved, ""),
                route_hints: Vec::new(),
                warnings: vec!["no seed".to_string()],
            },
        ];

        let summary = LocalConstraintSummary::from_candidates(&candidates);
        assert_eq!(summary.total_candidates, 3);
        assert_eq!(summary.statuses.partial, 1);
        assert_eq!(summary.statuses.truncated, 1);
        assert_eq!(summary.statuses.unresolved, 1);
        assert_eq!(summary.candidates_with_seed_context, 1);
        assert_eq!(summary.candidates_with_nonempty_expression, 2);
    }

    #[test]
    fn report_round_trips_through_json() {
        let report = LocalConstraintReport::new(
            "repo".to_string(),
            sample_dispatch_model(),
            sample_input_artifacts(),
            vec![LocalConstraintCandidate {
                candidate_id: "LC_0001".to_string(),
                panic_site: sample_panic_site(),
                seed_context: Some(SeedContext {
                    seed_expression: "_8 < _3".to_string(),
                    seed_locals: vec!["_8".to_string(), "_3".to_string()],
                    evidence: "assert".to_string(),
                    source_function: "leaf".to_string(),
                    source_basic_block: "bb3".to_string(),
                }),
                constraint: sample_constraint(ConstraintStatus::Partial, "a0 < a1"),
                route_hints: vec![RouteHint {
                    hint_kind: RouteHintKind::InlineHelper,
                    value: "_rustsbi_timer".to_string(),
                    source_function: Some("handle_ecall".to_string()),
                    source_basic_block: None,
                    evidence: Some("scope 1 (inlined _rustsbi_timer::<T>)".to_string()),
                }],
                warnings: Vec::new(),
            }],
        );

        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"schema_version\":\"1.0.0\""));
        assert!(json.contains("\"route_hints\""));

        let decoded: LocalConstraintReport = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.summary.total_candidates, 1);
        assert_eq!(decoded.summary.statuses.partial, 1);
        assert_eq!(
            decoded.candidates[0].route_hints[0].hint_kind,
            RouteHintKind::InlineHelper
        );
    }
}
