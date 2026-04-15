use serde::{Deserialize, Serialize};

/// Top-level JSON report for the AST interface extractor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceReport {
    /// Path to the analyzed RustSBI repository root.
    pub target_repository: String,
    /// Register used for the SBI extension ID.
    pub extension_register: String,
    /// Register used for the SBI function ID.
    pub function_register: String,
    /// Ordered SBI argument registers consumed by calls.
    pub parameter_registers: Vec<String>,
    /// Extracted extension interface definitions.
    pub extensions: Vec<ExtensionInterface>,
}

/// One SBI extension and its routed helper information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionInterface {
    /// Module name in `library/sbi-spec/src/<module>.rs`.
    pub module: String,
    /// Uppercase extension name derived from `EID_*`.
    pub extension_name: String,
    /// Extension identifier constant name, such as `EID_TIME`.
    pub eid_constant: String,
    /// Hexadecimal extension identifier value, such as `0x54494D45`.
    pub eid_value_hex: String,
    /// Source file where the extension constants were parsed.
    pub source_file: String,
    /// Dispatcher helper determined from RustSBI helper routing.
    pub dispatcher_helper: String,
    /// Functions emitted for the extension.
    pub functions: Vec<FunctionInterface>,
}

/// One routed function under an SBI extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionInterface {
    /// Function identifier constant name, such as `SET_TIMER`.
    pub function_name: String,
    /// Hexadecimal function identifier value.
    pub function_id_hex: String,
    /// Best-effort summary of which SBI argument registers were observed.
    pub used_registers: Vec<String>,
    /// Best-effort rendered call argument expressions from helper code.
    pub argument_expressions: Vec<String>,
}

impl InterfaceReport {
    /// Builds a new interface report with the standard SBI calling convention registers.
    pub fn new(target_repository: String, extensions: Vec<ExtensionInterface>) -> Self {
        Self {
            target_repository,
            extension_register: "a7".to_string(),
            function_register: "a6".to_string(),
            parameter_registers: ["a0", "a1", "a2", "a3", "a4", "a5"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            extensions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_default_register_contract() {
        let report = InterfaceReport::new("repo".to_string(), Vec::new());
        assert_eq!(report.extension_register, "a7");
        assert_eq!(report.function_register, "a6");
        assert_eq!(report.parameter_registers.len(), 6);
        assert_eq!(report.parameter_registers[0], "a0");
    }
}
