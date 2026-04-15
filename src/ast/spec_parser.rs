//! Parsing of `library/sbi-spec` modules using real Rust syntax via `syn`.

use std::collections::BTreeMap;
use std::path::Path;

use syn::{Expr, ExprCall, ExprCast, ExprLit, File, Item, Lit};

use crate::utils::error::{AnalyzerError, ScanWarning};
use crate::utils::fs::read_text;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedSpecExtension {
    pub module: String,
    pub extension_name: String,
    pub eid_constant: String,
    pub eid_value: u64,
    pub source_file: String,
    pub functions: BTreeMap<String, u64>,
}

#[derive(Debug)]
pub(crate) struct ParsedSpecRepository {
    pub extensions: Vec<ParsedSpecExtension>,
    pub warnings: Vec<ScanWarning>,
}

pub(crate) fn parse_spec_repository(
    repo_root: &Path,
) -> Result<ParsedSpecRepository, AnalyzerError> {
    let spec_root = repo_root.join("library/sbi-spec/src");
    let root_path = spec_root.join("lib.rs");
    let root_source = read_text(&root_path)?;
    let root_ast = parse_rust_file(&root_source, &root_path)?;

    let mut extensions = Vec::new();
    let mut warnings = Vec::new();
    for module in discover_extension_modules(&root_ast, &root_path)? {
        let path = spec_root.join(format!("{module}.rs"));
        let source = read_text(&path)?;
        let ast = parse_rust_file(&source, &path)?;
        match parse_extension_module(&ast, &path, &module)? {
            Some(extension) => extensions.push(extension),
            None => warnings.push(ScanWarning::new(
                path.display().to_string(),
                format!(
                    "skipping module '{module}' because it does not expose a standard `EID_*` extension contract"
                ),
            )),
        }
    }

    Ok(ParsedSpecRepository {
        extensions,
        warnings,
    })
}

fn discover_extension_modules(file: &File, path: &Path) -> Result<Vec<String>, AnalyzerError> {
    let mut modules = Vec::new();
    for item in &file.items {
        if let Item::Mod(item_mod) = item {
            if item_mod.ident == "binary" || item_mod.ident == "tests" {
                continue;
            }
            if item_mod.content.is_some() {
                continue;
            }
            modules.push(item_mod.ident.to_string());
        }
    }

    if modules.is_empty() {
        return Err(AnalyzerError::new(
            path.display().to_string(),
            "failed to discover extension modules in sbi-spec root",
        ));
    }

    Ok(modules)
}

fn parse_extension_module(
    file: &File,
    path: &Path,
    expected_module: &str,
) -> Result<Option<ParsedSpecExtension>, AnalyzerError> {
    let Some(eid_const) = file.items.iter().find_map(|item| match item {
        Item::Const(item_const) if item_const.ident.to_string().starts_with("EID_") => {
            Some(item_const)
        }
        _ => None,
    }) else {
        return Ok(None);
    };

    let fid_module = file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Mod(item_mod) if item_mod.ident == "fid" => Some(item_mod),
            _ => None,
        })
        .ok_or_else(|| {
            AnalyzerError::new(
                path.display().to_string(),
                format!("failed to locate `mod fid` in module '{expected_module}'"),
            )
        })?;

    let eid_constant = eid_const.ident.to_string();
    let extension_name = eid_constant
        .strip_prefix("EID_")
        .ok_or_else(|| {
            AnalyzerError::new(
                path.display().to_string(),
                format!("invalid EID constant name `{eid_constant}`"),
            )
        })?
        .to_string();

    let (_, fid_items) = fid_module.content.as_ref().ok_or_else(|| {
        AnalyzerError::new(
            path.display().to_string(),
            format!("module '{expected_module}' uses out-of-line `mod fid`, unsupported"),
        )
    })?;

    let mut functions = BTreeMap::new();
    for item in fid_items {
        if let Item::Const(item_const) = item {
            functions.insert(
                item_const.ident.to_string(),
                eval_usize_expr(&item_const.expr, path)?,
            );
        }
    }

    if functions.is_empty() {
        return Err(AnalyzerError::new(
            path.display().to_string(),
            format!("no function IDs found in `mod fid` for '{expected_module}'"),
        ));
    }

    Ok(Some(ParsedSpecExtension {
        module: expected_module.to_string(),
        extension_name,
        eid_constant,
        eid_value: eval_usize_expr(&eid_const.expr, path)?,
        source_file: path.display().to_string(),
        functions,
    }))
}

fn eval_usize_expr(expr: &Expr, path: &Path) -> Result<u64, AnalyzerError> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Int(value),
            ..
        }) => value.base10_parse::<u64>().map_err(|err| {
            AnalyzerError::new(
                path.display().to_string(),
                format!("invalid integer literal `{}`: {err}", value.token()),
            )
        }),
        Expr::Cast(ExprCast { expr, .. }) => eval_usize_expr(expr, path),
        Expr::Call(ExprCall { func, args, .. }) => {
            if quote::ToTokens::to_token_stream(func)
                .to_string()
                .replace(' ', "")
                == "crate::eid_from_str"
                && args.len() == 1
            {
                match &args[0] {
                    Expr::Lit(ExprLit {
                        lit: Lit::Str(value),
                        ..
                    }) => eid_from_str(&value.value()).map_err(|message| {
                        AnalyzerError::new(
                            path.display().to_string(),
                            format!("invalid EID literal {:?}: {message}", value.value()),
                        )
                    }),
                    _ => Err(AnalyzerError::new(
                        path.display().to_string(),
                        "eid_from_str expects a string literal argument",
                    )),
                }
            } else {
                Err(AnalyzerError::new(
                    path.display().to_string(),
                    format!("unsupported const expression for numeric evaluation: {expr:?}"),
                ))
            }
        }
        _ => Err(AnalyzerError::new(
            path.display().to_string(),
            format!("unsupported const expression for numeric evaluation: {expr:?}"),
        )),
    }
}

fn parse_rust_file(source: &str, path: &Path) -> Result<File, AnalyzerError> {
    syn::parse_file(source).map_err(|err| {
        AnalyzerError::new(
            path.display().to_string(),
            format!("failed to parse Rust source: {err}"),
        )
    })
}

fn eid_from_str(name: &str) -> Result<u64, String> {
    match name.as_bytes() {
        [a] => Ok(u64::from(u32::from_be_bytes([0, 0, 0, *a]))),
        [a, b] => Ok(u64::from(u32::from_be_bytes([0, 0, *a, *b]))),
        [a, b, c] => Ok(u64::from(u32::from_be_bytes([0, *a, *b, *c]))),
        [a, b, c, d] => Ok(u64::from(u32::from_be_bytes([*a, *b, *c, *d]))),
        _ => Err(format!("unsupported EID string length {}", name.len())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_modules_and_extension_constants() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("spec_parser_{unique}"));
        let spec_dir = root.join("library/sbi-spec/src");
        fs::create_dir_all(&spec_dir).unwrap();
        fs::write(spec_dir.join("lib.rs"), "pub mod time;\npub mod base;\n").unwrap();
        fs::write(
            spec_dir.join("time.rs"),
            r#"pub const EID_TIME: usize = crate::eid_from_str("TIME") as _;
            mod fid { pub const SET_TIMER: usize = 0; }"#,
        )
        .unwrap();
        fs::write(
            spec_dir.join("base.rs"),
            r#"pub const EID_BASE: usize = 0x10;
            mod fid { pub const PROBE_EXTENSION: usize = 3; }"#,
        )
        .unwrap();

        let parsed = parse_spec_repository(&root).unwrap();
        assert_eq!(parsed.extensions.len(), 2);
        assert_eq!(parsed.extensions[0].functions.len(), 1);
        assert!(parsed.warnings.is_empty());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_invalid_eid_and_out_of_line_fid() {
        let bad = r#"pub const EID_BAD: usize = crate::eid_from_str("TOO_LONG") as _; mod fid { pub const X: usize = 0; }"#;
        let path = PathBuf::from("bad.rs");
        let error = parse_extension_module(&parse_rust_file(bad, &path).unwrap(), &path, "bad")
            .unwrap_err();
        assert!(error.to_string().contains("invalid EID literal"));

        let bad_fid = r#"pub const EID_BASE: usize = 0x10; mod fid;"#;
        let error = parse_extension_module(
            &parse_rust_file(bad_fid, &PathBuf::from("fid.rs")).unwrap(),
            &PathBuf::from("fid.rs"),
            "base",
        )
        .unwrap_err();
        assert!(error.to_string().contains("out-of-line `mod fid`"));
    }

    #[test]
    fn warns_and_skips_non_standard_module() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("spec_parser_nonstandard_{unique}"));
        let spec_dir = root.join("library/sbi-spec/src");
        fs::create_dir_all(&spec_dir).unwrap();
        fs::write(spec_dir.join("lib.rs"), "pub mod legacy;\npub mod time;\n").unwrap();
        fs::write(
            spec_dir.join("legacy.rs"),
            "pub const LEGACY_SET_TIMER: usize = 0;",
        )
        .unwrap();
        fs::write(
            spec_dir.join("time.rs"),
            r#"pub const EID_TIME: usize = crate::eid_from_str("TIME") as _;
            mod fid { pub const SET_TIMER: usize = 0; }"#,
        )
        .unwrap();

        let parsed = parse_spec_repository(&root).unwrap();
        assert_eq!(parsed.extensions.len(), 1);
        assert_eq!(parsed.warnings.len(), 1);
        assert!(
            parsed.warnings[0]
                .to_string()
                .contains("skipping module 'legacy'")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn eid_from_str_rejects_empty_and_oversized() {
        assert!(eid_from_str("").is_err());
        assert!(eid_from_str("TOOLONG").is_err());
        assert_eq!(
            eid_from_str("AB").unwrap(),
            u64::from(u32::from_be_bytes([0, 0, 0x41, 0x42]))
        );
    }

    #[test]
    fn parses_large_usize_constants_as_u64() {
        let path = PathBuf::from("large.rs");
        let source = r#"
        pub const EID_BIG: usize = 0x1_0000_0000;
        mod fid { pub const BIG_FN: usize = 0x1_0000_0001; }
        "#;
        let parsed = parse_extension_module(&parse_rust_file(source, &path).unwrap(), &path, "big")
            .unwrap()
            .unwrap();

        assert_eq!(parsed.eid_value, 0x1_0000_0000);
        assert_eq!(parsed.functions["BIG_FN"], 0x1_0000_0001);
    }
}
