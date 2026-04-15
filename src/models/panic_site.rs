use serde::{Deserialize, Serialize};

/// Schema version currently emitted for `panic_sites.json`.
pub const PANIC_SITE_SCHEMA_VERSION: &str = "2.0.0";

/// Top-level panic category used by the MIR scanner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanicCategory {
    /// Explicit panic or unwrap/expect failure.
    #[serde(rename = "EA")]
    Ea,
    /// Bounds violation.
    #[serde(rename = "BV")]
    Bv,
    /// Arithmetic error such as overflow or division failure.
    #[serde(rename = "AE")]
    Ae,
}

/// Concrete panic subtype emitted for one MIR site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanicKind {
    /// Explicit panic helper call.
    ExplicitPanic,
    /// `assert!()` lowered into an `assert_failed` helper call.
    AssertFailed,
    /// `assert_eq!()` lowered into an `assert_failed_eq` helper call.
    AssertFailedEq,
    /// `assert_ne!()` lowered into an `assert_failed_ne` helper call.
    AssertFailedNe,
    /// `unwrap()` or `expect()` failure path.
    UnwrapFailed,
    /// Slice or array index out of bounds.
    IndexOutOfBounds,
    /// String or slice access crossed a UTF-8 char boundary.
    NotACharBoundary,
    /// Addition overflow.
    AddOverflow,
    /// Subtraction overflow.
    SubtractOverflow,
    /// Multiplication overflow.
    MultiplyOverflow,
    /// Unary negation overflow.
    NegOverflow,
    /// Left shift overflow.
    ShiftLeftOverflow,
    /// Right shift overflow.
    ShiftRightOverflow,
    /// Division by zero.
    DivideByZero,
    /// Remainder by zero.
    RemainderByZero,
    /// Resuming a completed coroutine or async fn.
    ResumedAfterReturn,
    /// Resuming a coroutine after panic.
    ResumedAfterPanic,
    /// Dereferencing a misaligned pointer.
    MisalignedPointerDereference,
    /// Dereferencing a null pointer.
    NullPointerDereference,
    /// Constructing an enum from an invalid discriminant.
    InvalidEnumConstruction,
    /// An implicit MIR assert that is not yet mapped to a known family.
    UnknownAssert,
    /// A panic-like MIR call that is not yet mapped to a known helper family.
    UnknownExplicitCall,
}

/// Evidence source that caused the classifier to emit one panic site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceType {
    /// Text came from an MIR assert terminator.
    Assert,
    /// Text came from a panic helper call or panic string constant.
    Call,
}

/// High-level source family for one panic site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanicOrigin {
    /// Site originated from an MIR `Assert` terminator.
    ImplicitAssert,
    /// Site originated from an MIR `Call` path.
    ExplicitCall,
}

/// One extracted panic-oriented MIR basic block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanicSite {
    /// MIR function name containing the panic site.
    pub function: String,
    /// Basic block label such as `bb7`.
    pub basic_block: String,
    /// Whether this basic block was marked as cleanup MIR.
    pub is_cleanup: bool,
    /// Macro panic category used by downstream tooling.
    pub category: PanicCategory,
    /// Concrete panic subtype within the macro category.
    pub panic_kind: PanicKind,
    /// Whether the site came from an implicit MIR assert or an explicit call path.
    pub panic_origin: PanicOrigin,
    /// Whether evidence was matched from an assert terminator or call path.
    pub evidence_type: EvidenceType,
    /// Normalized matched text fragment that justified classification.
    pub evidence: String,
    /// 1-based line number where the basic block header starts.
    pub line_start: usize,
    /// 1-based line number where the basic block closes.
    pub line_end: usize,
}

/// Aggregate count summary grouped by top-level panic category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategorySummary {
    /// Number of `EA` sites.
    #[serde(rename = "EA")]
    pub ea: usize,
    /// Number of `BV` sites.
    #[serde(rename = "BV")]
    pub bv: usize,
    /// Number of `AE` sites.
    #[serde(rename = "AE")]
    pub ae: usize,
}

/// Top-level summary section of `panic_sites.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanicSummary {
    /// Total number of emitted panic sites.
    pub total_sites: usize,
    /// Per-category aggregate counts.
    pub categories: CategorySummary,
}

/// Top-level JSON document emitted by the MIR scanner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanicSiteReport {
    /// Schema version understood by downstream validators.
    pub schema_version: String,
    /// Path to the MIR file that was analyzed.
    pub mir_source: String,
    /// Aggregate counts over all emitted sites.
    pub summary: PanicSummary,
    /// All extracted panic-oriented MIR sites.
    pub sites: Vec<PanicSite>,
}

impl PanicSiteReport {
    /// Builds a complete panic-site report and derives summary counts from the provided sites.
    pub fn new(mir_source: String, sites: Vec<PanicSite>) -> Self {
        Self {
            schema_version: PANIC_SITE_SCHEMA_VERSION.to_string(),
            mir_source,
            summary: PanicSummary::from_sites(&sites),
            sites,
        }
    }
}

impl PanicSummary {
    /// Computes aggregate category counts from a slice of panic sites.
    pub fn from_sites(sites: &[PanicSite]) -> Self {
        let mut categories = CategorySummary {
            ea: 0,
            bv: 0,
            ae: 0,
        };

        for site in sites {
            match site.category {
                PanicCategory::Ea => categories.ea += 1,
                PanicCategory::Bv => categories.bv += 1,
                PanicCategory::Ae => categories.ae += 1,
            }
        }

        Self {
            total_sites: sites.len(),
            categories,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_summary_for_all_categories() {
        let sites = vec![
            PanicSite {
                function: "f".to_string(),
                basic_block: "bb0".to_string(),
                is_cleanup: false,
                category: PanicCategory::Ea,
                panic_kind: PanicKind::ExplicitPanic,
                panic_origin: PanicOrigin::ExplicitCall,
                evidence_type: EvidenceType::Call,
                evidence: "panic_fmt".to_string(),
                line_start: 1,
                line_end: 2,
            },
            PanicSite {
                function: "f".to_string(),
                basic_block: "bb1".to_string(),
                is_cleanup: false,
                category: PanicCategory::Bv,
                panic_kind: PanicKind::IndexOutOfBounds,
                panic_origin: PanicOrigin::ImplicitAssert,
                evidence_type: EvidenceType::Assert,
                evidence: "index out of bounds".to_string(),
                line_start: 3,
                line_end: 4,
            },
            PanicSite {
                function: "f".to_string(),
                basic_block: "bb2".to_string(),
                is_cleanup: false,
                category: PanicCategory::Ae,
                panic_kind: PanicKind::AddOverflow,
                panic_origin: PanicOrigin::ImplicitAssert,
                evidence_type: EvidenceType::Assert,
                evidence: "attempt to add with overflow".to_string(),
                line_start: 5,
                line_end: 6,
            },
            PanicSite {
                function: "f".to_string(),
                basic_block: "bb3".to_string(),
                is_cleanup: true,
                category: PanicCategory::Ae,
                panic_kind: PanicKind::DivideByZero,
                panic_origin: PanicOrigin::ImplicitAssert,
                evidence_type: EvidenceType::Assert,
                evidence: "divide by zero".to_string(),
                line_start: 7,
                line_end: 8,
            },
        ];

        let summary = PanicSummary::from_sites(&sites);
        assert_eq!(summary.total_sites, 4);
        assert_eq!(summary.categories.ea, 1);
        assert_eq!(summary.categories.bv, 1);
        assert_eq!(summary.categories.ae, 2);
    }
}
