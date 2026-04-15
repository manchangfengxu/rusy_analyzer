//! Classification of parsed MIR basic blocks into the public panic taxonomy.

use std::path::Path;

use crate::mir::parser::ParsedBasicBlock;
use crate::mir::patterns::{ClassifierPatterns, normalize_whitespace};
use crate::models::panic_site::{EvidenceType, PanicCategory, PanicKind, PanicOrigin, PanicSite};
use crate::utils::error::{AnalyzerError, ScanWarning};

pub(crate) struct ClassifiedBlocks {
    pub sites: Vec<PanicSite>,
    pub warnings: Vec<ScanWarning>,
    pub unknown_site_count: usize,
}

struct Classification {
    site: Option<PanicSite>,
    warning: Option<ScanWarning>,
}

/// Classifies parsed MIR basic blocks using regex patterns over the captured block body text.
///
/// The current classifier emits three macro categories: `EA`, `BV`, and `AE`.
pub(crate) fn classify_blocks(
    blocks: Vec<ParsedBasicBlock>,
    mir_path: &Path,
) -> Result<ClassifiedBlocks, AnalyzerError> {
    let patterns = ClassifierPatterns::compile()?;
    let mut sites = Vec::new();
    let mut warnings = Vec::new();
    let mut unknown_site_count = 0;

    for block in blocks {
        let classification = classify_block(&block, &patterns, mir_path);
        if let Some(site) = classification.site {
            if matches!(
                site.panic_kind,
                PanicKind::UnknownAssert | PanicKind::UnknownExplicitCall
            ) {
                unknown_site_count += 1;
            }
            sites.push(site);
        }
        if let Some(warning) = classification.warning {
            warnings.push(warning);
        }
    }

    Ok(ClassifiedBlocks {
        sites,
        warnings,
        unknown_site_count,
    })
}

fn classify_block(
    block: &ParsedBasicBlock,
    patterns: &ClassifierPatterns,
    mir_path: &Path,
) -> Classification {
    let body = block.body.as_str();
    let is_assert = patterns.assert_terminator.is_match(body);

    if is_assert {
        return classify_assert_block(block, patterns, body, mir_path);
    }

    classify_call_block(block, patterns, body, mir_path)
}

fn classify_assert_block(
    block: &ParsedBasicBlock,
    patterns: &ClassifierPatterns,
    body: &str,
    mir_path: &Path,
) -> Classification {
    if let Some(evidence) = first_match_text(&patterns.index_out_of_bounds, body) {
        return classified(build_site(
            block,
            PanicCategory::Bv,
            PanicKind::IndexOutOfBounds,
            PanicOrigin::ImplicitAssert,
            EvidenceType::Assert,
            evidence,
        ));
    }

    if let Some(evidence) = first_match_text(&patterns.not_a_char_boundary, body) {
        return classified(build_site(
            block,
            PanicCategory::Bv,
            PanicKind::NotACharBoundary,
            PanicOrigin::ImplicitAssert,
            EvidenceType::Assert,
            evidence,
        ));
    }

    if let Some(evidence) = first_match_text(&patterns.add_overflow, body) {
        return classified(build_site(
            block,
            PanicCategory::Ae,
            PanicKind::AddOverflow,
            PanicOrigin::ImplicitAssert,
            EvidenceType::Assert,
            evidence,
        ));
    }

    if let Some(evidence) = first_match_text(&patterns.subtract_overflow, body) {
        return classified(build_site(
            block,
            PanicCategory::Ae,
            PanicKind::SubtractOverflow,
            PanicOrigin::ImplicitAssert,
            EvidenceType::Assert,
            evidence,
        ));
    }

    if let Some(evidence) = first_match_text(&patterns.multiply_overflow, body) {
        return classified(build_site(
            block,
            PanicCategory::Ae,
            PanicKind::MultiplyOverflow,
            PanicOrigin::ImplicitAssert,
            EvidenceType::Assert,
            evidence,
        ));
    }

    if let Some(evidence) = first_match_text(&patterns.neg_overflow, body) {
        return classified(build_site(
            block,
            PanicCategory::Ae,
            PanicKind::NegOverflow,
            PanicOrigin::ImplicitAssert,
            EvidenceType::Assert,
            evidence,
        ));
    }

    if let Some(evidence) = first_match_text(&patterns.shift_left_overflow, body) {
        return classified(build_site(
            block,
            PanicCategory::Ae,
            PanicKind::ShiftLeftOverflow,
            PanicOrigin::ImplicitAssert,
            EvidenceType::Assert,
            evidence,
        ));
    }

    if let Some(evidence) = first_match_text(&patterns.shift_right_overflow, body) {
        return classified(build_site(
            block,
            PanicCategory::Ae,
            PanicKind::ShiftRightOverflow,
            PanicOrigin::ImplicitAssert,
            EvidenceType::Assert,
            evidence,
        ));
    }

    if let Some(evidence) = first_match_text(&patterns.divide_by_zero, body) {
        return classified(build_site(
            block,
            PanicCategory::Ae,
            PanicKind::DivideByZero,
            PanicOrigin::ImplicitAssert,
            EvidenceType::Assert,
            evidence,
        ));
    }

    if let Some(evidence) = first_match_text(&patterns.remainder_by_zero, body) {
        return classified(build_site(
            block,
            PanicCategory::Ae,
            PanicKind::RemainderByZero,
            PanicOrigin::ImplicitAssert,
            EvidenceType::Assert,
            evidence,
        ));
    }

    if let Some(evidence) = first_match_text(&patterns.resumed_after_return, body) {
        return classified(build_site(
            block,
            PanicCategory::Ea,
            PanicKind::ResumedAfterReturn,
            PanicOrigin::ImplicitAssert,
            EvidenceType::Assert,
            evidence,
        ));
    }

    if let Some(evidence) = first_match_text(&patterns.resumed_after_panic, body) {
        return classified(build_site(
            block,
            PanicCategory::Ea,
            PanicKind::ResumedAfterPanic,
            PanicOrigin::ImplicitAssert,
            EvidenceType::Assert,
            evidence,
        ));
    }

    if let Some(evidence) = first_match_text(&patterns.misaligned_pointer_dereference, body) {
        return classified(build_site(
            block,
            PanicCategory::Ea,
            PanicKind::MisalignedPointerDereference,
            PanicOrigin::ImplicitAssert,
            EvidenceType::Assert,
            evidence,
        ));
    }

    if let Some(evidence) = first_match_text(&patterns.null_pointer_dereference, body) {
        return classified(build_site(
            block,
            PanicCategory::Ea,
            PanicKind::NullPointerDereference,
            PanicOrigin::ImplicitAssert,
            EvidenceType::Assert,
            evidence,
        ));
    }

    if let Some(evidence) = first_match_text(&patterns.invalid_enum_construction, body) {
        return classified(build_site(
            block,
            PanicCategory::Ea,
            PanicKind::InvalidEnumConstruction,
            PanicOrigin::ImplicitAssert,
            EvidenceType::Assert,
            evidence,
        ));
    }

    if let Some(evidence) = first_match_text(&patterns.unwrap_failed, body) {
        return classified(build_site(
            block,
            PanicCategory::Ea,
            PanicKind::UnwrapFailed,
            PanicOrigin::ImplicitAssert,
            EvidenceType::Assert,
            evidence,
        ));
    }

    classified_with_warning(
        build_site(
            block,
            PanicCategory::Ea,
            PanicKind::UnknownAssert,
            PanicOrigin::ImplicitAssert,
            EvidenceType::Assert,
            fallback_assert_evidence(body),
        ),
        ScanWarning::new(
            format!(
                "{}:{}-{}",
                mir_path.display(),
                block.start_line,
                block.end_line
            ),
            format!(
                "unrecognized Assert terminator in {} {}",
                block.function_name, block.block_label
            ),
        ),
    )
}

fn classify_call_block(
    block: &ParsedBasicBlock,
    patterns: &ClassifierPatterns,
    body: &str,
    mir_path: &Path,
) -> Classification {
    if let Some(evidence) = first_match_text(&patterns.panic_bounds_check_call, body) {
        return classified(build_site(
            block,
            PanicCategory::Bv,
            PanicKind::IndexOutOfBounds,
            PanicOrigin::ExplicitCall,
            EvidenceType::Call,
            evidence,
        ));
    }

    if let Some(evidence) = first_match_text(&patterns.not_a_char_boundary, body) {
        return classified(build_site(
            block,
            PanicCategory::Bv,
            PanicKind::NotACharBoundary,
            PanicOrigin::ExplicitCall,
            EvidenceType::Call,
            evidence,
        ));
    }

    if let Some(evidence) = first_match_text(&patterns.assert_failed_eq, body) {
        return classified(build_site(
            block,
            PanicCategory::Ea,
            PanicKind::AssertFailedEq,
            PanicOrigin::ExplicitCall,
            EvidenceType::Call,
            evidence,
        ));
    }

    if let Some(evidence) = first_match_text(&patterns.assert_failed_ne, body) {
        return classified(build_site(
            block,
            PanicCategory::Ea,
            PanicKind::AssertFailedNe,
            PanicOrigin::ExplicitCall,
            EvidenceType::Call,
            evidence,
        ));
    }

    if let Some(evidence) = first_match_text(&patterns.assert_failed, body) {
        return classified(build_site(
            block,
            PanicCategory::Ea,
            PanicKind::AssertFailed,
            PanicOrigin::ExplicitCall,
            EvidenceType::Call,
            evidence,
        ));
    }

    if let Some(evidence) = first_match_text(&patterns.unwrap_failed, body) {
        return classified(build_site(
            block,
            PanicCategory::Ea,
            PanicKind::UnwrapFailed,
            PanicOrigin::ExplicitCall,
            EvidenceType::Call,
            evidence,
        ));
    }

    if let Some(evidence) = first_match_text(&patterns.explicit_panic_call, body) {
        return classified(build_site(
            block,
            PanicCategory::Ea,
            PanicKind::ExplicitPanic,
            PanicOrigin::ExplicitCall,
            EvidenceType::Call,
            evidence,
        ));
    }

    if let Some(evidence) = first_match_text(&patterns.panic_like_call, body) {
        return classified_with_warning(
            build_site(
                block,
                PanicCategory::Ea,
                PanicKind::UnknownExplicitCall,
                PanicOrigin::ExplicitCall,
                EvidenceType::Call,
                evidence,
            ),
            ScanWarning::new(
                format!(
                    "{}:{}-{}",
                    mir_path.display(),
                    block.start_line,
                    block.end_line
                ),
                format!(
                    "unrecognized panic-like call in {} {}",
                    block.function_name, block.block_label
                ),
            ),
        );
    }

    Classification {
        site: None,
        warning: None,
    }
}

fn build_site(
    block: &ParsedBasicBlock,
    category: PanicCategory,
    panic_kind: PanicKind,
    panic_origin: PanicOrigin,
    evidence_type: EvidenceType,
    evidence: String,
) -> PanicSite {
    PanicSite {
        function: block.function_name.clone(),
        basic_block: block.block_label.clone(),
        is_cleanup: block.is_cleanup,
        category,
        panic_kind,
        panic_origin,
        evidence_type,
        evidence,
        line_start: block.start_line,
        line_end: block.end_line,
    }
}

fn classified(site: PanicSite) -> Classification {
    Classification {
        site: Some(site),
        warning: None,
    }
}

fn classified_with_warning(site: PanicSite, warning: ScanWarning) -> Classification {
    Classification {
        site: Some(site),
        warning: Some(warning),
    }
}

fn first_match_text(pattern: &regex::Regex, text: &str) -> Option<String> {
    pattern
        .find(text)
        .map(|matched| normalize_whitespace(matched.as_str()))
}

fn fallback_assert_evidence(text: &str) -> String {
    if let Some(start) = text.find('"') {
        if let Some(end) = text[start + 1..].find('"') {
            return normalize_whitespace(&text[start + 1..start + 1 + end]);
        }
    }
    normalize_whitespace(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(body: &str) -> ParsedBasicBlock {
        ParsedBasicBlock {
            function_name: "demo::f".to_string(),
            block_label: "bb7".to_string(),
            is_cleanup: true,
            start_line: 10,
            end_line: 13,
            body: body.to_string(),
        }
    }

    #[test]
    fn classifies_all_supported_categories() {
        let path = Path::new("mock.mir");
        let warnings = classify_blocks(
            vec![
                block(r#"assert(move _1, "index out of bounds")"#),
                block(r#"assert(move _1, "byte index 1 is not a char boundary; it is inside 'é'")"#),
                block(
                    r#"assert(!move (_1.1: bool), "attempt to compute `{} + {}`, which would overflow")"#,
                ),
                block(
                    r#"assert(!move (_1.1: bool), "attempt to compute `{} - {}`, which would overflow")"#,
                ),
                block(r#"assert(!move (_1.1: bool), "attempt to compute `{} * {}`, which would overflow")"#),
                block(r#"assert(!move (_1.1: bool), "attempt to negate `{}`, which would overflow")"#),
                block(r#"assert(!move (_1.1: bool), "attempt to shift left by `{}`, which would overflow")"#),
                block(r#"assert(!move (_1.1: bool), "attempt to shift right by `{}`, which would overflow")"#),
                block(r#"assert(!move _4, "attempt to divide `{}` by zero")"#),
                block(
                    r#"assert(!move _4, "attempt to calculate the remainder of `{}` with a divisor of zero")"#,
                ),
                block(r#"assert(!move _1, "coroutine resumed after completion")"#),
                block(r#"assert(!move _1, "coroutine resumed after panic")"#),
                block(
                    r#"assert(!move _1, "misaligned pointer dereference: address must be a multiple of 0x{:x} but is 0x{:x}")"#,
                ),
                block(r#"assert(!move _1, "null pointer dereference occurred")"#),
                block(r#"assert(!move _1, "trying to construct an enum from an invalid value 5")"#),
                block(r#"_9 = assert_failed::<bool, bool>(move _1, move _2)"#),
                block(r#"_9 = assert_failed_eq::<usize, usize>(move _1, move _2)"#),
                block(r#"_9 = assert_failed_ne::<usize, usize>(move _1, move _2)"#),
                block(r#"_3 = core::panicking::panic_bounds_check(move _4, move _5) -> unwind unreachable;"#),
                block(r#"_3 = panic_fmt(move _4) -> unwind unreachable;"#),
                block(r#"_3 = const "called `Result::unwrap()` on an `Err` value";"#),
            ],
            path,
        )
        .unwrap();

        assert_eq!(warnings.warnings.len(), 0);
        assert_eq!(warnings.unknown_site_count, 0);
        assert_eq!(warnings.sites.len(), 21);
        assert_eq!(warnings.sites[0].category, PanicCategory::Bv);
        assert_eq!(warnings.sites[1].panic_kind, PanicKind::NotACharBoundary);
        assert_eq!(warnings.sites[2].panic_kind, PanicKind::AddOverflow);
        assert_eq!(warnings.sites[3].panic_kind, PanicKind::SubtractOverflow);
        assert_eq!(warnings.sites[4].panic_kind, PanicKind::MultiplyOverflow);
        assert_eq!(warnings.sites[5].panic_kind, PanicKind::NegOverflow);
        assert_eq!(warnings.sites[6].panic_kind, PanicKind::ShiftLeftOverflow);
        assert_eq!(warnings.sites[7].panic_kind, PanicKind::ShiftRightOverflow);
        assert_eq!(warnings.sites[8].category, PanicCategory::Ae);
        assert_eq!(warnings.sites[9].panic_kind, PanicKind::RemainderByZero);
        assert_eq!(warnings.sites[10].panic_kind, PanicKind::ResumedAfterReturn);
        assert_eq!(warnings.sites[11].panic_kind, PanicKind::ResumedAfterPanic);
        assert_eq!(
            warnings.sites[12].panic_kind,
            PanicKind::MisalignedPointerDereference
        );
        assert_eq!(
            warnings.sites[13].panic_kind,
            PanicKind::NullPointerDereference
        );
        assert_eq!(
            warnings.sites[14].panic_kind,
            PanicKind::InvalidEnumConstruction
        );
        assert_eq!(warnings.sites[15].panic_kind, PanicKind::AssertFailed);
        assert_eq!(warnings.sites[16].panic_kind, PanicKind::AssertFailedEq);
        assert_eq!(warnings.sites[17].panic_kind, PanicKind::AssertFailedNe);
        assert_eq!(warnings.sites[18].category, PanicCategory::Bv);
        assert_eq!(warnings.sites[18].panic_kind, PanicKind::IndexOutOfBounds);
        assert_eq!(warnings.sites[19].category, PanicCategory::Ea);
        assert_eq!(warnings.sites[20].panic_kind, PanicKind::UnwrapFailed);
        assert!(warnings.sites[0].is_cleanup);
        assert_eq!(warnings.sites[0].panic_origin, PanicOrigin::ImplicitAssert);
        assert_eq!(warnings.sites[0].evidence_type, EvidenceType::Assert);
        assert_eq!(warnings.sites[15].panic_origin, PanicOrigin::ExplicitCall);
        assert_eq!(warnings.sites[15].evidence_type, EvidenceType::Call);
        assert_eq!(warnings.sites[19].panic_kind, PanicKind::ExplicitPanic);
        assert_eq!(warnings.sites[20].evidence_type, EvidenceType::Call);
    }

    #[test]
    fn warns_on_unrecognized_assert() {
        let result = classify_blocks(
            vec![block(r#"assert(move _1, "some other assert text")"#)],
            Path::new("warn.mir"),
        )
        .unwrap();

        assert_eq!(result.sites.len(), 1);
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(result.unknown_site_count, 1);
        assert_eq!(result.sites[0].panic_kind, PanicKind::UnknownAssert);
        assert!(
            result.warnings[0]
                .to_string()
                .contains("unrecognized Assert terminator")
        );
    }

    #[test]
    fn detects_assert_evidence_for_unwrap_failure() {
        let result = classify_blocks(
            vec![block(
                r#"assert(!move (_1.0: bool), "called `Option::unwrap()` on a `None` value")"#,
            )],
            Path::new("unwrap_assert.mir"),
        )
        .unwrap();

        assert_eq!(result.sites.len(), 1);
        assert_eq!(result.sites[0].panic_kind, PanicKind::UnwrapFailed);
        assert_eq!(result.sites[0].panic_origin, PanicOrigin::ImplicitAssert);
        assert_eq!(result.sites[0].evidence_type, EvidenceType::Assert);
    }

    #[test]
    fn prefers_unwrap_failed_over_explicit_panic_when_both_match() {
        let result = classify_blocks(
            vec![block(
                r#"
                assert(!move (_1.0: bool), "called `Result::unwrap()` on an `Err` value");
                _5 = core::panicking::panic_fmt(move _4) -> unwind unreachable;
                "#,
            )],
            Path::new("priority.mir"),
        )
        .unwrap();

        assert_eq!(result.sites.len(), 1);
        assert_eq!(result.sites[0].panic_kind, PanicKind::UnwrapFailed);
        assert_eq!(result.sites[0].evidence_type, EvidenceType::Assert);
    }

    #[test]
    fn routes_assert_failed_and_bounds_helpers_by_semantics() {
        let result = classify_blocks(
            vec![
                block(r#"_9 = core::panicking::assert_failed::<bool, bool>(move _1, move _2)"#),
                block(r#"_9 = assert_failed_eq::<usize, usize>(move _1, move _2)"#),
                block(r#"_9 = assert_failed_ne::<usize, usize>(move _1, move _2)"#),
                block(r#"_3 = core::panicking::panic_bounds_check(move _4, move _5) -> unwind unreachable;"#),
                block(r#"byte index 1 is not a char boundary; it is inside 'é'"#),
            ],
            Path::new("semantic_calls.mir"),
        )
        .unwrap();

        assert_eq!(result.sites.len(), 5);
        assert_eq!(result.sites[0].panic_kind, PanicKind::AssertFailed);
        assert_eq!(result.sites[1].panic_kind, PanicKind::AssertFailedEq);
        assert_eq!(result.sites[2].panic_kind, PanicKind::AssertFailedNe);
        assert_eq!(result.sites[3].category, PanicCategory::Bv);
        assert_eq!(result.sites[3].panic_kind, PanicKind::IndexOutOfBounds);
        assert_eq!(result.sites[4].panic_kind, PanicKind::NotACharBoundary);
    }

    #[test]
    fn emits_unknown_explicit_call_instead_of_skipping() {
        let result = classify_blocks(
            vec![block(
                r#"_8 = core::panicking::panic_immediate(move _1) -> unwind unreachable;"#,
            )],
            Path::new("unknown_call.mir"),
        )
        .unwrap();

        assert_eq!(result.sites.len(), 1);
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(result.unknown_site_count, 1);
        assert_eq!(result.sites[0].panic_kind, PanicKind::UnknownExplicitCall);
        assert_eq!(result.sites[0].panic_origin, PanicOrigin::ExplicitCall);
        assert_eq!(result.sites[0].evidence_type, EvidenceType::Call);
        assert!(
            result.warnings[0]
                .to_string()
                .contains("unrecognized panic-like call")
        );
    }

    #[test]
    fn prefers_assert_semantics_when_block_contains_assert_and_call_signals() {
        let result = classify_blocks(
            vec![block(
                r#"
                assert(move _1, "index out of bounds");
                _3 = core::panicking::panic_bounds_check(move _4, move _5) -> unwind unreachable;
                "#,
            )],
            Path::new("assert_precedence.mir"),
        )
        .unwrap();

        assert_eq!(result.sites.len(), 1);
        assert_eq!(result.sites[0].panic_kind, PanicKind::IndexOutOfBounds);
        assert_eq!(result.sites[0].panic_origin, PanicOrigin::ImplicitAssert);
        assert_eq!(result.sites[0].evidence_type, EvidenceType::Assert);
    }
}
