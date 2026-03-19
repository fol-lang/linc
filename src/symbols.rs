use std::path::Path;

use object::read::Object;
use object::read::archive::ArchiveFile;
use object::{ObjectSymbol, SymbolKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactFormat {
    ElfObject,
    ElfStaticLibrary,
    ElfSharedLibrary,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolVisibility {
    Default,
    Hidden,
    Protected,
    Internal,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolBinding {
    Local,
    Global,
    Weak,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolEntry {
    pub name: String,
    pub visibility: SymbolVisibility,
    pub is_function: bool,
    pub binding: SymbolBinding,
    pub size: Option<u64>,
    pub section: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolInventory {
    pub artifact_path: String,
    pub format: ArtifactFormat,
    pub symbols: Vec<SymbolEntry>,
}

impl SymbolInventory {
    pub fn has_symbol(&self, name: &str) -> bool {
        self.symbols.iter().any(|s| s.name == name)
    }

    pub fn function_names(&self) -> Vec<&str> {
        self.symbols
            .iter()
            .filter(|s| s.is_function)
            .map(|s| s.name.as_str())
            .collect()
    }
}

pub fn inspect_file(path: impl AsRef<Path>) -> Result<SymbolInventory, String> {
    let path = path.as_ref();
    let data = std::fs::read(path).map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
    inspect_bytes(&data, path.display().to_string())
}

pub fn inspect_bytes(data: &[u8], artifact_path: String) -> Result<SymbolInventory, String> {
    // Try as archive first (static library)
    if let Ok(archive) = ArchiveFile::parse(data) {
        return inspect_archive(archive, data, artifact_path);
    }

    // Try as single object file
    let obj = object::File::parse(data)
        .map_err(|e| format!("failed to parse {}: {}", artifact_path, e))?;

    let format = classify_format(&obj);
    let symbols = extract_symbols_from_object(&obj);

    Ok(SymbolInventory {
        artifact_path,
        format,
        symbols,
    })
}

fn inspect_archive(
    archive: ArchiveFile<'_>,
    data: &[u8],
    artifact_path: String,
) -> Result<SymbolInventory, String> {
    let mut symbols = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for member in archive.members() {
        let member = member.map_err(|e| format!("failed to read archive member: {}", e))?;
        let member_data = member
            .data(data)
            .map_err(|e| format!("failed to read archive member data: {}", e))?;

        if let Ok(obj) = object::File::parse(member_data) {
            for sym in extract_symbols_from_object(&obj) {
                if seen.insert(sym.name.clone()) {
                    symbols.push(sym);
                }
            }
        }
    }

    Ok(SymbolInventory {
        artifact_path,
        format: ArtifactFormat::ElfStaticLibrary,
        symbols,
    })
}

fn classify_format(obj: &object::File<'_>) -> ArtifactFormat {
    use object::ObjectKind;
    match obj.kind() {
        ObjectKind::Executable | ObjectKind::Dynamic => ArtifactFormat::ElfSharedLibrary,
        ObjectKind::Relocatable => ArtifactFormat::ElfObject,
        other => ArtifactFormat::Unknown(format!("{:?}", other)),
    }
}

fn extract_symbols_from_object(obj: &object::File<'_>) -> Vec<SymbolEntry> {
    use object::ObjectSection;

    let mut symbols = Vec::new();

    // Check both regular and dynamic symbol tables
    let iter = obj.symbols().chain(obj.dynamic_symbols());
    for sym in iter {
        // Skip unnamed symbols and undefined symbols
        let name = match sym.name() {
            Ok(n) if !n.is_empty() => n.to_string(),
            _ => continue,
        };

        // Keep defined symbols (have a section) or global undefined symbols
        // that might be relevant for dynamic linking. Skip local undefined symbols.
        if !sym.is_definition() {
            continue;
        }

        let is_function = sym.kind() == SymbolKind::Text;

        let (visibility, binding) = match sym.flags() {
            object::SymbolFlags::Elf { st_info, st_other } => {
                let vis = match st_other & 0x3 {
                    0 => SymbolVisibility::Default,
                    1 => SymbolVisibility::Internal,
                    2 => SymbolVisibility::Hidden,
                    3 => SymbolVisibility::Protected,
                    _ => SymbolVisibility::Unknown,
                };
                let bind = match st_info >> 4 {
                    0 => SymbolBinding::Local,
                    1 => SymbolBinding::Global,
                    2 => SymbolBinding::Weak,
                    _ => SymbolBinding::Unknown,
                };
                (vis, bind)
            }
            _ => (SymbolVisibility::Unknown, SymbolBinding::Unknown),
        };

        let size = {
            let s = sym.size();
            if s > 0 { Some(s) } else { None }
        };

        let section = sym
            .section_index()
            .and_then(|idx| obj.section_by_index(idx).ok())
            .and_then(|sec| sec.name().ok().map(|n| n.to_string()));

        symbols.push(SymbolEntry {
            name,
            visibility,
            is_function,
            binding,
            size,
            section,
        });
    }

    symbols
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_inventory_has_symbol() {
        let inv = SymbolInventory {
            artifact_path: "test.o".into(),
            format: ArtifactFormat::ElfObject,
            symbols: vec![
                SymbolEntry {
                    name: "foo".into(),
                    visibility: SymbolVisibility::Default,
                    is_function: true,
                    binding: SymbolBinding::Global,
                    size: None,
                    section: None,
                },
                SymbolEntry {
                    name: "bar".into(),
                    visibility: SymbolVisibility::Default,
                    is_function: false,
                    binding: SymbolBinding::Global,
                    size: None,
                    section: None,
                },
            ],
        };
        assert!(inv.has_symbol("foo"));
        assert!(inv.has_symbol("bar"));
        assert!(!inv.has_symbol("baz"));
    }

    #[test]
    fn symbol_inventory_function_names() {
        let inv = SymbolInventory {
            artifact_path: "test.o".into(),
            format: ArtifactFormat::ElfObject,
            symbols: vec![
                SymbolEntry {
                    name: "func1".into(),
                    visibility: SymbolVisibility::Default,
                    is_function: true,
                    binding: SymbolBinding::Global,
                    size: None,
                    section: None,
                },
                SymbolEntry {
                    name: "data1".into(),
                    visibility: SymbolVisibility::Default,
                    is_function: false,
                    binding: SymbolBinding::Global,
                    size: None,
                    section: None,
                },
                SymbolEntry {
                    name: "func2".into(),
                    visibility: SymbolVisibility::Default,
                    is_function: true,
                    binding: SymbolBinding::Global,
                    size: None,
                    section: None,
                },
            ],
        };
        let funcs = inv.function_names();
        assert_eq!(funcs, vec!["func1", "func2"]);
    }

    #[test]
    fn symbol_inventory_serialization() {
        let inv = SymbolInventory {
            artifact_path: "libfoo.a".into(),
            format: ArtifactFormat::ElfStaticLibrary,
            symbols: vec![SymbolEntry {
                name: "foo_init".into(),
                visibility: SymbolVisibility::Default,
                is_function: true,
                binding: SymbolBinding::Global,
                size: None,
                section: None,
            }],
        };
        let json = serde_json::to_string(&inv).unwrap();
        let inv2: SymbolInventory = serde_json::from_str(&json).unwrap();
        assert_eq!(inv, inv2);
    }

    #[test]
    fn inspect_nonexistent_file() {
        let result = inspect_file("/nonexistent/path.o");
        assert!(result.is_err());
    }

    /// Compile a minimal C file to .o and inspect its symbols.
    #[test]
    #[ignore] // Requires cc
    fn inspect_compiled_object() {
        let dir = std::env::temp_dir().join("bic_sym_test");
        std::fs::create_dir_all(&dir).unwrap();
        let c_path = dir.join("test.c");
        let o_path = dir.join("test.o");

        std::fs::write(&c_path, "int foo(void) { return 42; }\nint bar = 7;\n").unwrap();

        let status = std::process::Command::new("cc")
            .args(["-c", "-o"])
            .arg(&o_path)
            .arg(&c_path)
            .status()
            .expect("cc not found");
        assert!(status.success());

        let inv = inspect_file(&o_path).unwrap();
        assert!(matches!(inv.format, ArtifactFormat::ElfObject));
        assert!(inv.has_symbol("foo"));
        assert!(inv.has_symbol("bar"));

        let funcs = inv.function_names();
        assert!(funcs.contains(&"foo"));

        std::fs::remove_file(&c_path).ok();
        std::fs::remove_file(&o_path).ok();
        std::fs::remove_dir(&dir).ok();
    }

    /// Compile to .a and inspect.
    #[test]
    #[ignore] // Requires cc and ar
    fn inspect_static_library() {
        let dir = std::env::temp_dir().join("bic_ar_test");
        std::fs::create_dir_all(&dir).unwrap();
        let c_path = dir.join("lib.c");
        let o_path = dir.join("lib.o");
        let a_path = dir.join("libtest.a");

        std::fs::write(&c_path, "int add(int a, int b) { return a + b; }\n").unwrap();

        let cc = std::process::Command::new("cc")
            .args(["-c", "-o"])
            .arg(&o_path)
            .arg(&c_path)
            .status()
            .expect("cc not found");
        assert!(cc.success());

        let ar = std::process::Command::new("ar")
            .args(["rcs"])
            .arg(&a_path)
            .arg(&o_path)
            .status()
            .expect("ar not found");
        assert!(ar.success());

        let inv = inspect_file(&a_path).unwrap();
        assert_eq!(inv.format, ArtifactFormat::ElfStaticLibrary);
        assert!(inv.has_symbol("add"));

        std::fs::remove_file(&c_path).ok();
        std::fs::remove_file(&o_path).ok();
        std::fs::remove_file(&a_path).ok();
        std::fs::remove_dir(&dir).ok();
    }
}
