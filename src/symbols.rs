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
    MachOObject,
    MachODylib,
    MachOStaticLibrary,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactPlatform {
    Elf,
    MachO,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactKind {
    Object,
    StaticLibrary,
    SharedLibrary,
    Executable,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ArtifactCapabilities {
    #[serde(default)]
    pub exports_symbols: bool,
    #[serde(default)]
    pub imports_symbols: bool,
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
    #[serde(default)]
    pub raw_name: Option<String>,
    pub visibility: SymbolVisibility,
    pub is_function: bool,
    pub binding: SymbolBinding,
    pub size: Option<u64>,
    pub section: Option<String>,
    #[serde(default)]
    pub archive_member: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolInventory {
    pub artifact_path: String,
    pub format: ArtifactFormat,
    pub platform: ArtifactPlatform,
    pub kind: ArtifactKind,
    #[serde(default)]
    pub capabilities: ArtifactCapabilities,
    #[serde(default)]
    pub dependency_edges: Vec<String>,
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
    let dependency_edges = detect_dependency_edges(&artifact_path, &obj);

    Ok(SymbolInventory {
        artifact_path,
        format,
        platform: classify_platform(&obj),
        kind: classify_kind(&obj),
        capabilities: classify_capabilities(&obj),
        dependency_edges,
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
    let mut is_macho = false;
    let mut format_detected = false;

    for member in archive.members() {
        let member = member.map_err(|e| format!("failed to read archive member: {}", e))?;
        let member_name = Some(String::from_utf8_lossy(member.name()).into_owned());
        let member_data = member
            .data(data)
            .map_err(|e| format!("failed to read archive member data: {}", e))?;

        if let Ok(obj) = object::File::parse(member_data) {
            if !format_detected {
                is_macho = obj.format() == object::BinaryFormat::MachO;
                format_detected = true;
            }
            for mut sym in extract_symbols_from_object(&obj) {
                sym.archive_member = member_name.clone();
                if seen.insert((sym.name.clone(), sym.archive_member.clone())) {
                    symbols.push(sym);
                }
            }
        }
    }

    let format = if is_macho {
        ArtifactFormat::MachOStaticLibrary
    } else {
        ArtifactFormat::ElfStaticLibrary
    };

    Ok(SymbolInventory {
        artifact_path,
        format,
        platform: if is_macho {
            ArtifactPlatform::MachO
        } else {
            ArtifactPlatform::Elf
        },
        kind: ArtifactKind::StaticLibrary,
        capabilities: ArtifactCapabilities {
            exports_symbols: true,
            imports_symbols: false,
        },
        dependency_edges: Vec::new(),
        symbols,
    })
}

fn detect_dependency_edges(artifact_path: &str, obj: &object::File<'_>) -> Vec<String> {
    if obj.format() != object::BinaryFormat::Elf {
        return Vec::new();
    }
    if !matches!(obj.kind(), object::ObjectKind::Dynamic | object::ObjectKind::Executable) {
        return Vec::new();
    }

    let Ok(output) = std::process::Command::new("readelf")
        .args(["-d", artifact_path])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let Ok(stdout) = String::from_utf8(output.stdout) else {
        return Vec::new();
    };
    parse_elf_needed_entries(&stdout)
}

fn parse_elf_needed_entries(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|line| {
            let needed_index = line.find("(NEEDED)")?;
            let after_needed = &line[needed_index..];
            let start = after_needed.find('[')? + 1;
            let end = after_needed[start..].find(']')? + start;
            Some(after_needed[start..end].to_string())
        })
        .collect()
}

fn classify_platform(obj: &object::File<'_>) -> ArtifactPlatform {
    match obj.format() {
        object::BinaryFormat::Elf => ArtifactPlatform::Elf,
        object::BinaryFormat::MachO => ArtifactPlatform::MachO,
        _ => ArtifactPlatform::Unknown,
    }
}

fn classify_kind(obj: &object::File<'_>) -> ArtifactKind {
    use object::ObjectKind;
    match obj.kind() {
        ObjectKind::Relocatable => ArtifactKind::Object,
        ObjectKind::Dynamic => ArtifactKind::SharedLibrary,
        ObjectKind::Executable => ArtifactKind::Executable,
        _ => ArtifactKind::Unknown,
    }
}

fn classify_capabilities(obj: &object::File<'_>) -> ArtifactCapabilities {
    use object::ObjectKind;
    match obj.kind() {
        ObjectKind::Relocatable => ArtifactCapabilities {
            exports_symbols: true,
            imports_symbols: false,
        },
        ObjectKind::Dynamic | ObjectKind::Executable => ArtifactCapabilities {
            exports_symbols: true,
            imports_symbols: true,
        },
        _ => ArtifactCapabilities::default(),
    }
}

fn classify_format(obj: &object::File<'_>) -> ArtifactFormat {
    use object::ObjectKind;
    let kind = obj.kind();
    match obj.format() {
        object::BinaryFormat::Elf => match kind {
            ObjectKind::Executable | ObjectKind::Dynamic => ArtifactFormat::ElfSharedLibrary,
            ObjectKind::Relocatable => ArtifactFormat::ElfObject,
            other => ArtifactFormat::Unknown(format!("Elf {:?}", other)),
        },
        object::BinaryFormat::MachO => match kind {
            ObjectKind::Executable | ObjectKind::Dynamic => ArtifactFormat::MachODylib,
            ObjectKind::Relocatable => ArtifactFormat::MachOObject,
            other => ArtifactFormat::Unknown(format!("MachO {:?}", other)),
        },
        other => ArtifactFormat::Unknown(format!("{:?} {:?}", other, kind)),
    }
}

fn extract_symbols_from_object(obj: &object::File<'_>) -> Vec<SymbolEntry> {
    use object::ObjectSection;

    let is_macho = obj.format() == object::BinaryFormat::MachO;
    let mut symbols = Vec::new();

    // Check both regular and dynamic symbol tables
    let iter = obj.symbols().chain(obj.dynamic_symbols());
    for sym in iter {
        // Skip unnamed symbols and undefined symbols
        let raw_name = match sym.name() {
            Ok(n) if !n.is_empty() => n,
            _ => continue,
        };

        // Mach-O prefixes C symbols with '_'; strip it for consistency
        // with C identifier names used in header declarations.
        let name = if is_macho {
            raw_name.strip_prefix('_').unwrap_or(raw_name).to_string()
        } else {
            raw_name.to_string()
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
            object::SymbolFlags::MachO { n_desc } => {
                // Mach-O visibility: use ObjectSymbol::scope() for portable detection
                let vis = match sym.scope() {
                    object::SymbolScope::Dynamic => SymbolVisibility::Default,
                    object::SymbolScope::Linkage => SymbolVisibility::Hidden, // private external
                    object::SymbolScope::Compilation => SymbolVisibility::Hidden, // local
                    _ => SymbolVisibility::Unknown,
                };
                let bind = if n_desc & 0x0080 != 0 {
                    // N_WEAK_DEF
                    SymbolBinding::Weak
                } else if sym.is_global() {
                    SymbolBinding::Global
                } else {
                    SymbolBinding::Local
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
            raw_name: Some(raw_name.to_string()),
            visibility,
            is_function,
            binding,
            size,
            section,
            archive_member: None,
        });
    }

    symbols
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid Mach-O 64-bit (x86_64) relocatable object with
    /// a single global function symbol `_foo` in a `__TEXT,__text` section.
    fn minimal_macho_object() -> Vec<u8> {
        let mut d = Vec::new();

        let header_size: usize = 32;
        let seg_cmd_size: usize = 72 + 80; // LC_SEGMENT_64 + 1 section_64
        let symtab_cmd_size: usize = 24;
        let cmds_size = seg_cmd_size + symtab_cmd_size; // 176
        let text_offset = header_size + cmds_size; // 208
        let text_size: usize = 4;
        let symtab_offset = text_offset + text_size; // 212
        let nsyms: u32 = 1;
        let nlist_size: usize = 16;
        let strtab_offset = symtab_offset + (nsyms as usize * nlist_size); // 228
        let strtab: &[u8] = b"\0_foo\0";
        let strtab_size = strtab.len(); // 6

        // --- mach_header_64 (32 bytes) ---
        d.extend_from_slice(&0xFEEDFACFu32.to_le_bytes()); // magic
        d.extend_from_slice(&0x01000007u32.to_le_bytes()); // cputype: CPU_TYPE_X86_64
        d.extend_from_slice(&0x00000003u32.to_le_bytes()); // cpusubtype: CPU_SUBTYPE_ALL
        d.extend_from_slice(&0x00000001u32.to_le_bytes()); // filetype: MH_OBJECT
        d.extend_from_slice(&2u32.to_le_bytes());           // ncmds
        d.extend_from_slice(&(cmds_size as u32).to_le_bytes());
        d.extend_from_slice(&0u32.to_le_bytes());           // flags
        d.extend_from_slice(&0u32.to_le_bytes());           // reserved

        // --- LC_SEGMENT_64 (72 bytes base) ---
        d.extend_from_slice(&0x19u32.to_le_bytes());        // cmd: LC_SEGMENT_64
        d.extend_from_slice(&(seg_cmd_size as u32).to_le_bytes());
        d.extend_from_slice(&[0u8; 16]);                    // segname (empty)
        d.extend_from_slice(&0u64.to_le_bytes());           // vmaddr
        d.extend_from_slice(&(text_size as u64).to_le_bytes()); // vmsize
        d.extend_from_slice(&(text_offset as u64).to_le_bytes()); // fileoff
        d.extend_from_slice(&(text_size as u64).to_le_bytes()); // filesize
        d.extend_from_slice(&0x07u32.to_le_bytes());        // maxprot: rwx
        d.extend_from_slice(&0x05u32.to_le_bytes());        // initprot: rx
        d.extend_from_slice(&1u32.to_le_bytes());           // nsects
        d.extend_from_slice(&0u32.to_le_bytes());           // flags

        // --- section_64 (80 bytes) ---
        let mut sectname = [0u8; 16];
        sectname[..6].copy_from_slice(b"__text");
        d.extend_from_slice(&sectname);
        let mut segname = [0u8; 16];
        segname[..6].copy_from_slice(b"__TEXT");
        d.extend_from_slice(&segname);
        d.extend_from_slice(&0u64.to_le_bytes());           // addr
        d.extend_from_slice(&(text_size as u64).to_le_bytes()); // size
        d.extend_from_slice(&(text_offset as u32).to_le_bytes()); // offset
        d.extend_from_slice(&0u32.to_le_bytes());           // align
        d.extend_from_slice(&0u32.to_le_bytes());           // reloff
        d.extend_from_slice(&0u32.to_le_bytes());           // nreloc
        d.extend_from_slice(&0x80000400u32.to_le_bytes()); // flags: S_REGULAR|PURE_INSTRUCTIONS|SOME_INSTRUCTIONS
        d.extend_from_slice(&0u32.to_le_bytes());           // reserved1
        d.extend_from_slice(&0u32.to_le_bytes());           // reserved2
        d.extend_from_slice(&0u32.to_le_bytes());           // reserved3

        // --- LC_SYMTAB (24 bytes) ---
        d.extend_from_slice(&0x02u32.to_le_bytes());        // cmd: LC_SYMTAB
        d.extend_from_slice(&(symtab_cmd_size as u32).to_le_bytes());
        d.extend_from_slice(&(symtab_offset as u32).to_le_bytes());
        d.extend_from_slice(&nsyms.to_le_bytes());
        d.extend_from_slice(&(strtab_offset as u32).to_le_bytes());
        d.extend_from_slice(&(strtab_size as u32).to_le_bytes());

        // --- __text section data ---
        assert_eq!(d.len(), text_offset);
        d.extend_from_slice(&[0xC3, 0x90, 0x90, 0x90]); // ret, nop, nop, nop

        // --- nlist_64 symbol entry (16 bytes) ---
        assert_eq!(d.len(), symtab_offset);
        d.extend_from_slice(&1u32.to_le_bytes());           // n_strx -> "_foo"
        d.push(0x0F); // n_type: N_SECT (0x0E) | N_EXT (0x01)
        d.push(0x01); // n_sect: 1 (first section, 1-based)
        d.extend_from_slice(&0u16.to_le_bytes());           // n_desc
        d.extend_from_slice(&0u64.to_le_bytes());           // n_value

        // --- string table ---
        assert_eq!(d.len(), strtab_offset);
        d.extend_from_slice(strtab);

        d
    }

    /// Build a Mach-O 64-bit object with a weak function symbol `_weak_fn`.
    fn macho_object_with_weak_symbol() -> Vec<u8> {
        let mut d = minimal_macho_object();
        // Patch nlist_64 n_desc to N_WEAK_DEF (0x0080)
        // nlist_64: n_strx(4) + n_type(1) + n_sect(1) + n_desc(2) + n_value(8)
        // n_desc is at symtab_offset + 6 = 212 + 6 = 218
        let n_desc_offset = 212 + 6;
        d[n_desc_offset] = 0x80; // low byte of n_desc = 0x0080
        d[n_desc_offset + 1] = 0x00;
        // Also patch the string table to say "_weak_fn" instead of "_foo"
        let strtab_offset = 228;
        // Replace "\0_foo\0" with "\0_weak_fn\0" — need to extend
        d.truncate(strtab_offset);
        d.extend_from_slice(b"\0_weak_fn\0");
        // Update strsize in LC_SYMTAB (at offset 0xCC = 204)
        let new_strsize = 10u32; // "\0_weak_fn\0".len()
        d[204..208].copy_from_slice(&new_strsize.to_le_bytes());
        d
    }

    #[test]
    fn macho_format_variants_serde() {
        for fmt in [
            ArtifactFormat::MachOObject,
            ArtifactFormat::MachODylib,
            ArtifactFormat::MachOStaticLibrary,
        ] {
            let json = serde_json::to_string(&fmt).unwrap();
            let fmt2: ArtifactFormat = serde_json::from_str(&json).unwrap();
            assert_eq!(fmt, fmt2);
        }
    }

    #[test]
    fn inspect_macho_object() {
        let data = minimal_macho_object();
        let inv = inspect_bytes(&data, "test.o".into()).unwrap();
        assert_eq!(inv.format, ArtifactFormat::MachOObject);
        assert_eq!(inv.platform, ArtifactPlatform::MachO);
        assert_eq!(inv.kind, ArtifactKind::Object);
        assert!(inv.capabilities.exports_symbols);
        assert!(inv.dependency_edges.is_empty());
        // Leading '_' should be stripped
        assert!(inv.has_symbol("foo"), "symbols: {:?}", inv.symbols);
        assert!(!inv.has_symbol("_foo"), "underscore should be stripped");
        assert_eq!(inv.function_names(), vec!["foo"]);
    }

    #[test]
    fn macho_symbol_visibility_and_binding() {
        let data = minimal_macho_object();
        let inv = inspect_bytes(&data, "test.o".into()).unwrap();
        let sym = inv.symbols.iter().find(|s| s.name == "foo").unwrap();
        assert_eq!(sym.raw_name.as_deref(), Some("_foo"));
        assert_eq!(sym.visibility, SymbolVisibility::Default);
        assert_eq!(sym.binding, SymbolBinding::Global);
        assert!(sym.is_function);
    }

    #[test]
    fn macho_weak_symbol_detected() {
        let data = macho_object_with_weak_symbol();
        let inv = inspect_bytes(&data, "test.o".into()).unwrap();
        let sym = inv.symbols.iter().find(|s| s.name == "weak_fn").unwrap();
        assert_eq!(sym.binding, SymbolBinding::Weak);
        assert!(sym.is_function);
    }

    #[test]
    fn macho_section_name() {
        let data = minimal_macho_object();
        let inv = inspect_bytes(&data, "test.o".into()).unwrap();
        let sym = inv.symbols.iter().find(|s| s.name == "foo").unwrap();
        assert_eq!(sym.section.as_deref(), Some("__text"));
    }

    #[test]
    fn symbol_inventory_has_symbol() {
        let inv = SymbolInventory {
            artifact_path: "test.o".into(),
            format: ArtifactFormat::ElfObject,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::Object,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![
                SymbolEntry {
                    name: "foo".into(),
                    raw_name: None,
                    visibility: SymbolVisibility::Default,
                    is_function: true,
                    binding: SymbolBinding::Global,
                    size: None,
                    section: None,
                    archive_member: None,
                },
                SymbolEntry {
                    name: "bar".into(),
                    raw_name: None,
                    visibility: SymbolVisibility::Default,
                    is_function: false,
                    binding: SymbolBinding::Global,
                    size: None,
                    section: None,
                    archive_member: None,
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
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::Object,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![
                SymbolEntry {
                    name: "func1".into(),
                    raw_name: None,
                    visibility: SymbolVisibility::Default,
                    is_function: true,
                    binding: SymbolBinding::Global,
                    size: None,
                    section: None,
                    archive_member: None,
                },
                SymbolEntry {
                    name: "data1".into(),
                    raw_name: None,
                    visibility: SymbolVisibility::Default,
                    is_function: false,
                    binding: SymbolBinding::Global,
                    size: None,
                    section: None,
                    archive_member: None,
                },
                SymbolEntry {
                    name: "func2".into(),
                    raw_name: None,
                    visibility: SymbolVisibility::Default,
                    is_function: true,
                    binding: SymbolBinding::Global,
                    size: None,
                    section: None,
                    archive_member: None,
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
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::StaticLibrary,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![SymbolEntry {
                name: "foo_init".into(),
                raw_name: Some("foo_init".into()),
                visibility: SymbolVisibility::Default,
                is_function: true,
                binding: SymbolBinding::Global,
                size: None,
                section: None,
                archive_member: Some("foo.o".into()),
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

    #[test]
    fn parse_elf_needed_entries_extracts_dependencies() {
        let parsed = parse_elf_needed_entries(
            r#"
Dynamic section at offset 0x2de0 contains 3 entries:
  Tag        Type                         Name/Value
 0x0000000000000001 (NEEDED)             Shared library: [libm.so.6]
 0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]
"#,
        );
        assert_eq!(parsed, vec!["libm.so.6".to_string(), "libc.so.6".to_string()]);
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
        assert_eq!(inv.platform, ArtifactPlatform::Elf);
        assert_eq!(inv.kind, ArtifactKind::Object);
        assert!(inv.has_symbol("foo"));
        assert!(inv.has_symbol("bar"));
        let foo = inv.symbols.iter().find(|sym| sym.name == "foo").unwrap();
        assert_eq!(foo.raw_name.as_deref(), Some("foo"));

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
        assert_eq!(inv.platform, ArtifactPlatform::Elf);
        assert_eq!(inv.kind, ArtifactKind::StaticLibrary);
        assert!(inv.has_symbol("add"));
        let add = inv.symbols.iter().find(|sym| sym.name == "add").unwrap();
        assert_eq!(add.raw_name.as_deref(), Some("add"));
        assert_eq!(add.archive_member.as_deref(), Some("lib.o"));

        std::fs::remove_file(&c_path).ok();
        std::fs::remove_file(&o_path).ok();
        std::fs::remove_file(&a_path).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[test]
    #[ignore] // Requires cc and ar
    fn inspect_static_library_preserves_member_provenance() {
        let dir = std::env::temp_dir().join("bic_ar_members_test");
        std::fs::create_dir_all(&dir).unwrap();
        let a_c_path = dir.join("alpha.c");
        let a_o_path = dir.join("alpha.o");
        let b_c_path = dir.join("beta.c");
        let b_o_path = dir.join("beta.o");
        let a_path = dir.join("libmembers.a");

        std::fs::write(&a_c_path, "int alpha(void) { return 1; }\n").unwrap();
        std::fs::write(&b_c_path, "int beta(void) { return 2; }\n").unwrap();

        let cc_alpha = std::process::Command::new("cc")
            .args(["-c", "-o"])
            .arg(&a_o_path)
            .arg(&a_c_path)
            .status()
            .expect("cc not found");
        assert!(cc_alpha.success());

        let cc_beta = std::process::Command::new("cc")
            .args(["-c", "-o"])
            .arg(&b_o_path)
            .arg(&b_c_path)
            .status()
            .expect("cc not found");
        assert!(cc_beta.success());

        let ar = std::process::Command::new("ar")
            .args(["rcs"])
            .arg(&a_path)
            .arg(&a_o_path)
            .arg(&b_o_path)
            .status()
            .expect("ar not found");
        assert!(ar.success());

        let inv = inspect_file(&a_path).unwrap();
        let alpha = inv.symbols.iter().find(|sym| sym.name == "alpha").unwrap();
        let beta = inv.symbols.iter().find(|sym| sym.name == "beta").unwrap();
        assert_eq!(alpha.raw_name.as_deref(), Some("alpha"));
        assert_eq!(beta.raw_name.as_deref(), Some("beta"));
        assert_eq!(alpha.archive_member.as_deref(), Some("alpha.o"));
        assert_eq!(beta.archive_member.as_deref(), Some("beta.o"));

        std::fs::remove_file(&a_c_path).ok();
        std::fs::remove_file(&a_o_path).ok();
        std::fs::remove_file(&b_c_path).ok();
        std::fs::remove_file(&b_o_path).ok();
        std::fs::remove_file(&a_path).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[test]
    #[ignore] // Requires cc and readelf
    fn inspect_shared_library_captures_dependency_edges() {
        let dir = std::env::temp_dir().join("bic_shared_dep_test");
        std::fs::create_dir_all(&dir).unwrap();
        let c_path = dir.join("lib.c");
        let so_path = dir.join("libdep.so");

        std::fs::write(&c_path, "double call_cos(double x) { extern double cos(double); return cos(x); }\n").unwrap();

        let cc = std::process::Command::new("cc")
            .args(["-shared", "-fPIC", "-o"])
            .arg(&so_path)
            .arg(&c_path)
            .arg("-lm")
            .status()
            .expect("cc not found");
        assert!(cc.success());

        let inv = inspect_file(&so_path).unwrap();
        assert_eq!(inv.kind, ArtifactKind::SharedLibrary);
        assert!(inv.dependency_edges.iter().any(|edge| edge.contains("libm")));

        std::fs::remove_file(&c_path).ok();
        std::fs::remove_file(&so_path).ok();
        std::fs::remove_dir(&dir).ok();
    }
}
