//! Structural parser for textual `-Z unpretty=mir` output.

use std::path::Path;

use crate::mir::patterns::ParserPatterns;
use crate::utils::error::{AnalyzerError, ScanWarning};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedBasicBlock {
    pub function_name: String,
    pub block_label: String,
    pub is_cleanup: bool,
    pub start_line: usize,
    pub end_line: usize,
    pub body: String,
}

#[derive(Debug)]
pub(crate) struct ParsedMir {
    pub blocks: Vec<ParsedBasicBlock>,
    pub warnings: Vec<ScanWarning>,
}

#[derive(Debug)]
struct ActiveFunction {
    name: String,
    brace_depth: usize,
}

#[derive(Debug)]
struct ActiveBlock {
    function_name: String,
    label: String,
    is_cleanup: bool,
    start_line: usize,
    body: Vec<String>,
}

/// Parses one `unpretty MIR` text file into function-scoped basic blocks.
///
/// The parser treats real `fn ... {`, `scope ... {`, and `bbN: {` forms as the authoritative
/// structure contract. Top-level `const ... = {}` and `alloc... {}` regions are skipped.
pub(crate) fn parse_mir_text(mir_text: &str, mir_path: &Path) -> Result<ParsedMir, AnalyzerError> {
    let patterns = ParserPatterns::compile()?;
    let mut current_function: Option<ActiveFunction> = None;
    let mut current_block: Option<ActiveBlock> = None;
    let mut blocks = Vec::new();
    let mut warnings = Vec::new();

    for (index, line) in mir_text.lines().enumerate() {
        let line_number = index + 1;

        if current_function.is_none() {
            if let Some(captures) = patterns.function_start.captures(line) {
                current_function = Some(ActiveFunction {
                    name: captures[1].trim().to_string(),
                    brace_depth: 1,
                });
            }
            continue;
        }

        if let Some(block) = current_block.as_mut() {
            block.body.push(line.to_string());
            if line.trim() == "}" {
                let finished = current_block.take().ok_or_else(|| {
                    AnalyzerError::new(
                        mir_path.display().to_string(),
                        format!("internal error while closing block at line {line_number}"),
                    )
                })?;
                blocks.push(ParsedBasicBlock {
                    function_name: finished.function_name,
                    block_label: finished.label,
                    is_cleanup: finished.is_cleanup,
                    start_line: finished.start_line,
                    end_line: line_number,
                    body: finished.body.join("\n"),
                });
            }
            continue;
        }

        if let Some(captures) = patterns.block_start.captures(line) {
            let trimmed = line.trim();
            current_block = Some(ActiveBlock {
                // SAFETY: the early `current_function.is_none()` guard above ensures every
                // recognized basic block header is observed inside an active function scope.
                function_name: current_function
                    .as_ref()
                    .map(|function| function.name.clone())
                    .expect("block_start only reached inside a function scope"),
                label: captures[1].to_string(),
                is_cleanup: trimmed.contains("(cleanup)"),
                start_line: line_number,
                body: vec![line.to_string()],
            });
            continue;
        }

        let trimmed = line.trim();

        if trimmed.starts_with("bb") && trimmed.contains(':') {
            warnings.push(ScanWarning::new(
                format!("{}:{line_number}", mir_path.display()),
                format!("encountered malformed MIR basic block header: {trimmed}"),
            ));
            continue;
        }

        if trimmed.ends_with('{') {
            if let Some(function) = current_function.as_mut() {
                function.brace_depth += 1;
            }
            continue;
        }

        if trimmed == "}" {
            let function = current_function.as_mut().ok_or_else(|| {
                AnalyzerError::new(
                    mir_path.display().to_string(),
                    format!("unexpected function scope close at line {line_number}"),
                )
            })?;
            if function.brace_depth == 0 {
                return Err(AnalyzerError::new(
                    mir_path.display().to_string(),
                    format!("invalid function scope depth at line {line_number}"),
                ));
            }
            function.brace_depth -= 1;
            if function.brace_depth == 0 {
                current_function = None;
            }
        }
    }

    if let Some(block) = current_block {
        return Err(AnalyzerError::new(
            format!("{}:{}", mir_path.display(), block.start_line),
            format!(
                "unterminated MIR basic block '{}' in function '{}'",
                block.label, block.function_name
            ),
        ));
    }

    if let Some(function) = current_function {
        return Err(AnalyzerError::new(
            mir_path.display().to_string(),
            format!(
                "reached EOF while still inside MIR function '{}'",
                function.name
            ),
        ));
    }

    Ok(ParsedMir { blocks, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_scope_and_cleanup_blocks() {
        let mir = r#"
const X: usize = {
    bb0: {
        return;
    }
}

fn demo::scoped(_1: usize) -> () {
    scope 1 {
        debug x => _1;
    }

    bb0: {
        assert(move _1, "index out of bounds") -> [success: bb1, unwind unreachable];
    }

    bb1 (cleanup): {
        resume;
    }
}
"#;
        let parsed = parse_mir_text(mir, Path::new("mock.mir")).unwrap();

        assert_eq!(parsed.blocks.len(), 2);
        assert_eq!(parsed.blocks[0].function_name, "demo::scoped");
        assert_eq!(parsed.blocks[0].block_label, "bb0");
        assert!(!parsed.blocks[0].is_cleanup);
        assert_eq!(parsed.blocks[1].block_label, "bb1");
        assert!(parsed.blocks[1].is_cleanup);
    }

    #[test]
    fn warns_on_malformed_block_header() {
        let mir = r#"
fn demo::broken() -> () {
    bb0:
    bb1: {
        return;
    }
}
"#;
        let parsed = parse_mir_text(mir, Path::new("broken.mir")).unwrap();
        assert_eq!(parsed.blocks.len(), 1);
        assert_eq!(parsed.warnings.len(), 1);
        assert!(
            parsed.warnings[0]
                .to_string()
                .contains("malformed MIR basic block header")
        );
    }

    #[test]
    fn errors_on_unterminated_function_or_block() {
        let unterminated_block = r#"
fn demo::bad() -> () {
    bb0: {
        return;
"#;
        let error = parse_mir_text(unterminated_block, Path::new("bad_block.mir")).unwrap_err();
        assert!(error.to_string().contains("unterminated MIR basic block"));

        let unterminated_function = r#"
fn demo::bad() -> () {
    scope 1 {
        debug x => _1;
    }
"#;
        let error =
            parse_mir_text(unterminated_function, Path::new("bad_function.mir")).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("reached EOF while still inside MIR function")
        );
    }

    #[test]
    fn parses_generic_and_qualified_function_headers() {
        let mir = r#"
fn <impl at src/lib.rs:10:1: 18:2>::scan::<T>(_1: &T) -> () {
    bb0: {
        _2 = panic_fmt(move _3) -> unwind unreachable;
    }
}
"#;
        let parsed = parse_mir_text(mir, Path::new("generic.mir")).unwrap();
        assert_eq!(parsed.blocks.len(), 1);
        assert_eq!(
            parsed.blocks[0].function_name,
            "<impl at src/lib.rs:10:1: 18:2>::scan::<T>"
        );
    }
}
