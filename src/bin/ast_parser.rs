//! CLI entrypoint for the AST-based `sbi_interfaces.json` extractor.

use rusy_analyzer::ast::scan_repository;
use rusy_analyzer::utils::error::AnalyzerError;
use rusy_analyzer::utils::fs::write_json;
use std::env;
use std::path::PathBuf;

const DEFAULT_TARGET_REPO: &str = "target_repo/rustsbi";
const DEFAULT_OUTPUT_PATH: &str = "sbi_interfaces.json";

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), AnalyzerError> {
    let mut args = env::args().skip(1);
    let repo_root = PathBuf::from(
        args.next()
            .unwrap_or_else(|| DEFAULT_TARGET_REPO.to_string()),
    );
    let output_path = PathBuf::from(
        args.next()
            .unwrap_or_else(|| DEFAULT_OUTPUT_PATH.to_string()),
    );

    let report = scan_repository(&repo_root)?;
    write_json(&output_path, &report)?;
    println!("generated {}", output_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusy_analyzer::models::sbi_interface::InterfaceReport;
    use rusy_analyzer::utils::fs::read_text;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn writes_end_to_end_json_output() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ast_bin_{unique}"));
        let spec_dir = root.join("library/sbi-spec/src");
        let traits_dir = root.join("library/rustsbi/src");
        let macros_dir = root.join("library/macros/src");
        fs::create_dir_all(&spec_dir).unwrap();
        fs::create_dir_all(&traits_dir).unwrap();
        fs::create_dir_all(&macros_dir).unwrap();

        fs::write(spec_dir.join("lib.rs"), "pub mod time;\n").unwrap();
        fs::write(
            spec_dir.join("time.rs"),
            r#"pub const EID_TIME: usize = crate::eid_from_str("TIME") as _;
               mod fid { pub const SET_TIMER: usize = 0; }"#,
        )
        .unwrap();
        fs::write(
            traits_dir.join("traits.rs"),
            r#"
            pub trait _ExtensionProbe { fn probe_extension(&self, extension: usize) -> usize; }
            pub struct _StandardExtensionProbe { pub timer: usize }
            impl _ExtensionProbe for _StandardExtensionProbe {
                fn probe_extension(&self, extension: usize) -> usize {
                    match extension {
                        spec::time::EID_TIME => self.timer,
                        _ => 0,
                    }
                }
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
            r#"match_arms.extend(quote! {
                ::rustsbi::spec::time::EID_TIME => ::rustsbi::_rustsbi_timer(&self.#timer, param, function),
            });
            let base = ::rustsbi::spec::base::EID_BASE => ::rustsbi::_rustsbi_base_env_info(param, function, &self.#env_info, #probe);
            fn probe_extension(&self, extension: usize) -> usize {
                match extension {
                    ::rustsbi::spec::base::EID_BASE => 1,
                    ::rustsbi::spec::time::EID_TIME => { let value = ::rustsbi::_rustsbi_timer_probe(&self.0.#timer); value },
                    _ => ::rustsbi::spec::base::UNAVAILABLE_EXTENSION,
                }
            }"#,
        )
        .unwrap();

        let output = root.join("sbi_interfaces.json");
        let report = scan_repository(&root).unwrap();
        write_json(&output, &report).unwrap();
        let json = read_text(&output).unwrap();
        let parsed: InterfaceReport = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.extensions.len(), 1);
        assert_eq!(parsed.extensions[0].extension_name, "TIME");
        assert!(json.contains("\"dispatcher_helper\": \"_rustsbi_timer\""));

        fs::remove_dir_all(root).unwrap();
    }
}
