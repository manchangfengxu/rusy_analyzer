//! File-system helpers for reading source text and writing JSON reports.

use serde::Serialize;
use std::fs;
use std::path::Path;

use crate::utils::error::AnalyzerError;

/// Reads a UTF-8 text file into memory.
///
/// # Errors
///
/// Returns an error when the file cannot be read as UTF-8 text.
pub fn read_text(path: &Path) -> Result<String, AnalyzerError> {
    fs::read_to_string(path).map_err(|err| {
        AnalyzerError::new(
            path.display().to_string(),
            format!("failed to read source file: {err}"),
        )
    })
}

/// Serializes one value as pretty JSON and writes it to disk.
///
/// # Errors
///
/// Returns an error when serialization fails or when the destination file cannot be written.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), AnalyzerError> {
    let json = serde_json::to_string_pretty(value).map_err(|err| {
        AnalyzerError::new(
            path.display().to_string(),
            format!("failed to serialize JSON output: {err}"),
        )
    })?;
    fs::write(path, json).map_err(|err| {
        AnalyzerError::new(
            path.display().to_string(),
            format!("failed to write JSON output to disk: {err}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Serialize)]
    struct TestValue {
        value: usize,
    }

    #[test]
    fn reads_and_writes_files() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let text_path = std::env::temp_dir().join(format!("fs_test_{unique}.txt"));
        let json_path = std::env::temp_dir().join(format!("fs_test_{unique}.json"));

        fs::write(&text_path, "hello").unwrap();
        let text = read_text(&text_path).unwrap();
        assert_eq!(text, "hello");

        write_json(&json_path, &TestValue { value: 7 }).unwrap();
        let json = fs::read_to_string(&json_path).unwrap();
        assert!(json.contains("\"value\": 7"));

        fs::remove_file(text_path).unwrap();
        fs::remove_file(json_path).unwrap();
    }
}
