use std::path::Path;

use crate::diagnostics::{Diagnostic, DiagnosticKind};
use crate::extract::Extractor;
use crate::ir::BindingPackage;

/// Transitional preprocessing wrapper — used internally by tests.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PreprocessedInput {
    pub source: String,
    pub source_path: Option<String>,
    pub flavor: pac::driver::Flavor,
}

#[allow(dead_code)]
impl PreprocessedInput {
    pub fn from_string(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            source_path: None,
            flavor: pac::driver::Flavor::GnuC11,
        }
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        let path = path.as_ref();
        let source = std::fs::read_to_string(path)?;
        Ok(Self {
            source,
            source_path: Some(path.display().to_string()),
            flavor: pac::driver::Flavor::GnuC11,
        })
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.source_path = Some(path.into());
        self
    }

    pub fn with_flavor(mut self, flavor: pac::driver::Flavor) -> Self {
        self.flavor = flavor;
        self
    }

    pub fn extract(&self) -> BindingPackage {
        let unit = match pac::parse::translation_unit(&self.source, self.flavor) {
            Ok(unit) => unit,
            Err(e) => {
                return BindingPackage {
                    source_path: self.source_path.clone(),
                    items: Vec::new(),
                    diagnostics: vec![Diagnostic::error(
                        DiagnosticKind::ParseFailed,
                        format!(
                            "parse error at line {}:{}: {:?}",
                            e.line, e.column, e.expected
                        ),
                    )],
                    ..BindingPackage::new()
                };
            }
        };

        let extractor = Extractor::new();
        let (items, diagnostics) = extractor.extract(&unit);

        BindingPackage {
            source_path: self.source_path.clone(),
            items,
            diagnostics,
            ..BindingPackage::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::*;

    #[test]
    fn preprocessed_from_string() {
        let input = PreprocessedInput::from_string("typedef int int32_t;");
        let pkg = input.extract();
        assert_eq!(pkg.items.len(), 1);
        assert!(pkg.diagnostics.is_empty());
        assert!(pkg.source_path.is_none());
    }

    #[test]
    fn preprocessed_with_path() {
        let input =
            PreprocessedInput::from_string("void foo(void);").with_path("test.i");
        let pkg = input.extract();
        assert_eq!(pkg.source_path.as_deref(), Some("test.i"));
        assert_eq!(pkg.items.len(), 1);
    }

    #[test]
    fn preprocessed_parse_error() {
        let input = PreprocessedInput::from_string("this is not valid c @@@@");
        let pkg = input.extract();
        assert!(pkg.items.is_empty());
        assert!(!pkg.diagnostics.is_empty());
        assert_eq!(pkg.diagnostics[0].kind, DiagnosticKind::ParseFailed);
    }

    #[test]
    fn preprocessed_multiple_items() {
        let src = r#"
            typedef unsigned long size_t;
            struct point { int x; int y; };
            void *malloc(size_t n);
            void free(void *ptr);
        "#;
        let input = PreprocessedInput::from_string(src).with_path("stdlib.i");
        let pkg = input.extract();
        assert_eq!(pkg.source_path.as_deref(), Some("stdlib.i"));
        assert_eq!(pkg.items.len(), 4); // typedef + struct + malloc + free
    }

    #[test]
    fn preprocessed_from_tempfile() {
        let dir = std::env::temp_dir().join("linc_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_preproc.i");
        std::fs::write(&path, "extern int puts(const char *s);").unwrap();

        let input = PreprocessedInput::from_file(&path).unwrap();
        let pkg = input.extract();
        assert_eq!(pkg.items.len(), 1);
        match &pkg.items[0] {
            BindingItem::Function(f) => assert_eq!(f.name, "puts"),
            other => panic!("expected Function, got {:?}", other),
        }

        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[test]
    fn preprocessed_file_not_found() {
        let result = PreprocessedInput::from_file("/nonexistent/path.i");
        assert!(result.is_err());
    }

    #[test]
    fn preprocessed_deterministic_output() {
        let src = "typedef int my_t; void foo(my_t x);";
        let pkg1 = PreprocessedInput::from_string(src).extract();
        let pkg2 = PreprocessedInput::from_string(src).extract();
        let j1 = serde_json::to_string(&pkg1).unwrap();
        let j2 = serde_json::to_string(&pkg2).unwrap();
        assert_eq!(j1, j2);
    }

    #[test]
    fn preprocessed_with_flavor() {
        let input = PreprocessedInput::from_string("typedef int x;")
            .with_flavor(pac::driver::Flavor::StdC11);
        let pkg = input.extract();
        assert_eq!(pkg.items.len(), 1);
    }
}
