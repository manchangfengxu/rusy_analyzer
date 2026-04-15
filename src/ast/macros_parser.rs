//! Parsing of real `#[derive(RustSBI)]` quote templates from `library/macros/src/lib.rs`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use regex::Regex;

use crate::utils::error::{AnalyzerError, ScanWarning};
use crate::utils::fs::read_text;

#[derive(Debug)]
pub(crate) struct ParsedMacroRoutes {
    pub routes: BTreeMap<String, BTreeSet<String>>,
    pub warnings: Vec<ScanWarning>,
}

/// Parses derive-macro routing templates from the RustSBI macros crate.
///
/// The parser is intentionally grounded in the real `quote!` source text rather than a guessed
/// post-expansion shape. It extracts `spec::<module>::EID_* => _rustsbi_*` route pairs and
/// confirms that both direct dispatch templates and probe-related templates are present.
pub(crate) fn parse_macro_routes(repo_root: &Path) -> Result<ParsedMacroRoutes, AnalyzerError> {
    let path = repo_root.join("library/macros/src/lib.rs");
    let source = read_text(&path)?;
    let mut routes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let warnings = Vec::new();

    let static_route = Regex::new(
        r"::rustsbi::spec::([a-z0-9_]+)::EID_[A-Z0-9_]+\s*=>\s*::rustsbi::(_rustsbi_[a-z0-9_]+)",
    )
    .map_err(|err| {
        AnalyzerError::new(
            path.display().to_string(),
            format!("failed to compile derive route regex: {err}"),
        )
    })?;

    for captures in static_route.captures_iter(&source) {
        let module = captures[1].to_string();
        let helper = captures[2].to_string();
        routes.entry(module).or_default().insert(helper);
    }

    let has_base_dispatch = source
        .contains("::rustsbi::spec::base::EID_BASE => ::rustsbi::_rustsbi_base_env_info")
        || source.contains("::rustsbi::spec::base::EID_BASE => ::rustsbi::_rustsbi_base_bare");
    let has_probe_impl = source.contains("fn probe_extension(&self, extension: usize) -> usize")
        && source.contains("::rustsbi::spec::base::EID_BASE => 1");
    let has_helper_probe = source.contains("_probe(&self.")
        || source.contains("_probe(&self.#")
        || source.contains("_probe(&self.0.");

    if !has_base_dispatch || !has_probe_impl || !has_helper_probe {
        return Err(AnalyzerError::new(
            path.display().to_string(),
            "failed to confirm expected static/dynamic derive helper templates",
        ));
    }

    if routes.is_empty() {
        return Err(AnalyzerError::new(
            path.display().to_string(),
            "failed to extract any derive helper routes from macros source",
        ));
    }

    Ok(ParsedMacroRoutes { routes, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn extracts_routes_from_macro_templates() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("macros_parser_{unique}"));
        let macros_dir = root.join("library/macros/src");
        fs::create_dir_all(&macros_dir).unwrap();
        fs::write(
            macros_dir.join("lib.rs"),
            r#"
            match_arms.extend(quote! {
                ::rustsbi::spec::time::EID_TIME => ::rustsbi::_rustsbi_timer(&self.#timer, param, function),
                ::rustsbi::spec::rfnc::EID_RFNC => ::rustsbi::_rustsbi_fence(&self.#fence, param, function),
            });
            let base = quote! {
                ::rustsbi::spec::base::EID_BASE => ::rustsbi::_rustsbi_base_env_info(param, function, &self.#env_info, #probe),
            };
            fn probe_extension(&self, extension: usize) -> usize {
                match extension {
                    ::rustsbi::spec::base::EID_BASE => 1,
                    ::rustsbi::spec::time::EID_TIME => { let value = ::rustsbi::_rustsbi_timer_probe(&self.0.#timer); value },
                    ::rustsbi::spec::rfnc::EID_RFNC => { let value = ::rustsbi::_rustsbi_fence_probe(&self.0.#fence); value },
                    _ => ::rustsbi::spec::base::UNAVAILABLE_EXTENSION,
                }
            }
            "#,
        )
        .unwrap();

        let parsed = parse_macro_routes(&root).unwrap();
        assert!(parsed.routes["time"].contains("_rustsbi_timer"));
        assert!(parsed.routes["rfnc"].contains("_rustsbi_fence"));
        assert!(parsed.routes["base"].contains("_rustsbi_base_env_info"));
        assert!(parsed.warnings.is_empty());

        fs::remove_dir_all(root).unwrap();
    }
}
