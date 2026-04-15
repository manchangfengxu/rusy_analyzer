//! AST-based extraction of SBI extension interfaces from RustSBI sources.
//!
//! This subsystem joins three real sources of truth:
//! `library/sbi-spec` for EID/FID constants, `library/rustsbi/src/traits.rs` for helper dispatch
//! behavior, and `library/macros/src/lib.rs` for derive-macro routing templates.

mod macros_parser;
mod spec_parser;
mod traits_parser;

use std::path::Path;

use macros_parser::parse_macro_routes;
use spec_parser::parse_spec_repository;
use traits_parser::{DispatchInfo, parse_traits_repository};

use crate::models::sbi_interface::{ExtensionInterface, FunctionInterface, InterfaceReport};
use crate::utils::error::AnalyzerError;

/// Scans a RustSBI checkout and builds the `sbi_interfaces.json` report.
///
/// # Errors
///
/// Returns an error when the RustSBI source tree is missing required files or when a supported
/// AST construct cannot be parsed into the expected interface representation.
pub fn scan_repository(repo_root: &Path) -> Result<InterfaceReport, AnalyzerError> {
    let spec = parse_spec_repository(repo_root)?;
    let traits = parse_traits_repository(repo_root)?;
    let macro_routes = parse_macro_routes(repo_root)?;

    for warning in &spec.warnings {
        eprintln!("warning: {warning}");
    }
    for warning in &traits.warnings {
        eprintln!("warning: {warning}");
    }
    for warning in &macro_routes.warnings {
        eprintln!("warning: {warning}");
    }

    let mut extensions = Vec::new();
    for extension in spec.extensions {
        let Some(dispatch) = traits.dispatchers.get(&extension.module) else {
            eprintln!(
                "warning: extension '{}' from '{}' has no matching RustSBI dispatcher helper",
                extension.extension_name, extension.source_file
            );
            continue;
        };
        validate_macro_binding(&extension.module, dispatch, &macro_routes.routes)?;
        let mut functions = Vec::new();
        let mut missing = Vec::new();

        for (function_name, function_id) in &extension.functions {
            let Some(dispatch_info) = dispatch.functions.get(function_name) else {
                missing.push(function_name.clone());
                continue;
            };
            functions.push(FunctionInterface {
                function_name: function_name.clone(),
                function_id_hex: format!("0x{function_id:X}"),
                used_registers: dispatch_info.used_registers.iter().cloned().collect(),
                argument_expressions: dispatch_info.argument_expressions.iter().cloned().collect(),
            });
        }

        if !missing.is_empty() {
            eprintln!(
                "warning: dispatcher '{}' does not implement {} functions from extension '{}': {}",
                dispatch.helper_name,
                missing.len(),
                extension.extension_name,
                missing.join(", ")
            );
        }

        if functions.is_empty() {
            return Err(AnalyzerError::new(
                extension.source_file.clone(),
                format!(
                    "dispatcher '{}' has no overlapping function IDs with extension '{}'",
                    dispatch.helper_name, extension.extension_name
                ),
            ));
        }

        extensions.push(ExtensionInterface {
            module: extension.module,
            extension_name: extension.extension_name,
            eid_constant: extension.eid_constant,
            eid_value_hex: format!("0x{:X}", extension.eid_value),
            source_file: extension.source_file,
            dispatcher_helper: dispatch.helper_name.clone(),
            functions,
        });
    }

    Ok(InterfaceReport::new(
        repo_root.display().to_string(),
        extensions,
    ))
}

fn validate_macro_binding(
    module: &str,
    dispatch: &DispatchInfo,
    routes: &std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
) -> Result<(), AnalyzerError> {
    let Some(route_helpers) = routes.get(module) else {
        return Err(AnalyzerError::new(
            "library/macros/src/lib.rs",
            format!("missing derive route for extension module '{module}'"),
        ));
    };

    if !route_helpers.contains(&dispatch.helper_name) {
        return Err(AnalyzerError::new(
            "library/macros/src/lib.rs",
            format!(
                "derive route for extension module '{module}' does not include expected helper '{}' (available: {})",
                dispatch.helper_name,
                route_helpers.iter().cloned().collect::<Vec<_>>().join(", ")
            ),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::fs::write_json;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn scans_mock_repository_end_to_end() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ast_scan_repo_{unique}"));
        let spec_dir = root.join("library/sbi-spec/src");
        let rustsbi_dir = root.join("library/rustsbi/src");
        let macros_dir = root.join("library/macros/src");
        fs::create_dir_all(&spec_dir).unwrap();
        fs::create_dir_all(&rustsbi_dir).unwrap();
        fs::create_dir_all(&macros_dir).unwrap();

        fs::write(spec_dir.join("lib.rs"), "pub mod base;\npub mod time;\n").unwrap();
        fs::write(
            spec_dir.join("base.rs"),
            r#"
            pub const EID_BASE: usize = 0x10;
            mod fid { pub const PROBE_EXTENSION: usize = 3; }
            "#,
        )
        .unwrap();
        fs::write(
            spec_dir.join("time.rs"),
            r#"
            pub const EID_TIME: usize = crate::eid_from_str("TIME") as _;
            mod fid { pub const SET_TIMER: usize = 0; }
            "#,
        )
        .unwrap();
        fs::write(
            rustsbi_dir.join("traits.rs"),
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
                let [param0] = [param[0]];
                match function {
                    spec::time::SET_TIMER => { timer.set_timer(param0 as _); SbiRet::success(0) }
                    _ => SbiRet::not_supported(),
                }
            }
            "#,
        )
        .unwrap();
        fs::write(
            macros_dir.join("lib.rs"),
            r#"
            match_arms.extend(quote! {
                ::rustsbi::spec::time::EID_TIME => ::rustsbi::_rustsbi_timer(&self.#timer, param, function),
            });
            let base_procedure = quote! {
                ::rustsbi::spec::base::EID_BASE => ::rustsbi::_rustsbi_base_env_info(param, function, &self.#env_info, #probe),
            };
            fn probe_extension(&self, extension: usize) -> usize {
                match extension {
                    ::rustsbi::spec::base::EID_BASE => 1,
                    ::rustsbi::spec::time::EID_TIME => { let value = ::rustsbi::_rustsbi_timer_probe(&self.0.#timer); value },
                    _ => ::rustsbi::spec::base::UNAVAILABLE_EXTENSION,
                }
            }
            "#,
        )
        .unwrap();

        let report = scan_repository(&root).unwrap();
        assert_eq!(report.extensions.len(), 2);
        assert_eq!(report.extensions[0].extension_name, "BASE");
        assert_eq!(report.extensions[1].functions[0].used_registers, vec!["a0"]);

        let output = root.join("sbi_interfaces.json");
        write_json(&output, &report).unwrap();
        let json = fs::read_to_string(&output).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(value.get("extensions").is_some());

        fs::remove_dir_all(root).unwrap();
    }
}
