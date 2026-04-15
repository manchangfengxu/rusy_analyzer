//! CLI entrypoint for the MIR-based `panic_sites.json` scanner.

use rusy_analyzer::mir::scan_mir_file;
use rusy_analyzer::utils::error::AnalyzerError;
use rusy_analyzer::utils::fs::write_json;
use std::env;
use std::path::PathBuf;

const DEFAULT_MIR_PATH: &str = "sbi.mir";
const DEFAULT_OUTPUT_PATH: &str = "panic_sites.json";

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), AnalyzerError> {
    let mut args = env::args().skip(1);
    let mir_path = PathBuf::from(args.next().unwrap_or_else(|| DEFAULT_MIR_PATH.to_string()));
    let output_path = PathBuf::from(
        args.next()
            .unwrap_or_else(|| DEFAULT_OUTPUT_PATH.to_string()),
    );

    let report = scan_mir_file(&mir_path)?;
    write_json(&output_path, &report)?;
    println!("generated {}", output_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusy_analyzer::models::panic_site::PanicSiteReport;
    use rusy_analyzer::utils::fs::read_text;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn writes_end_to_end_json_output() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mir_path = std::env::temp_dir().join(format!("panic_scanner_{unique}.mir"));
        let output_path = std::env::temp_dir().join(format!("panic_scanner_{unique}.json"));
        fs::write(
            &mir_path,
            r#"
fn demo::panic() -> () {
    bb2: {
        assert(!move _1, "attempt to multiply with overflow") -> [success: bb3, unwind unreachable];
    }
}
"#,
        )
        .unwrap();

        let report = scan_mir_file(&mir_path).unwrap();
        write_json(&output_path, &report).unwrap();
        let json = read_text(&output_path).unwrap();
        let parsed: PanicSiteReport = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.summary.categories.ae, 1);
        assert_eq!(parsed.sites[0].basic_block, "bb2");
        assert!(json.contains("\"category\": \"AE\""));
        assert!(json.contains("\"panic_kind\": \"multiply_overflow\""));

        fs::remove_file(mir_path).unwrap();
        fs::remove_file(output_path).unwrap();
    }
}
