//! Centralized regex patterns for MIR structure parsing and panic classification.

use regex::Regex;

use crate::utils::error::AnalyzerError;

/// Regex bundle used by the MIR structural parser.
pub struct ParserPatterns {
    /// Matches a textual MIR function header such as `fn demo::f(`.
    pub function_start: Regex,
    /// Matches a basic block header such as `bb7: {` or `bb7 (cleanup): {`.
    pub block_start: Regex,
}

/// Regex bundle used by the MIR panic classifier.
pub struct ClassifierPatterns {
    /// Matches generic explicit panic helper calls such as `panic_fmt` or `begin_panic`.
    pub explicit_panic_call: Regex,
    /// Matches `assert_failed` helper calls.
    pub assert_failed: Regex,
    /// Matches `assert_failed_eq` helper calls.
    pub assert_failed_eq: Regex,
    /// Matches `assert_failed_ne` helper calls.
    pub assert_failed_ne: Regex,
    /// Matches `panic_bounds_check` helper calls that should still classify as `BV`.
    pub panic_bounds_check_call: Regex,
    /// Matches broader panic-like helper calls for unknown explicit fallback output.
    pub panic_like_call: Regex,
    /// Matches unwrap/expect failure strings.
    pub unwrap_failed: Regex,
    /// Matches bounds-check panic text.
    pub index_out_of_bounds: Regex,
    /// Matches char-boundary panic text.
    pub not_a_char_boundary: Regex,
    /// Matches add-overflow panic text.
    pub add_overflow: Regex,
    /// Matches subtract-overflow panic text.
    pub subtract_overflow: Regex,
    /// Matches multiply-overflow panic text.
    pub multiply_overflow: Regex,
    /// Matches negation-overflow panic text.
    pub neg_overflow: Regex,
    /// Matches left-shift-overflow panic text.
    pub shift_left_overflow: Regex,
    /// Matches right-shift-overflow panic text.
    pub shift_right_overflow: Regex,
    /// Matches divide-by-zero panic text.
    pub divide_by_zero: Regex,
    /// Matches remainder-by-zero panic text.
    pub remainder_by_zero: Regex,
    /// Matches resumed-after-return panic text.
    pub resumed_after_return: Regex,
    /// Matches resumed-after-panic panic text.
    pub resumed_after_panic: Regex,
    /// Matches misaligned-pointer-dereference panic text.
    pub misaligned_pointer_dereference: Regex,
    /// Matches null-pointer-dereference panic text.
    pub null_pointer_dereference: Regex,
    /// Matches invalid-enum-construction panic text.
    pub invalid_enum_construction: Regex,
    /// Matches line-oriented MIR assert terminators so unknown assert kinds can emit warnings.
    pub assert_terminator: Regex,
}

impl ParserPatterns {
    /// Compiles all regexes required by the MIR structural parser.
    ///
    /// # Errors
    ///
    /// Returns an error if any parser regex is invalid.
    pub fn compile() -> Result<Self, AnalyzerError> {
        Ok(Self {
            function_start: compile_regex(r"^\s*fn\s+(.+?)\s*\(")?,
            block_start: compile_regex(r"^\s*(bb\d+)(?:\s+\(cleanup\))?:\s*\{$")?,
        })
    }
}

impl ClassifierPatterns {
    /// Compiles all regexes required by the MIR panic classifier.
    ///
    /// # Errors
    ///
    /// Returns an error if any classifier regex is invalid.
    pub fn compile() -> Result<Self, AnalyzerError> {
        Ok(Self {
            explicit_panic_call: compile_regex(
                r"(?:(?:[A-Za-z0-9_]+::)*(?:panic|panic_fmt|panic_display|panic_nounwind(?:_[a-z_]+)?|panic_explicit|panic_cold_explicit|begin_panic(?:_[a-z_]+)?))(?:::<[^>\n]+>)?\s*\(",
            )?,
            assert_failed: compile_regex(
                r"(?:^|[^A-Za-z0-9_:])(?:(?:core|std)::panicking::)?assert_failed(?:::<[^>\n]+>)?\s*\(",
            )?,
            assert_failed_eq: compile_regex(
                r"(?:^|[^A-Za-z0-9_:])(?:(?:core|std)::panicking::)?assert_failed_eq(?:::<[^>\n]+>)?\s*\(",
            )?,
            assert_failed_ne: compile_regex(
                r"(?:^|[^A-Za-z0-9_:])(?:(?:core|std)::panicking::)?assert_failed_ne(?:::<[^>\n]+>)?\s*\(",
            )?,
            panic_bounds_check_call: compile_regex(
                r"(?:^|[^A-Za-z0-9_:])(?:(?:core|std)::panicking::)?panic_bounds_check(?:::<[^>\n]+>)?\s*\(",
            )?,
            panic_like_call: compile_regex(
                r"(?:(?:[A-Za-z0-9_]+::)*[A-Za-z_][A-Za-z0-9_]*panic[A-Za-z0-9_]*|(?:core|std)::(?:option|result)::[A-Za-z0-9_:]*(?:unwrap|expect)[A-Za-z0-9_:]*|(?:[A-Za-z0-9_]+::)*(?:panic|begin_panic)[A-Za-z_]*)(?:::<[^>\n]+>)?\s*\(",
            )?,
            unwrap_failed: compile_regex(
                r#"unwrap failed|expect failed|called `(?:Option|Result)::unwrap\(\)`|called `Option::expect\(\)`|called `Result::expect\(\)`|(?:core|std)::(?:option|result)::[A-Za-z0-9_:]*unwrap_failed|(?:core|std)::(?:option|result)::[A-Za-z0-9_:]*expect_failed"#,
            )?,
            index_out_of_bounds: compile_regex(r#"index out of bounds"#)?,
            not_a_char_boundary: compile_regex(r#"not a char boundary"#)?,
            add_overflow: compile_regex(
                r#"attempt to add with overflow|attempt to compute `\{\} \+ \{\}`, which would overflow"#,
            )?,
            subtract_overflow: compile_regex(
                r#"attempt to subtract with overflow|attempt to compute `\{\} - \{\}`, which would overflow"#,
            )?,
            multiply_overflow: compile_regex(
                r#"attempt to multiply with overflow|attempt to compute `\{\} \* \{\}`, which would overflow"#,
            )?,
            neg_overflow: compile_regex(
                r#"attempt to negate with overflow|attempt to negate `\{\}`, which would overflow"#,
            )?,
            shift_left_overflow: compile_regex(
                r#"attempt to shift left with overflow|attempt to shift left by `\{\}`, which would overflow"#,
            )?,
            shift_right_overflow: compile_regex(
                r#"attempt to shift right with overflow|attempt to shift right by `\{\}`, which would overflow"#,
            )?,
            divide_by_zero: compile_regex(r#"divide by zero|attempt to divide `\{\}` by zero"#)?,
            remainder_by_zero: compile_regex(
                r#"calculate the remainder with a divisor of zero|attempt to calculate the remainder of `\{\}` with a divisor of zero"#,
            )?,
            resumed_after_return: compile_regex(
                r#"coroutine resumed after completion|async fn resumed after completion"#,
            )?,
            resumed_after_panic: compile_regex(r#"coroutine resumed after panic"#)?,
            misaligned_pointer_dereference: compile_regex(r#"misaligned pointer dereference"#)?,
            null_pointer_dereference: compile_regex(r#"null pointer dereference"#)?,
            invalid_enum_construction: compile_regex(
                r#"trying to construct an enum from an invalid value|invalid enum discriminant"#,
            )?,
            assert_terminator: compile_regex(r"(?m)^\s*assert\s*\(")?,
        })
    }
}

/// Collapses internal whitespace so matched evidence is stable in JSON output.
pub fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn compile_regex(pattern: &str) -> Result<Regex, AnalyzerError> {
    Regex::new(pattern).map_err(|err| {
        AnalyzerError::new(
            "compile MIR regex",
            format!("failed to compile pattern `{pattern}`: {err}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_patterns_and_normalizes_text() {
        let parser = ParserPatterns::compile().unwrap();
        let classifier = ClassifierPatterns::compile().unwrap();

        assert!(parser.function_start.is_match("fn demo() {"));
        assert!(parser.block_start.is_match("    bb1 (cleanup): {"));
        assert!(
            classifier
                .explicit_panic_call
                .is_match("panic_fmt(move _4)")
        );
        assert!(
            classifier
                .explicit_panic_call
                .is_match("core::panicking::panic_cold_explicit(move _4)")
        );
        assert!(
            classifier
                .unwrap_failed
                .is_match("core::result::unwrap_failed(_1, _2)")
        );
        assert!(
            classifier
                .assert_failed
                .is_match("_9 = assert_failed::<usize, usize>(const core::panicking::AssertKind::Eq, move _6, move _53, move _10)")
        );
        assert!(
            !classifier
                .explicit_panic_call
                .is_match("const core::panicking::AssertKind::Eq")
        );
        assert!(
            classifier
                .panic_bounds_check_call
                .is_match("_3 = core::panicking::panic_bounds_check(move _4, move _5)")
        );
        assert!(
            classifier
                .not_a_char_boundary
                .is_match("byte index 1 is not a char boundary; it is inside")
        );
        assert!(
            classifier
                .panic_like_call
                .is_match("_4 = core::panicking::panic_always::<T>(move _3)")
        );
        assert!(
            classifier
                .assert_terminator
                .is_match("    assert(!move _1, \"index out of bounds\")")
        );
        assert!(
            !classifier
                .assert_terminator
                .is_match("_3 = const \"assert failed\";")
        );
        assert!(classifier.add_overflow.is_match(
            r#"assert(!move (_1.1: bool), "attempt to compute `{} + {}`, which would overflow")"#
        ));
        assert!(classifier.subtract_overflow.is_match(
            r#"assert(!move (_1.1: bool), "attempt to compute `{} - {}`, which would overflow")"#
        ));
        assert!(classifier.multiply_overflow.is_match(
            r#"assert(!move (_1.1: bool), "attempt to compute `{} * {}`, which would overflow")"#
        ));
        assert!(classifier.shift_left_overflow.is_match(
            r#"assert(!move (_1.1: bool), "attempt to shift left by `{}`, which would overflow")"#
        ));
        assert!(classifier.shift_right_overflow.is_match(
            r#"assert(!move (_1.1: bool), "attempt to shift right by `{}`, which would overflow")"#
        ));
        assert!(classifier.neg_overflow.is_match(
            r#"assert(!move (_1.1: bool), "attempt to negate `{}`, which would overflow")"#
        ));
        assert!(
            classifier
                .resumed_after_return
                .is_match(r#"assert(!move _1, "coroutine resumed after completion")"#)
        );
        assert!(
            classifier
                .resumed_after_panic
                .is_match(r#"assert(!move _1, "coroutine resumed after panic")"#)
        );
        assert!(classifier.misaligned_pointer_dereference.is_match(
            r#"assert(!move _1, "misaligned pointer dereference: address must be a multiple of 0x{:x} but is 0x{:x}")"#
        ));
        assert!(
            classifier
                .null_pointer_dereference
                .is_match(r#"assert(!move _1, "null pointer dereference occurred")"#)
        );
        assert!(classifier.invalid_enum_construction.is_match(
            r#"assert(!move _1, "trying to construct an enum from an invalid value 5")"#
        ));
        assert!(classifier.remainder_by_zero.is_match(
            r#"assert(!move _4, "attempt to calculate the remainder of `{}` with a divisor of zero")"#
        ));
        assert_eq!(normalize_whitespace("a   b\n c"), "a b c");
    }
}
