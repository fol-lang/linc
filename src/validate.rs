use serde::{Deserialize, Serialize};

use crate::ir::{BindingItem, BindingPackage};
use crate::symbols::SymbolInventory;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchStatus {
    Matched,
    Missing,
    NotAFunction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionMatch {
    pub name: String,
    pub status: MatchStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub matches: Vec<FunctionMatch>,
}

impl ValidationReport {
    pub fn matched(&self) -> Vec<&FunctionMatch> {
        self.matches
            .iter()
            .filter(|m| m.status == MatchStatus::Matched)
            .collect()
    }

    pub fn missing(&self) -> Vec<&FunctionMatch> {
        self.matches
            .iter()
            .filter(|m| m.status == MatchStatus::Missing)
            .collect()
    }

    pub fn all_matched(&self) -> bool {
        self.matches.iter().all(|m| m.status == MatchStatus::Matched)
    }
}

pub fn validate(package: &BindingPackage, inventory: &SymbolInventory) -> ValidationReport {
    let mut matches = Vec::new();

    for item in &package.items {
        match item {
            BindingItem::Function(f) => {
                let status = if inventory.has_symbol(&f.name) {
                    // Verify it's actually a function symbol
                    let sym = inventory
                        .symbols
                        .iter()
                        .find(|s| s.name == f.name)
                        .unwrap();
                    if sym.is_function {
                        MatchStatus::Matched
                    } else {
                        MatchStatus::NotAFunction
                    }
                } else {
                    MatchStatus::Missing
                };
                matches.push(FunctionMatch {
                    name: f.name.clone(),
                    status,
                });
            }
            _ => {}
        }
    }

    ValidationReport { matches }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::*;
    use crate::symbols::*;

    fn make_inventory(funcs: &[&str], data: &[&str]) -> SymbolInventory {
        let mut symbols = Vec::new();
        for name in funcs {
            symbols.push(SymbolEntry {
                name: name.to_string(),
                visibility: SymbolVisibility::Default,
                is_function: true,
            });
        }
        for name in data {
            symbols.push(SymbolEntry {
                name: name.to_string(),
                visibility: SymbolVisibility::Default,
                is_function: false,
            });
        }
        SymbolInventory {
            artifact_path: "test.o".into(),
            format: ArtifactFormat::ElfObject,
            symbols,
        }
    }

    fn make_package(func_names: &[&str]) -> BindingPackage {
        let items = func_names
            .iter()
            .map(|name| {
                BindingItem::Function(FunctionBinding {
                    name: name.to_string(),
                    calling_convention: CallingConvention::C,
                    parameters: Vec::new(),
                    return_type: BindingType::Void,
                    variadic: false,
                    source_offset: None,
                })
            })
            .collect();
        BindingPackage {
            source_path: None,
            items,
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn all_functions_matched() {
        let inv = make_inventory(&["foo", "bar"], &[]);
        let pkg = make_package(&["foo", "bar"]);
        let report = validate(&pkg, &inv);
        assert!(report.all_matched());
        assert_eq!(report.matched().len(), 2);
        assert_eq!(report.missing().len(), 0);
    }

    #[test]
    fn some_functions_missing() {
        let inv = make_inventory(&["foo"], &[]);
        let pkg = make_package(&["foo", "bar", "baz"]);
        let report = validate(&pkg, &inv);
        assert!(!report.all_matched());
        assert_eq!(report.matched().len(), 1);
        assert_eq!(report.missing().len(), 2);
    }

    #[test]
    fn symbol_exists_but_not_function() {
        let inv = make_inventory(&[], &["data_sym"]);
        let pkg = make_package(&["data_sym"]);
        let report = validate(&pkg, &inv);
        assert!(!report.all_matched());
        assert_eq!(report.matches[0].status, MatchStatus::NotAFunction);
    }

    #[test]
    fn empty_package() {
        let inv = make_inventory(&["foo"], &[]);
        let pkg = make_package(&[]);
        let report = validate(&pkg, &inv);
        assert!(report.all_matched()); // vacuously true
        assert_eq!(report.matches.len(), 0);
    }

    #[test]
    fn non_function_items_ignored() {
        let inv = make_inventory(&["foo"], &[]);
        let mut pkg = make_package(&["foo"]);
        pkg.items.push(BindingItem::TypeAlias(TypeAliasBinding {
            name: "my_type".into(),
            target: BindingType::Int,
            source_offset: None,
        }));
        let report = validate(&pkg, &inv);
        assert_eq!(report.matches.len(), 1); // only the function
        assert!(report.all_matched());
    }

    #[test]
    fn report_serialization() {
        let inv = make_inventory(&["foo"], &[]);
        let pkg = make_package(&["foo", "missing"]);
        let report = validate(&pkg, &inv);
        let json = serde_json::to_string(&report).unwrap();
        let report2: ValidationReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report, report2);
    }

    /// End-to-end: parse C, compile it, validate symbols.
    #[test]
    #[ignore] // Requires cc
    fn end_to_end_validation() {
        let c_src = "int add(int a, int b) { return a + b; }\nint mul(int a, int b) { return a * b; }\n";
        let dir = std::env::temp_dir().join("bic_validate_test");
        std::fs::create_dir_all(&dir).unwrap();
        let c_path = dir.join("funcs.c");
        let o_path = dir.join("funcs.o");
        std::fs::write(&c_path, c_src).unwrap();

        let status = std::process::Command::new("cc")
            .args(["-c", "-o"])
            .arg(&o_path)
            .arg(&c_path)
            .status()
            .expect("cc not found");
        assert!(status.success());

        // Parse declarations
        let header = "int add(int a, int b); int mul(int a, int b); int missing_func(void);";
        let pkg = crate::extract_from_source(header).unwrap();

        // Inspect symbols
        let inv = crate::symbols::inspect_file(&o_path).unwrap();

        // Validate
        let report = validate(&pkg, &inv);
        assert_eq!(report.matched().len(), 2);
        assert_eq!(report.missing().len(), 1);
        assert_eq!(report.missing()[0].name, "missing_func");

        std::fs::remove_file(&c_path).ok();
        std::fs::remove_file(&o_path).ok();
        std::fs::remove_dir(&dir).ok();
    }
}
