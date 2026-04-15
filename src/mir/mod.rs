//! MIR scanning facade for `-Z unpretty=mir` text inputs.

mod classifier;
mod parser;
mod patterns;

use std::path::Path;

use crate::models::panic_site::PanicSiteReport;
use crate::utils::error::AnalyzerError;
use crate::utils::fs::read_text;

/// Scans already-loaded `unpretty MIR` text and produces a structured panic-site report.
///
/// The accepted input contract is the textual output of commands such as
/// `cargo rustc -- -Z unpretty=mir`.
///
/// # Errors
///
/// Returns an error when the MIR text cannot be parsed structurally or when internal regex-based
/// classification patterns fail to compile.
pub fn scan_mir_text(mir_text: &str, mir_path: &Path) -> Result<PanicSiteReport, AnalyzerError> {
    let parsed = parser::parse_mir_text(mir_text, mir_path)?;
    for warning in &parsed.warnings {
        eprintln!("warning: {warning}");
    }

    let classified = classifier::classify_blocks(parsed.blocks, mir_path)?;
    for warning in &classified.warnings {
        eprintln!("warning: {warning}");
    }
    if classified.unknown_site_count > 0 {
        eprintln!(
            "warning: {} unknown panic site(s) were emitted into panic_sites.json",
            classified.unknown_site_count
        );
    }

    Ok(PanicSiteReport::new(
        mir_path.display().to_string(),
        classified.sites,
    ))
}

/// Reads one `unpretty MIR` file from disk and scans it for panic-oriented basic blocks.
///
/// # Errors
///
/// Returns an error when the MIR file cannot be read or when the textual MIR content fails
/// structural parsing or classification setup.
pub fn scan_mir_file(mir_path: &Path) -> Result<PanicSiteReport, AnalyzerError> {
    let mir_text = read_text(mir_path)?;
    scan_mir_text(&mir_text, mir_path)
}
