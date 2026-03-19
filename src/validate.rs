use serde::{Deserialize, Serialize};

use crate::ir::{BindingItem, BindingPackage};
use crate::symbols::{SymbolBinding, SymbolInventory, SymbolVisibility};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemKind {
    Function,
    Variable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchStatus {
    Matched,
    Missing,
    NotAFunction,
    NotAVariable,
    Hidden,
    WeakMatch,
}

/// Renamed from FunctionMatch to support both functions and variables.
pub type FunctionMatch = SymbolMatch;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolMatch {
    pub name: String,
    pub item_kind: ItemKind,
    pub status: MatchStatus,
    pub visibility: Option<SymbolVisibility>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub matches: Vec<SymbolMatch>,
}

impl ValidationReport {
    pub fn matched(&self) -> Vec<&SymbolMatch> {
        self.matches
            .iter()
            .filter(|m| m.status == MatchStatus::Matched)
            .collect()
    }

    pub fn missing(&self) -> Vec<&SymbolMatch> {
        self.matches
            .iter()
            .filter(|m| m.status == MatchStatus::Missing)
            .collect()
    }

    pub fn hidden(&self) -> Vec<&SymbolMatch> {
        self.matches
            .iter()
            .filter(|m| m.status == MatchStatus::Hidden)
            .collect()
    }

    pub fn all_matched(&self) -> bool {
        self.matches
            .iter()
            .all(|m| m.status == MatchStatus::Matched)
    }
}

pub fn validate(package: &BindingPackage, inventory: &SymbolInventory) -> ValidationReport {
    let mut matches = Vec::new();

    for item in &package.items {
        let (name, kind, expect_function) = match item {
            BindingItem::Function(f) => (&f.name, ItemKind::Function, true),
            BindingItem::Variable(v) => (&v.name, ItemKind::Variable, false),
            _ => continue,
        };

        let sym = inventory.symbols.iter().find(|s| &s.name == name);
        let (status, visibility) = match sym {
            Some(s) => {
                let vis = s.visibility.clone();
                if matches!(vis, SymbolVisibility::Hidden | SymbolVisibility::Internal) {
                    (MatchStatus::Hidden, Some(vis))
                } else if expect_function && !s.is_function {
                    (MatchStatus::NotAFunction, Some(vis))
                } else if !expect_function && s.is_function {
                    (MatchStatus::NotAVariable, Some(vis))
                } else if s.binding == SymbolBinding::Weak {
                    (MatchStatus::WeakMatch, Some(vis))
                } else {
                    (MatchStatus::Matched, Some(vis))
                }
            }
            None => (MatchStatus::Missing, None),
        };

        matches.push(SymbolMatch {
            name: name.clone(),
            item_kind: kind,
            status,
            visibility,
        });
    }

    ValidationReport { matches }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::*;
    use crate::symbols::*;

    fn make_inventory_with_vis(
        entries: &[(&str, bool, SymbolVisibility)],
    ) -> SymbolInventory {
        let symbols = entries
            .iter()
            .map(|(name, is_func, vis)| SymbolEntry {
                name: name.to_string(),
                raw_name: None,
                visibility: vis.clone(),
                is_function: *is_func,
                binding: SymbolBinding::Global,
                size: None,
                section: None,
                archive_member: None,
            })
            .collect();
        SymbolInventory {
            artifact_path: "test.o".into(),
            format: ArtifactFormat::ElfObject,
            symbols,
        }
    }

    fn make_inventory(funcs: &[&str], data: &[&str]) -> SymbolInventory {
        let mut entries: Vec<(&str, bool, SymbolVisibility)> = Vec::new();
        for name in funcs {
            entries.push((name, true, SymbolVisibility::Default));
        }
        for name in data {
            entries.push((name, false, SymbolVisibility::Default));
        }
        make_inventory_with_vis(&entries)
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
            ..BindingPackage::new()
        }
    }

    fn make_package_with_vars(
        func_names: &[&str],
        var_names: &[&str],
    ) -> BindingPackage {
        let mut items: Vec<BindingItem> = func_names
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
        for name in var_names {
            items.push(BindingItem::Variable(VariableBinding {
                name: name.to_string(),
                ty: BindingType::Int,
                source_offset: None,
            }));
        }
        BindingPackage {
            source_path: None,
            items,
            diagnostics: Vec::new(),
            ..BindingPackage::new()
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

    // --- Phase 12 tests ---

    #[test]
    fn variable_matched() {
        let inv = make_inventory(&[], &["errno"]);
        let pkg = make_package_with_vars(&[], &["errno"]);
        let report = validate(&pkg, &inv);
        assert!(report.all_matched());
        assert_eq!(report.matched().len(), 1);
        assert_eq!(report.matches[0].item_kind, ItemKind::Variable);
    }

    #[test]
    fn variable_missing() {
        let inv = make_inventory(&[], &[]);
        let pkg = make_package_with_vars(&[], &["errno"]);
        let report = validate(&pkg, &inv);
        assert!(!report.all_matched());
        assert_eq!(report.missing().len(), 1);
        assert_eq!(report.missing()[0].item_kind, ItemKind::Variable);
    }

    #[test]
    fn variable_name_is_function() {
        let inv = make_inventory(&["errno"], &[]);
        let pkg = make_package_with_vars(&[], &["errno"]);
        let report = validate(&pkg, &inv);
        assert!(!report.all_matched());
        assert_eq!(report.matches[0].status, MatchStatus::NotAVariable);
    }

    #[test]
    fn mixed_functions_and_variables() {
        let inv = make_inventory(&["foo"], &["bar"]);
        let pkg = make_package_with_vars(&["foo"], &["bar"]);
        let report = validate(&pkg, &inv);
        assert!(report.all_matched());
        assert_eq!(report.matched().len(), 2);
    }

    #[test]
    fn hidden_function_not_matched() {
        let inv = make_inventory_with_vis(&[("foo", true, SymbolVisibility::Hidden)]);
        let pkg = make_package(&["foo"]);
        let report = validate(&pkg, &inv);
        assert!(!report.all_matched());
        assert_eq!(report.hidden().len(), 1);
        assert_eq!(
            report.matches[0].visibility,
            Some(SymbolVisibility::Hidden)
        );
    }

    #[test]
    fn internal_variable_not_matched() {
        let inv = make_inventory_with_vis(&[("data", false, SymbolVisibility::Internal)]);
        let pkg = make_package_with_vars(&[], &["data"]);
        let report = validate(&pkg, &inv);
        assert!(!report.all_matched());
        assert_eq!(report.hidden().len(), 1);
    }

    #[test]
    fn weak_function_match() {
        let inv = SymbolInventory {
            artifact_path: "test.o".into(),
            format: ArtifactFormat::ElfObject,
            symbols: vec![SymbolEntry {
                name: "foo".into(),
                raw_name: None,
                visibility: SymbolVisibility::Default,
                is_function: true,
                binding: SymbolBinding::Weak,
                size: None,
                section: None,
                archive_member: None,
            }],
        };
        let pkg = make_package(&["foo"]);
        let report = validate(&pkg, &inv);
        assert_eq!(report.matches[0].status, MatchStatus::WeakMatch);
        // WeakMatch is not the same as Matched
        assert!(!report.all_matched());
    }

    #[test]
    fn default_visibility_matched() {
        let inv = make_inventory_with_vis(&[("foo", true, SymbolVisibility::Default)]);
        let pkg = make_package(&["foo"]);
        let report = validate(&pkg, &inv);
        assert!(report.all_matched());
        assert_eq!(
            report.matches[0].visibility,
            Some(SymbolVisibility::Default)
        );
    }

    #[test]
    fn match_has_item_kind() {
        let inv = make_inventory(&["foo"], &["bar"]);
        let pkg = make_package_with_vars(&["foo"], &["bar"]);
        let report = validate(&pkg, &inv);
        let func_match = report.matches.iter().find(|m| m.name == "foo").unwrap();
        let var_match = report.matches.iter().find(|m| m.name == "bar").unwrap();
        assert_eq!(func_match.item_kind, ItemKind::Function);
        assert_eq!(var_match.item_kind, ItemKind::Variable);
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
