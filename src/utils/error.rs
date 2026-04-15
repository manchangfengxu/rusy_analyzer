//! Shared error and warning types used by AST and MIR analyzers.

use std::error::Error;
use std::fmt::{Display, Formatter};

/// Structured error with contextual location information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzerError {
    /// File path or logical context where the error originated.
    pub context: String,
    /// Human-readable explanation of the failure.
    pub message: String,
}

impl AnalyzerError {
    /// Creates a new analyzer error with explicit context and message.
    pub fn new(context: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            context: context.into(),
            message: message.into(),
        }
    }
}

impl Display for AnalyzerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.context, self.message)
    }
}

impl Error for AnalyzerError {}

/// Non-fatal warning with contextual location information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanWarning {
    /// File path or logical context where the warning originated.
    pub context: String,
    /// Human-readable explanation of the warning.
    pub message: String,
}

impl ScanWarning {
    /// Creates a new scan warning with explicit context and message.
    pub fn new(context: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            context: context.into(),
            message: message.into(),
        }
    }
}

impl Display for ScanWarning {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.context, self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_error_and_warning() {
        let error = AnalyzerError::new("mock.mir:3", "broken block");
        let warning = ScanWarning::new("mock.mir:4", "unknown assert");

        assert_eq!(error.to_string(), "mock.mir:3: broken block");
        assert_eq!(warning.to_string(), "mock.mir:4: unknown assert");
    }
}
