//! Parsing of RustSBI helper dispatch logic from `library/rustsbi/src/traits.rs`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;

use quote::ToTokens;
use regex::Regex;
use syn::{
    Arm, Expr, ExprArray, ExprCall, ExprField, ExprIndex, ExprLit, ExprMatch, ExprMethodCall,
    ExprParen, ExprPath, ExprReference, File, ImplItem, Item, ItemFn, ItemImpl, Lit, Member, Pat,
    PatIdent, PatPath, PatType, Stmt, Type, TypePath,
};

use crate::utils::error::{AnalyzerError, ScanWarning};
use crate::utils::fs::read_text;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FunctionDispatch {
    pub used_registers: BTreeSet<String>,
    pub argument_expressions: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DispatchInfo {
    pub helper_name: String,
    pub functions: BTreeMap<String, FunctionDispatch>,
}

#[derive(Debug)]
pub(crate) struct ParsedTraitsRepository {
    pub dispatchers: BTreeMap<String, DispatchInfo>,
    pub warnings: Vec<ScanWarning>,
}

#[derive(Debug)]
struct SupportedExtension {
    module: String,
    helper_suffix: String,
}

pub(crate) fn parse_traits_repository(
    repo_root: &Path,
) -> Result<ParsedTraitsRepository, AnalyzerError> {
    let path = repo_root.join("library/rustsbi/src/traits.rs");
    let source = read_text(&path)?;
    let file = syn::parse_file(&source).map_err(|err| {
        AnalyzerError::new(
            path.display().to_string(),
            format!("failed to parse Rust source: {err}"),
        )
    })?;

    let supported = parse_supported_extensions(&file, &path)?;
    let dispatchers = parse_dispatch_helpers(&file, &path, &supported)?;

    Ok(ParsedTraitsRepository {
        dispatchers,
        warnings: Vec::new(),
    })
}

fn parse_supported_extensions(
    file: &File,
    path: &Path,
) -> Result<Vec<SupportedExtension>, AnalyzerError> {
    let impl_item = file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Impl(item_impl) if is_extension_probe_impl(item_impl) => Some(item_impl),
            _ => None,
        })
        .ok_or_else(|| {
            AnalyzerError::new(
                path.display().to_string(),
                "failed to locate `impl _ExtensionProbe for _StandardExtensionProbe`",
            )
        })?;

    let method = impl_item
        .items
        .iter()
        .find_map(|item| match item {
            ImplItem::Fn(item_fn) if item_fn.sig.ident == "probe_extension" => Some(item_fn),
            _ => None,
        })
        .ok_or_else(|| {
            AnalyzerError::new(
                path.display().to_string(),
                "failed to locate `_StandardExtensionProbe::probe_extension`",
            )
        })?;

    let match_expr =
        find_match_on_ident_in_stmts(&method.block.stmts, "extension").ok_or_else(|| {
            AnalyzerError::new(
                path.display().to_string(),
                "failed to locate `match extension` in probe_extension",
            )
        })?;

    let mut supported = Vec::new();
    for arm in &match_expr.arms {
        let Some(const_path) = extract_const_path_from_pat(&arm.pat) else {
            continue;
        };
        if !const_path.starts_with("spec::") || !const_path.contains("::EID_") {
            continue;
        }
        let module = const_path
            .split("::")
            .nth(1)
            .ok_or_else(|| {
                AnalyzerError::new(
                    path.display().to_string(),
                    format!("invalid extension path in probe match arm: {const_path}"),
                )
            })?
            .to_string();
        let helper_suffix = extract_self_field_name(&arm.body).ok_or_else(|| {
            AnalyzerError::new(
                path.display().to_string(),
                format!("unsupported probe body for extension module '{module}'"),
            )
        })?;
        supported.push(SupportedExtension {
            module,
            helper_suffix,
        });
    }

    if supported.is_empty() {
        return Err(AnalyzerError::new(
            path.display().to_string(),
            "no supported extension definitions were extracted from probe_extension",
        ));
    }

    Ok(supported)
}

fn parse_dispatch_helpers(
    file: &File,
    path: &Path,
    supported: &[SupportedExtension],
) -> Result<BTreeMap<String, DispatchInfo>, AnalyzerError> {
    let mut by_name = BTreeMap::new();
    for item in &file.items {
        if let Item::Fn(item_fn) = item {
            by_name.insert(item_fn.sig.ident.to_string(), item_fn);
        }
    }

    let mut dispatchers = BTreeMap::new();
    for extension in supported {
        let helper_name = if extension.module == "base" {
            "_rustsbi_base_env_info".to_string()
        } else {
            format!("_rustsbi_{}", extension.helper_suffix)
        };
        let function = by_name.get(&helper_name).ok_or_else(|| {
            AnalyzerError::new(
                path.display().to_string(),
                format!("failed to locate dispatcher helper `{helper_name}`"),
            )
        })?;
        dispatchers.insert(
            extension.module.clone(),
            parse_dispatch_function(function, path)?,
        );
    }

    Ok(dispatchers)
}

fn parse_dispatch_function(item_fn: &ItemFn, path: &Path) -> Result<DispatchInfo, AnalyzerError> {
    let mut functions = BTreeMap::new();
    collect_dispatch_matches(&item_fn.block.stmts, &BTreeMap::new(), &mut functions)?;
    if functions.is_empty() {
        return Err(AnalyzerError::new(
            path.display().to_string(),
            format!(
                "dispatcher helper `{}` did not expose any `match function` arms",
                item_fn.sig.ident
            ),
        ));
    }
    Ok(DispatchInfo {
        helper_name: item_fn.sig.ident.to_string(),
        functions,
    })
}

fn collect_dispatch_matches(
    stmts: &[Stmt],
    inherited_env: &BTreeMap<String, String>,
    functions: &mut BTreeMap<String, FunctionDispatch>,
) -> Result<(), AnalyzerError> {
    let mut env = inherited_env.clone();
    for stmt in stmts {
        for (name, value) in extract_simple_locals(stmt) {
            env.insert(name, value);
        }
        match stmt {
            Stmt::Expr(expr, _) => collect_dispatch_from_expr(expr, &env, functions)?,
            Stmt::Local(local) => {
                if let Some(init) = &local.init {
                    collect_dispatch_from_expr(&init.expr, &env, functions)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn collect_dispatch_from_expr(
    expr: &Expr,
    env: &BTreeMap<String, String>,
    functions: &mut BTreeMap<String, FunctionDispatch>,
) -> Result<(), AnalyzerError> {
    match expr {
        Expr::Match(expr_match) if expr_is_ident(&expr_match.expr, "function") => {
            for arm in &expr_match.arms {
                let Some(function_name) = extract_const_ident_from_pat(&arm.pat) else {
                    continue;
                };
                if body_is_not_supported(&arm.body) {
                    continue;
                }
                let mapping = summarize_dispatch_arm(arm, env);
                functions
                    .entry(function_name)
                    .and_modify(|current| merge_dispatch(current, &mapping))
                    .or_insert(mapping);
            }
        }
        Expr::Match(expr_match) => {
            for arm in &expr_match.arms {
                collect_dispatch_from_expr(&arm.body, env, functions)?;
            }
        }
        Expr::Block(expr_block) => {
            collect_dispatch_matches(&expr_block.block.stmts, env, functions)?
        }
        Expr::Paren(ExprParen { expr, .. }) => collect_dispatch_from_expr(expr, env, functions)?,
        Expr::Reference(ExprReference { expr, .. }) => {
            collect_dispatch_from_expr(expr, env, functions)?
        }
        _ => {}
    }
    Ok(())
}

fn summarize_dispatch_arm(arm: &Arm, env: &BTreeMap<String, String>) -> FunctionDispatch {
    let rendered = render_expr(&arm.body, env);
    let used_registers = collect_registers(&rendered);
    let mut argument_expressions = BTreeSet::new();
    if let Some(args) = extract_call_argument_expressions(&arm.body, env) {
        argument_expressions.extend(args);
    }
    FunctionDispatch {
        used_registers,
        argument_expressions,
    }
}

fn merge_dispatch(current: &mut FunctionDispatch, incoming: &FunctionDispatch) {
    current
        .used_registers
        .extend(incoming.used_registers.iter().cloned());
    current
        .argument_expressions
        .extend(incoming.argument_expressions.iter().cloned());
}

fn extract_simple_locals(stmt: &Stmt) -> Vec<(String, String)> {
    let Stmt::Local(local) = stmt else {
        return Vec::new();
    };
    let Some(init) = local.init.as_ref() else {
        return Vec::new();
    };
    if let Some(mappings) = bind_param_registers(&local.pat, &init.expr) {
        return mappings;
    }
    bind_simple_alias(&local.pat, &init.expr)
        .into_iter()
        .collect()
}

fn bind_param_registers(pat: &Pat, expr: &Expr) -> Option<Vec<(String, String)>> {
    let names = extract_ident_list_from_pat(pat)?;
    let Expr::Array(ExprArray { elems, .. }) = expr else {
        return None;
    };
    if names.len() != elems.len() {
        return None;
    }
    let mut mappings = Vec::new();
    for (name, expr) in names.into_iter().zip(elems) {
        if let Some(index) = param_index_from_expr(expr) {
            mappings.push((name, format!("a{index}")));
        }
    }
    (!mappings.is_empty()).then_some(mappings)
}

fn bind_simple_alias(pat: &Pat, expr: &Expr) -> Option<(String, String)> {
    let Pat::Ident(PatIdent { ident, .. }) = pat else {
        return None;
    };
    Some((ident.to_string(), render_expr(expr, &BTreeMap::new())))
}

fn extract_ident_list_from_pat(pat: &Pat) -> Option<Vec<String>> {
    match pat {
        Pat::Tuple(tuple) => tuple
            .elems
            .iter()
            .map(|elem| match elem {
                Pat::Ident(ident) => Some(ident.ident.to_string()),
                _ => None,
            })
            .collect(),
        Pat::Slice(slice) => slice
            .elems
            .iter()
            .map(|elem| match elem {
                Pat::Ident(ident) => Some(ident.ident.to_string()),
                _ => None,
            })
            .collect(),
        Pat::Type(PatType { pat, .. }) => extract_ident_list_from_pat(pat),
        _ => None,
    }
}

fn param_index_from_expr(expr: &Expr) -> Option<usize> {
    match expr {
        Expr::Index(ExprIndex { expr, index, .. }) if expr_is_ident(expr, "param") => {
            match &**index {
                Expr::Lit(ExprLit {
                    lit: Lit::Int(value),
                    ..
                }) => value.base10_parse::<usize>().ok(),
                _ => None,
            }
        }
        _ => None,
    }
}

fn find_match_on_ident_in_stmts<'a>(stmts: &'a [Stmt], ident: &str) -> Option<&'a ExprMatch> {
    for stmt in stmts {
        if let Stmt::Expr(Expr::Match(expr_match), _) = stmt
            && expr_is_ident(&expr_match.expr, ident)
        {
            return Some(expr_match);
        }
    }
    None
}

fn extract_self_field_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Field(ExprField {
            base,
            member: Member::Named(member),
            ..
        }) if expr_is_ident(base, "self") => Some(member.to_string()),
        Expr::Paren(ExprParen { expr, .. }) => extract_self_field_name(expr),
        _ => None,
    }
}

fn extract_const_path_from_pat(pat: &Pat) -> Option<String> {
    match pat {
        Pat::Path(PatPath { path, .. }) => Some(
            path.segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("::"),
        ),
        Pat::Type(PatType { pat, .. }) => extract_const_path_from_pat(pat),
        _ => None,
    }
}

fn extract_const_ident_from_pat(pat: &Pat) -> Option<String> {
    extract_const_path_from_pat(pat)?
        .split("::")
        .last()
        .map(ToOwned::to_owned)
}

fn body_is_not_supported(expr: &Expr) -> bool {
    match expr {
        Expr::Call(call) => quote::ToTokens::to_token_stream(&call.func)
            .to_string()
            .replace(' ', "")
            .ends_with("SbiRet::not_supported"),
        Expr::Paren(ExprParen { expr, .. }) => body_is_not_supported(expr),
        _ => false,
    }
}

fn extract_call_argument_expressions(
    expr: &Expr,
    env: &BTreeMap<String, String>,
) -> Option<Vec<String>> {
    match expr {
        Expr::MethodCall(ExprMethodCall { args, .. }) => {
            Some(args.iter().map(|arg| render_expr(arg, env)).collect())
        }
        Expr::Call(ExprCall { func, args, .. }) => {
            let path = quote::ToTokens::to_token_stream(func)
                .to_string()
                .replace(' ', "");
            if path.ends_with("SbiRet::success") || path.ends_with("SbiRet::invalid_param") {
                None
            } else {
                Some(args.iter().map(|arg| render_expr(arg, env)).collect())
            }
        }
        Expr::Block(expr_block) => {
            let mut nested_env = env.clone();
            for stmt in &expr_block.block.stmts {
                for (name, value) in extract_simple_locals(stmt) {
                    nested_env.insert(name, value);
                }
                if let Stmt::Expr(expr, _) = stmt
                    && let Some(result) = extract_call_argument_expressions(expr, &nested_env)
                {
                    return Some(result);
                }
            }
            None
        }
        Expr::Match(expr_match) => {
            for arm in &expr_match.arms {
                if body_is_not_supported(&arm.body) {
                    continue;
                }
                if let Some(result) = extract_call_argument_expressions(&arm.body, env) {
                    return Some(result);
                }
            }
            None
        }
        Expr::Paren(ExprParen { expr, .. }) => extract_call_argument_expressions(expr, env),
        _ => None,
    }
}

fn render_expr(expr: &Expr, env: &BTreeMap<String, String>) -> String {
    let rendered = expr.to_token_stream().to_string();
    let mut replacements: Vec<_> = env.iter().collect();
    replacements.sort_by_key(|(name, _)| usize::MAX - name.len());
    let mut replaced = String::with_capacity(rendered.len());
    let mut current = String::new();

    for ch in rendered.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            current.push(ch);
            continue;
        }
        if !current.is_empty() {
            replaced.push_str(&replace_identifier(&current, &replacements));
            current.clear();
        }
        replaced.push(ch);
    }
    if !current.is_empty() {
        replaced.push_str(&replace_identifier(&current, &replacements));
    }
    normalize_whitespace(&replaced)
}

fn replace_identifier(token: &str, replacements: &[(&String, &String)]) -> String {
    for (name, value) in replacements {
        if name.as_str() == token {
            return (*value).clone();
        }
    }
    token.to_string()
}

fn collect_registers(rendered: &str) -> BTreeSet<String> {
    register_regex()
        .captures_iter(rendered)
        .map(|capture| format!("a{}", &capture[1]))
        .collect()
}

fn register_regex() -> &'static Regex {
    static REGISTER_REGEX: OnceLock<Regex> = OnceLock::new();
    REGISTER_REGEX.get_or_init(|| {
        Regex::new(r"\ba([0-5])\b").expect("register regex pattern must stay valid")
    })
}

fn normalize_whitespace(value: &str) -> String {
    let mut normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    for (from, to) in [
        (" ( ", "("),
        (" )", ")"),
        ("[ ", "["),
        (" ]", "]"),
        (" , ", ", "),
        (" :: ", "::"),
    ] {
        normalized = normalized.replace(from, to);
    }
    normalized
}

fn expr_is_ident(expr: &Expr, ident: &str) -> bool {
    matches!(
        expr,
        Expr::Path(ExprPath { path, .. })
            if path.segments.len() == 1 && path.segments[0].ident == ident
    )
}

fn is_extension_probe_impl(item_impl: &ItemImpl) -> bool {
    type_to_string(item_impl.self_ty.as_ref()).as_deref() == Some("_StandardExtensionProbe")
        && item_impl.trait_.as_ref().map(|(_, path, _)| {
            path.segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("::")
        }) == Some("_ExtensionProbe".to_string())
}

fn type_to_string(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(TypePath { path, .. }) => Some(
            path.segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("::"),
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_probe_and_dispatch_helpers() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("traits_parser_{unique}"));
        let traits_dir = root.join("library/rustsbi/src");
        fs::create_dir_all(&traits_dir).unwrap();
        fs::write(
            traits_dir.join("traits.rs"),
            r#"
            pub trait _ExtensionProbe { fn probe_extension(&self, extension: usize) -> usize; }
            pub struct _StandardExtensionProbe { pub base: usize, pub timer: usize }
            impl _ExtensionProbe for _StandardExtensionProbe {
                fn probe_extension(&self, extension: usize) -> usize {
                    match extension {
                        spec::base::EID_BASE => self.base,
                        spec::time::EID_TIME => self.timer,
                        _ => 0,
                    }
                }
            }
            pub fn _rustsbi_base_env_info<T, U>(param: [usize; 6], function: usize, env_info: &T, probe: U) -> SbiRet {
                let [param0] = [param[0]];
                let value = match function {
                    spec::base::PROBE_EXTENSION => probe.probe_extension(param0),
                    _ => return SbiRet::not_supported(),
                };
                SbiRet::success(value)
            }
            pub fn _rustsbi_timer<T>(timer: &T, param: [usize; 6], function: usize) -> SbiRet {
                let [param0, param1] = [param[0], param[1]];
                match function {
                    spec::time::SET_TIMER => { timer.set_timer(concat_u32(param1, param0)); SbiRet::success(0) }
                    _ => SbiRet::not_supported(),
                }
            }
            "#,
        )
        .unwrap();

        let parsed = parse_traits_repository(&root).unwrap();
        assert_eq!(parsed.dispatchers.len(), 2);
        assert_eq!(parsed.dispatchers["time"].helper_name, "_rustsbi_timer");
        let dispatch = &parsed.dispatchers["time"].functions["SET_TIMER"];
        assert!(dispatch.used_registers.contains("a0"));
        assert!(dispatch.used_registers.contains("a1"));

        fs::remove_dir_all(root).unwrap();
    }
}
