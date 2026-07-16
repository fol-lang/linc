use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{OsStr, OsString},
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use object::{
    read::archive::ArchiveFile, Endianness as ObjectEndian, FileKind, Object, ObjectKind,
    ObjectSection, ObjectSymbol, SymbolFlags,
};
use parc::contract::{
    Architecture, ContentFingerprint, Endian, Environment, ObjectFormat, OperatingSystem,
    TargetSpec,
};

use crate::contract::{
    ArtifactFingerprint, ArtifactKind, ArtifactSymbolId, CrtFlavor, DependencyEdge,
    DependencyProvenance, InspectionParserIdentity, InspectionParserKind, InspectionProvenance,
    InspectionToolIdentity, InspectionToolKind, LinkerFlavor, NativeAbi, ObservedTarget,
    ObservedTargetParts, ProviderId, ProviderProvenance, ProviderResolution, ResolvedArtifact,
    ResolvedArtifactInput, SymbolBinding, SymbolDecoration, SymbolDirection, SymbolInventory,
    SymbolKind, SymbolRecord, SymbolRecordInput, SymbolVisibility,
};

use super::error::{io_error, NativeError, NativeResult};

pub const OBJECT_PARSER_VERSION: &str = "object-0.37.3+linc-elf-v1";
const TOOL_VERSION: &str = "linc-native-inspection-v1/object-0.37.3";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InspectionLimits {
    pub max_artifact_bytes: u64,
    pub max_archive_members: usize,
    pub max_symbols: usize,
    pub max_dependencies: usize,
    pub max_name_bytes: usize,
}

impl Default for InspectionLimits {
    fn default() -> Self {
        Self {
            max_artifact_bytes: 512 * 1024 * 1024,
            max_archive_members: 16_384,
            max_symbols: 1_000_000,
            max_dependencies: 65_536,
            max_name_bytes: 1024 * 1024,
        }
    }
}

impl InspectionLimits {
    pub fn validate(self) -> NativeResult<Self> {
        if self.max_artifact_bytes == 0
            || self.max_archive_members == 0
            || self.max_symbols == 0
            || self.max_dependencies == 0
            || self.max_name_bytes == 0
        {
            return Err(NativeError::InvalidPolicy {
                detail: "all inspection limits must be nonzero".to_owned(),
            });
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeededLibrary {
    name: OsString,
    provenance: DependencyProvenance,
}

impl NeededLibrary {
    pub fn name(&self) -> &OsStr {
        &self.name
    }

    pub const fn provenance(&self) -> DependencyProvenance {
        self.provenance
    }
}

/// Checked parser output before dependency names have been bound to providers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactInspection {
    artifact: ResolvedArtifact,
    inspection: InspectionProvenance,
    symbols: Vec<SymbolRecord>,
    dependencies: Vec<NeededLibrary>,
    provided_names: Vec<OsString>,
}

impl ArtifactInspection {
    pub fn artifact(&self) -> &ResolvedArtifact {
        &self.artifact
    }

    pub fn symbols(&self) -> &[SymbolRecord] {
        &self.symbols
    }

    pub fn inspection(&self) -> &InspectionProvenance {
        &self.inspection
    }

    pub fn dependencies(&self) -> &[NeededLibrary] {
        &self.dependencies
    }

    /// Return whether this artifact can satisfy an exact dependency identity.
    ///
    /// The set contains the canonical filename, any explicit input alias, and
    /// the ELF `DT_SONAME` when present. Native strings are compared without
    /// lossy conversion.
    pub fn provides(&self, requested: &OsStr) -> bool {
        self.provided_names.iter().any(|name| name == requested)
    }

    pub fn provided_names(&self) -> &[OsString] {
        &self.provided_names
    }

    pub(crate) fn add_provided_name(&mut self, name: Option<&OsStr>) {
        if let Some(name) = name {
            let name = name.to_os_string();
            if !self.provided_names.contains(&name) {
                self.provided_names.push(name);
            }
        }
    }

    /// Bind each dependency, in recorded dynamic-table order, to its resolved
    /// provider. An absent provider is preserved for diagnostic snapshots but
    /// strict packages reject it.
    pub fn inventory_with_dependencies(
        &self,
        providers: &[Option<ProviderId>],
    ) -> NativeResult<SymbolInventory> {
        if providers.len() != self.dependencies.len() {
            return Err(NativeError::InvalidPolicy {
                detail: format!(
                    "dependency provider count {} differs from inspected count {}",
                    providers.len(),
                    self.dependencies.len()
                ),
            });
        }
        let edges = self
            .dependencies
            .iter()
            .zip(providers)
            .map(|(dependency, provider)| {
                DependencyEdge::try_new(dependency.name.clone(), *provider, dependency.provenance)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SymbolInventory::try_new(
            self.artifact.clone(),
            self.inspection.clone(),
            self.symbols.clone(),
            edges,
        )?)
    }
}

#[derive(Debug, Clone, Default)]
pub struct NativeInspector {
    limits: InspectionLimits,
}

impl NativeInspector {
    pub fn new(limits: InspectionLimits) -> NativeResult<Self> {
        Ok(Self {
            limits: limits.validate()?,
        })
    }

    pub fn limits(&self) -> InspectionLimits {
        self.limits
    }

    #[allow(clippy::too_many_arguments)]
    pub fn inspect(
        &self,
        path: &Path,
        expected_kind: ArtifactKind,
        resolution: ProviderResolution,
        provenance: ProviderProvenance,
        requested_target: &TargetSpec,
    ) -> NativeResult<ArtifactInspection> {
        if requested_target.operating_system() != OperatingSystem::Linux
            || requested_target.object_format() != ObjectFormat::Elf
        {
            return Err(NativeError::UnsupportedArtifact {
                path: path.to_path_buf(),
                detail: format!(
                    "the certified native-inspection tier requires Linux ELF, got {:?}/{:?}",
                    requested_target.operating_system(),
                    requested_target.object_format()
                ),
            });
        }
        let canonical_path = fs::canonicalize(path)
            .map_err(|error| io_error("canonicalize artifact", path, error))?;
        let file = fs::File::open(&canonical_path)
            .map_err(|error| io_error("open artifact", &canonical_path, error))?;
        let metadata = file
            .metadata()
            .map_err(|error| io_error("stat artifact", &canonical_path, error))?;
        if !metadata.is_file() {
            return Err(NativeError::UnsupportedArtifact {
                path: canonical_path,
                detail: "provider is not a regular file".to_owned(),
            });
        }
        if metadata.len() > self.limits.max_artifact_bytes {
            return Err(NativeError::ArtifactTooLarge {
                path: canonical_path,
                limit: self.limits.max_artifact_bytes,
            });
        }
        let mut bytes = Vec::with_capacity(
            usize::try_from(metadata.len().min(self.limits.max_artifact_bytes))
                .unwrap_or(1024 * 1024),
        );
        file.take(self.limits.max_artifact_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| io_error("read artifact", &canonical_path, error))?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > self.limits.max_artifact_bytes {
            return Err(NativeError::ArtifactTooLarge {
                path: canonical_path,
                limit: self.limits.max_artifact_bytes,
            });
        }
        if bytes.is_empty() {
            return Err(NativeError::EmptyArtifact {
                path: canonical_path,
            });
        }
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != metadata.len() {
            return Err(NativeError::CorruptArtifact {
                path: canonical_path,
                detail: "artifact size changed during inspection".to_owned(),
            });
        }

        let file_kind = FileKind::parse(&*bytes).map_err(|error| NativeError::CorruptArtifact {
            path: canonical_path.clone(),
            detail: error.to_string(),
        })?;
        let parsed = match file_kind {
            FileKind::Archive => self.parse_archive(&canonical_path, &bytes)?,
            FileKind::Elf32 | FileKind::Elf64 => {
                self.parse_elf(&canonical_path, &bytes, 0, None)?
            }
            _ => {
                return Err(NativeError::UnsupportedArtifact {
                    path: canonical_path,
                    detail: format!("only archive and ELF inputs are certified, got {file_kind:?}"),
                });
            }
        };
        let observed_kind = match file_kind {
            FileKind::Archive => ArtifactKind::StaticLibrary,
            FileKind::Elf32 | FileKind::Elf64 => match parsed.object_kind {
                ObjectKind::Relocatable => ArtifactKind::Object,
                ObjectKind::Dynamic => ArtifactKind::DynamicLibrary,
                _ => {
                    return Err(NativeError::UnsupportedArtifact {
                        path: canonical_path,
                        detail: format!(
                            "ELF object kind {:?} is not a link provider",
                            parsed.object_kind
                        ),
                    });
                }
            },
            _ => unreachable!("non-ELF/non-archive kinds returned above"),
        };
        if expected_kind != observed_kind {
            return Err(NativeError::ArtifactKindMismatch {
                path: canonical_path,
                expected: artifact_kind_label(expected_kind),
                observed: artifact_kind_label(observed_kind),
            });
        }

        let observed_target = target_from_facts(requested_target, parsed.facts)?;
        if !observed_target.matches_target(requested_target) {
            return Err(NativeError::TargetMismatch {
                path: canonical_path,
                requested: target_label(requested_target),
                observed: observed_label(&observed_target),
            });
        }
        let mut provided_names = Vec::new();
        if let Some(name) = canonical_path.file_name() {
            provided_names.push(name.to_os_string());
        }
        if let Some(soname) = parsed.soname {
            if !provided_names.contains(&soname) {
                provided_names.push(soname);
            }
        }
        let artifact = ResolvedArtifact::try_new(ResolvedArtifactInput {
            artifact_fingerprint: ArtifactFingerprint::from_content(&bytes),
            canonical_path,
            kind: expected_kind,
            resolution,
            provenance,
            observed_target,
        })?;
        let inspection = inspection_provenance(file_kind)?;
        Ok(ArtifactInspection {
            artifact,
            inspection,
            symbols: parsed.symbols,
            dependencies: parsed.dependencies,
            provided_names,
        })
    }

    fn parse_archive(&self, path: &Path, bytes: &[u8]) -> NativeResult<ParsedArtifact> {
        let archive = ArchiveFile::parse(bytes).map_err(|error| NativeError::CorruptArtifact {
            path: path.to_path_buf(),
            detail: error.to_string(),
        })?;
        let mut facts = None;
        let mut symbols = Vec::new();
        let mut members = 0_usize;
        for member in archive.members() {
            let member = member.map_err(|error| NativeError::CorruptArtifact {
                path: path.to_path_buf(),
                detail: error.to_string(),
            })?;
            members = members
                .checked_add(1)
                .ok_or_else(|| NativeError::CorruptArtifact {
                    path: path.to_path_buf(),
                    detail: "archive member count overflow".to_owned(),
                })?;
            if members > self.limits.max_archive_members {
                return Err(NativeError::CorruptArtifact {
                    path: path.to_path_buf(),
                    detail: format!(
                        "archive exceeds {} members",
                        self.limits.max_archive_members
                    ),
                });
            }
            let member_name = member.name();
            if member_name.len() > self.limits.max_name_bytes || member_name.contains(&0) {
                return Err(NativeError::CorruptArtifact {
                    path: path.to_path_buf(),
                    detail: "archive member name is invalid or oversized".to_owned(),
                });
            }
            let member_bytes =
                member
                    .data(bytes)
                    .map_err(|error| NativeError::CorruptArtifact {
                        path: path.to_path_buf(),
                        detail: error.to_string(),
                    })?;
            let table_base = u32::try_from((members - 1).saturating_mul(2)).map_err(|_| {
                NativeError::CorruptArtifact {
                    path: path.to_path_buf(),
                    detail: "archive symbol-table identity overflow".to_owned(),
                }
            })?;
            let parsed =
                self.parse_elf(path, member_bytes, table_base, Some(member_name.to_vec()))?;
            if parsed.object_kind != ObjectKind::Relocatable {
                return Err(NativeError::UnsupportedArtifact {
                    path: path.to_path_buf(),
                    detail: "static archives may contain only relocatable ELF members".to_owned(),
                });
            }
            if let Some(expected) = facts {
                if expected != parsed.facts {
                    return Err(NativeError::TargetMismatch {
                        path: path.to_path_buf(),
                        requested: facts_label(expected),
                        observed: facts_label(parsed.facts),
                    });
                }
            } else {
                facts = Some(parsed.facts);
            }
            symbols.extend(parsed.symbols);
            if symbols.len() > self.limits.max_symbols {
                return Err(NativeError::CorruptArtifact {
                    path: path.to_path_buf(),
                    detail: format!("archive exceeds {} symbols", self.limits.max_symbols),
                });
            }
        }
        let facts = facts.ok_or_else(|| NativeError::CorruptArtifact {
            path: path.to_path_buf(),
            detail: "archive has no inspectable ELF members".to_owned(),
        })?;
        Ok(ParsedArtifact {
            facts,
            object_kind: ObjectKind::Relocatable,
            symbols,
            dependencies: Vec::new(),
            soname: None,
        })
    }

    fn parse_elf(
        &self,
        path: &Path,
        bytes: &[u8],
        table_base: u32,
        archive_member: Option<Vec<u8>>,
    ) -> NativeResult<ParsedArtifact> {
        let kind = FileKind::parse(bytes).map_err(|error| NativeError::CorruptArtifact {
            path: path.to_path_buf(),
            detail: error.to_string(),
        })?;
        if !matches!(kind, FileKind::Elf32 | FileKind::Elf64) {
            return Err(NativeError::CorruptArtifact {
                path: path.to_path_buf(),
                detail: format!("archive member is not ELF: {kind:?}"),
            });
        }
        let file = object::File::parse(bytes).map_err(|error| NativeError::CorruptArtifact {
            path: path.to_path_buf(),
            detail: error.to_string(),
        })?;
        if file.format() != object::BinaryFormat::Elf {
            return Err(NativeError::UnsupportedArtifact {
                path: path.to_path_buf(),
                detail: "only ELF object members are certified".to_owned(),
            });
        }
        let facts = TargetFacts {
            architecture: map_architecture(file.architecture()).ok_or_else(|| {
                NativeError::UnsupportedArtifact {
                    path: path.to_path_buf(),
                    detail: format!("unsupported ELF architecture {:?}", file.architecture()),
                }
            })?,
            endian: match file.endianness() {
                ObjectEndian::Little => Endian::Little,
                ObjectEndian::Big => Endian::Big,
            },
            pointer_width: if file.is_64() { 64 } else { 32 },
        };
        let versions = parse_symbol_versions(path, &file, self.limits)?;
        let mut symbols = Vec::new();
        let mut dynamic_physical = BTreeSet::new();
        let dynamic_table =
            table_base
                .checked_add(1)
                .ok_or_else(|| NativeError::CorruptArtifact {
                    path: path.to_path_buf(),
                    detail: "symbol table identity overflow".to_owned(),
                })?;
        for symbol in file.dynamic_symbols() {
            let symbol_index = symbol.index().0;
            if let Some(record) = symbol_record(
                path,
                &file,
                symbol,
                dynamic_table,
                archive_member.as_deref(),
                versions.get(&symbol_index),
                self.limits,
            )? {
                dynamic_physical.insert(physical_symbol_key(&record));
                symbols.push(record);
            }
            if symbols.len() > self.limits.max_symbols {
                return Err(symbol_limit_error(path, self.limits.max_symbols));
            }
        }
        for symbol in file.symbols() {
            if let Some(record) = symbol_record(
                path,
                &file,
                symbol,
                table_base,
                archive_member.as_deref(),
                None,
                self.limits,
            )? {
                if !dynamic_physical.contains(&physical_symbol_key(&record)) {
                    symbols.push(record);
                }
            }
            if symbols.len() > self.limits.max_symbols {
                return Err(symbol_limit_error(path, self.limits.max_symbols));
            }
        }
        let dynamic = parse_dynamic_metadata(path, &file, self.limits)?;
        Ok(ParsedArtifact {
            facts,
            object_kind: file.kind(),
            symbols,
            dependencies: dynamic.dependencies,
            soname: dynamic.soname,
        })
    }
}

#[derive(Debug)]
struct ParsedArtifact {
    facts: TargetFacts,
    object_kind: ObjectKind,
    symbols: Vec<SymbolRecord>,
    dependencies: Vec<NeededLibrary>,
    soname: Option<OsString>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TargetFacts {
    architecture: Architecture,
    endian: Endian,
    pointer_width: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VersionInfo {
    name: Vec<u8>,
    is_default: bool,
}

fn inspection_provenance(kind: FileKind) -> NativeResult<InspectionProvenance> {
    let implementation =
        ContentFingerprint::from_content(b"follang.linc.native-inspection.object-0.37.3.elf-v1");
    let tool = InspectionToolIdentity::try_new(
        InspectionToolKind::ObjectCrate,
        TOOL_VERSION.to_owned(),
        implementation,
    )?;
    let mut parsers = vec![InspectionParserIdentity::try_new(
        InspectionParserKind::Elf,
        OBJECT_PARSER_VERSION.to_owned(),
        implementation,
    )?];
    if kind == FileKind::Archive {
        parsers.push(InspectionParserIdentity::try_new(
            InspectionParserKind::Archive,
            OBJECT_PARSER_VERSION.to_owned(),
            implementation,
        )?);
    }
    Ok(InspectionProvenance::try_new(tool, parsers)?)
}

fn symbol_record<'data, S>(
    path: &Path,
    file: &object::File<'data>,
    symbol: S,
    table_index: u32,
    archive_member: Option<&[u8]>,
    version: Option<&VersionInfo>,
    limits: InspectionLimits,
) -> NativeResult<Option<SymbolRecord>>
where
    S: ObjectSymbol<'data>,
{
    let raw_name = symbol
        .name_bytes()
        .map_err(|error| NativeError::InvalidSymbol {
            path: path.to_path_buf(),
            detail: error.to_string(),
        })?;
    if raw_name.is_empty() {
        return Ok(None);
    }
    if raw_name.len() > limits.max_name_bytes || raw_name.contains(&0) {
        return Err(NativeError::InvalidSymbol {
            path: path.to_path_buf(),
            detail: "symbol name is invalid or oversized".to_owned(),
        });
    }
    let name = std::str::from_utf8(raw_name)
        .map_err(|_| NativeError::InvalidSymbol {
            path: path.to_path_buf(),
            detail: "ELF symbol name is not UTF-8; raw bytes were not discarded".to_owned(),
        })?
        .to_owned();
    let (binding, visibility) = symbol_binding_visibility(symbol.flags(), symbol.is_weak());
    let kind = match symbol.kind() {
        object::SymbolKind::Text => SymbolKind::Function,
        object::SymbolKind::Data => SymbolKind::Data,
        object::SymbolKind::Tls => SymbolKind::ThreadLocal,
        object::SymbolKind::Unknown
        | object::SymbolKind::Label
        | object::SymbolKind::Section
        | object::SymbolKind::File => SymbolKind::Unknown,
        _ => SymbolKind::Unknown,
    };
    let direction = if symbol.is_undefined() {
        SymbolDirection::Imported
    } else {
        SymbolDirection::Exported
    };
    let section = symbol
        .section_index()
        .and_then(|index| file.section_by_index(index).ok())
        .and_then(|section| section.name_bytes().ok())
        .map(<[u8]>::to_vec);
    let symbol_index = u64::try_from(symbol.index().0).map_err(|_| NativeError::InvalidSymbol {
        path: path.to_path_buf(),
        detail: "symbol index does not fit the contract".to_owned(),
    })?;
    let decoration = version.map_or(SymbolDecoration::None, |version| {
        SymbolDecoration::Versioned {
            version: version.name.clone(),
            is_default: version.is_default,
        }
    });
    Ok(Some(SymbolRecord::try_new(SymbolRecordInput {
        id: ArtifactSymbolId::new(table_index, symbol_index),
        name,
        raw_name: raw_name.to_vec(),
        version: version.map(|version| version.name.clone()),
        direction,
        kind,
        binding,
        visibility,
        decoration,
        size: symbol.size(),
        address: (!symbol.is_undefined()).then(|| symbol.address()),
        section,
        archive_member: archive_member.map(<[u8]>::to_vec),
    })?))
}

fn symbol_binding_visibility(
    flags: SymbolFlags<object::SectionIndex, object::SymbolIndex>,
    weak: bool,
) -> (SymbolBinding, SymbolVisibility) {
    if let SymbolFlags::Elf { st_info, st_other } = flags {
        let binding = match st_info >> 4 {
            object::elf::STB_LOCAL => SymbolBinding::Local,
            object::elf::STB_WEAK => SymbolBinding::Weak,
            _ => SymbolBinding::Global,
        };
        let visibility = match st_other & 0x3 {
            object::elf::STV_INTERNAL => SymbolVisibility::Internal,
            object::elf::STV_HIDDEN => SymbolVisibility::Hidden,
            object::elf::STV_PROTECTED => SymbolVisibility::Protected,
            _ => SymbolVisibility::Default,
        };
        (binding, visibility)
    } else if weak {
        (SymbolBinding::Weak, SymbolVisibility::Default)
    } else {
        (SymbolBinding::Global, SymbolVisibility::Default)
    }
}

type PhysicalSymbolKey = (
    Vec<u8>,
    SymbolDirection,
    SymbolKind,
    SymbolBinding,
    SymbolVisibility,
    u64,
    Option<u64>,
    Option<Vec<u8>>,
);

fn physical_symbol_key(symbol: &SymbolRecord) -> PhysicalSymbolKey {
    (
        symbol.raw_name().to_vec(),
        symbol.direction(),
        symbol.kind(),
        symbol.binding(),
        symbol.visibility(),
        symbol.size(),
        symbol.address(),
        symbol.archive_member().map(<[u8]>::to_vec),
    )
}

#[derive(Debug, Default)]
struct DynamicMetadata {
    dependencies: Vec<NeededLibrary>,
    soname: Option<OsString>,
}

fn parse_dynamic_metadata(
    path: &Path,
    file: &object::File<'_>,
    limits: InspectionLimits,
) -> NativeResult<DynamicMetadata> {
    let Some(dynamic) = file.section_by_name(".dynamic") else {
        return Ok(DynamicMetadata::default());
    };
    let Some(strings) = file.section_by_name(".dynstr") else {
        return Err(NativeError::CorruptArtifact {
            path: path.to_path_buf(),
            detail: "ELF dynamic table has no .dynstr".to_owned(),
        });
    };
    let dynamic = dynamic
        .data()
        .map_err(|error| NativeError::CorruptArtifact {
            path: path.to_path_buf(),
            detail: error.to_string(),
        })?;
    let strings = strings
        .data()
        .map_err(|error| NativeError::CorruptArtifact {
            path: path.to_path_buf(),
            detail: error.to_string(),
        })?;
    let entry_size = if file.is_64() { 16 } else { 8 };
    if dynamic.len() % entry_size != 0 {
        return Err(NativeError::CorruptArtifact {
            path: path.to_path_buf(),
            detail: "misaligned ELF dynamic table".to_owned(),
        });
    }
    let mut dependencies = Vec::new();
    let mut soname = None;
    for entry in dynamic.chunks_exact(entry_size) {
        let (tag, value) = if file.is_64() {
            (
                read_u64(entry, 0, file.endianness())?,
                read_u64(entry, 8, file.endianness())?,
            )
        } else {
            (
                u64::from(read_u32(entry, 0, file.endianness())?),
                u64::from(read_u32(entry, 4, file.endianness())?),
            )
        };
        if tag == u64::from(object::elf::DT_NULL) {
            break;
        }
        if tag != u64::from(object::elf::DT_NEEDED) && tag != u64::from(object::elf::DT_SONAME) {
            continue;
        }
        let offset = usize::try_from(value).map_err(|_| NativeError::CorruptArtifact {
            path: path.to_path_buf(),
            detail: "DT_NEEDED string offset overflow".to_owned(),
        })?;
        let name = c_string(strings, offset, limits.max_name_bytes).ok_or_else(|| {
            NativeError::CorruptArtifact {
                path: path.to_path_buf(),
                detail: "invalid DT_NEEDED string".to_owned(),
            }
        })?;
        let name = std::str::from_utf8(name).map_err(|_| NativeError::CorruptArtifact {
            path: path.to_path_buf(),
            detail: "DT_NEEDED name is not UTF-8".to_owned(),
        })?;
        if tag == u64::from(object::elf::DT_NEEDED) {
            dependencies.push(NeededLibrary {
                name: OsString::from(name),
                provenance: DependencyProvenance::DynamicTable,
            });
        } else if soname.replace(OsString::from(name)).is_some() {
            return Err(NativeError::CorruptArtifact {
                path: path.to_path_buf(),
                detail: "ELF contains multiple DT_SONAME entries".to_owned(),
            });
        }
        if dependencies.len() > limits.max_dependencies {
            return Err(NativeError::CorruptArtifact {
                path: path.to_path_buf(),
                detail: format!(
                    "ELF exceeds {} dynamic dependencies",
                    limits.max_dependencies
                ),
            });
        }
    }
    Ok(DynamicMetadata {
        dependencies,
        soname,
    })
}

fn parse_symbol_versions(
    path: &Path,
    file: &object::File<'_>,
    limits: InspectionLimits,
) -> NativeResult<BTreeMap<usize, VersionInfo>> {
    let Some(versym) = file.section_by_name(".gnu.version") else {
        return Ok(BTreeMap::new());
    };
    let Some(dynstr) = file.section_by_name(".dynstr") else {
        return Err(NativeError::CorruptArtifact {
            path: path.to_path_buf(),
            detail: "ELF version table has no .dynstr".to_owned(),
        });
    };
    let versym = versym
        .data()
        .map_err(|error| NativeError::CorruptArtifact {
            path: path.to_path_buf(),
            detail: error.to_string(),
        })?;
    if versym.len() % 2 != 0 {
        return Err(NativeError::CorruptArtifact {
            path: path.to_path_buf(),
            detail: "misaligned .gnu.version section".to_owned(),
        });
    }
    // ELF `.gnu.version` includes the reserved null symbol at index zero;
    // `object::File::dynamic_symbols()` intentionally starts at index one.
    if versym.len() / 2 != file.dynamic_symbols().count().saturating_add(1) {
        return Err(NativeError::CorruptArtifact {
            path: path.to_path_buf(),
            detail: "ELF version table count differs from the dynamic symbol table".to_owned(),
        });
    }
    let dynstr = dynstr
        .data()
        .map_err(|error| NativeError::CorruptArtifact {
            path: path.to_path_buf(),
            detail: error.to_string(),
        })?;
    let mut names = BTreeMap::<u16, Vec<u8>>::new();
    if let Some(section) = file.section_by_name(".gnu.version_d") {
        parse_version_definitions(
            path,
            section
                .data()
                .map_err(|error| NativeError::CorruptArtifact {
                    path: path.to_path_buf(),
                    detail: error.to_string(),
                })?,
            dynstr,
            file.endianness(),
            limits,
            &mut names,
        )?;
    }
    if let Some(section) = file.section_by_name(".gnu.version_r") {
        parse_version_requirements(
            path,
            section
                .data()
                .map_err(|error| NativeError::CorruptArtifact {
                    path: path.to_path_buf(),
                    detail: error.to_string(),
                })?,
            dynstr,
            file.endianness(),
            limits,
            &mut names,
        )?;
    }
    let mut versions = BTreeMap::new();
    for (index, chunk) in versym.chunks_exact(2).enumerate() {
        let raw = read_u16(chunk, 0, file.endianness())?;
        let version_index = raw & 0x7fff;
        if version_index <= 1 {
            continue;
        }
        let name = names
            .get(&version_index)
            .ok_or_else(|| NativeError::CorruptArtifact {
                path: path.to_path_buf(),
                detail: format!("version index {version_index} has no definition"),
            })?
            .clone();
        versions.insert(
            index,
            VersionInfo {
                name,
                is_default: raw & 0x8000 == 0,
            },
        );
    }
    Ok(versions)
}

#[allow(clippy::too_many_arguments)]
fn parse_version_definitions(
    path: &Path,
    data: &[u8],
    strings: &[u8],
    endian: ObjectEndian,
    limits: InspectionLimits,
    names: &mut BTreeMap<u16, Vec<u8>>,
) -> NativeResult<()> {
    let mut offset = 0_usize;
    let mut count = 0_usize;
    while offset < data.len() {
        let header = slice_at(data, offset, 20).ok_or_else(|| corrupt_version(path))?;
        let index = read_u16(header, 4, endian)? & 0x7fff;
        let aux_offset =
            usize::try_from(read_u32(header, 12, endian)?).map_err(|_| corrupt_version(path))?;
        let next =
            usize::try_from(read_u32(header, 16, endian)?).map_err(|_| corrupt_version(path))?;
        let aux_at = offset
            .checked_add(aux_offset)
            .ok_or_else(|| corrupt_version(path))?;
        let aux = slice_at(data, aux_at, 8).ok_or_else(|| corrupt_version(path))?;
        let name_offset =
            usize::try_from(read_u32(aux, 0, endian)?).map_err(|_| corrupt_version(path))?;
        let name = c_string(strings, name_offset, limits.max_name_bytes)
            .ok_or_else(|| corrupt_version(path))?;
        if index > 1 {
            insert_version_name(path, names, index, name)?;
        }
        count += 1;
        if count > limits.max_dependencies {
            return Err(corrupt_version(path));
        }
        if next == 0 {
            break;
        }
        offset = offset
            .checked_add(next)
            .ok_or_else(|| corrupt_version(path))?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn parse_version_requirements(
    path: &Path,
    data: &[u8],
    strings: &[u8],
    endian: ObjectEndian,
    limits: InspectionLimits,
    names: &mut BTreeMap<u16, Vec<u8>>,
) -> NativeResult<()> {
    let mut offset = 0_usize;
    let mut total = 0_usize;
    while offset < data.len() {
        let header = slice_at(data, offset, 16).ok_or_else(|| corrupt_version(path))?;
        let aux_count = usize::from(read_u16(header, 2, endian)?);
        let aux_offset =
            usize::try_from(read_u32(header, 8, endian)?).map_err(|_| corrupt_version(path))?;
        let next =
            usize::try_from(read_u32(header, 12, endian)?).map_err(|_| corrupt_version(path))?;
        let mut aux_at = offset
            .checked_add(aux_offset)
            .ok_or_else(|| corrupt_version(path))?;
        for _ in 0..aux_count {
            let aux = slice_at(data, aux_at, 16).ok_or_else(|| corrupt_version(path))?;
            let index = read_u16(aux, 6, endian)? & 0x7fff;
            let name_offset =
                usize::try_from(read_u32(aux, 8, endian)?).map_err(|_| corrupt_version(path))?;
            let aux_next =
                usize::try_from(read_u32(aux, 12, endian)?).map_err(|_| corrupt_version(path))?;
            let name = c_string(strings, name_offset, limits.max_name_bytes)
                .ok_or_else(|| corrupt_version(path))?;
            if index > 1 {
                insert_version_name(path, names, index, name)?;
            }
            total += 1;
            if total > limits.max_dependencies {
                return Err(corrupt_version(path));
            }
            if aux_next == 0 {
                break;
            }
            aux_at = aux_at
                .checked_add(aux_next)
                .ok_or_else(|| corrupt_version(path))?;
        }
        if next == 0 {
            break;
        }
        offset = offset
            .checked_add(next)
            .ok_or_else(|| corrupt_version(path))?;
    }
    Ok(())
}

fn insert_version_name(
    path: &Path,
    names: &mut BTreeMap<u16, Vec<u8>>,
    index: u16,
    name: &[u8],
) -> NativeResult<()> {
    if let Some(previous) = names.insert(index, name.to_vec()) {
        if previous != name {
            return Err(NativeError::CorruptArtifact {
                path: path.to_path_buf(),
                detail: format!("ELF version index {index} has conflicting names"),
            });
        }
    }
    Ok(())
}

fn target_from_facts(target: &TargetSpec, facts: TargetFacts) -> NativeResult<ObservedTarget> {
    let abi = native_abi(target, facts.architecture, facts.pointer_width)?;
    let linker = match target.object_format() {
        ObjectFormat::Elf => LinkerFlavor::Gnu,
        ObjectFormat::MachO => LinkerFlavor::Darwin,
        ObjectFormat::Coff if target.environment() == Environment::Msvc => LinkerFlavor::Msvc,
        ObjectFormat::Coff => LinkerFlavor::Gnu,
        ObjectFormat::Wasm => LinkerFlavor::WasmLd,
        ObjectFormat::Xcoff => LinkerFlavor::Lld,
    };
    let crt = match target.environment() {
        Environment::Gnu | Environment::GnuAbi64 => CrtFlavor::Glibc,
        Environment::Musl => CrtFlavor::Musl,
        Environment::Msvc => CrtFlavor::Msvc,
        _ if matches!(
            target.operating_system(),
            OperatingSystem::Darwin
                | OperatingSystem::MacOs
                | OperatingSystem::Ios
                | OperatingSystem::TvOs
                | OperatingSystem::WatchOs
        ) =>
        {
            CrtFlavor::Darwin
        }
        _ => CrtFlavor::None,
    };
    Ok(ObservedTarget::try_new(ObservedTargetParts {
        target_fingerprint: target.fingerprint(),
        architecture: facts.architecture,
        operating_system: target.operating_system(),
        environment: target.environment(),
        object_format: ObjectFormat::Elf,
        endian: facts.endian,
        pointer_width: facts.pointer_width,
        abi,
        linker,
        crt,
    })?)
}

fn native_abi(
    target: &TargetSpec,
    architecture: Architecture,
    pointer_width: u16,
) -> NativeResult<NativeAbi> {
    let windows = target.operating_system() == OperatingSystem::Windows;
    let abi = match (architecture, pointer_width, windows) {
        (Architecture::X86, 32, false) => NativeAbi::SysV32,
        (Architecture::X86, 32, true) => NativeAbi::Win32,
        (Architecture::X86_64, 64, false) => NativeAbi::SysV64,
        (Architecture::X86_64, 64, true) => NativeAbi::Win64,
        (Architecture::Arm, 32, _) => NativeAbi::Aapcs,
        (Architecture::Aarch64, 64, _) => NativeAbi::Aapcs64,
        (Architecture::RiscV32, 32, _) => NativeAbi::RiscV32,
        (Architecture::RiscV64, 64, _) => NativeAbi::RiscV64,
        (Architecture::PowerPc, 32, _) => NativeAbi::PowerPc32SysV,
        (Architecture::PowerPc64, 64, _) => NativeAbi::PowerPc64ElfV2,
        (Architecture::S390x, 64, _) => NativeAbi::S390x,
        (Architecture::Mips, 32, _) => NativeAbi::MipsO32,
        (Architecture::Mips64, 64, _) => NativeAbi::MipsN64,
        (Architecture::Wasm32, 32, _) => NativeAbi::Wasm32,
        (Architecture::Wasm64, 64, _) => NativeAbi::Wasm64,
        _ => {
            return Err(NativeError::UnsupportedArtifact {
                path: PathBuf::from("<target>"),
                detail: format!("no certified native ABI for {architecture:?}/{pointer_width}"),
            });
        }
    };
    Ok(abi)
}

fn map_architecture(architecture: object::Architecture) -> Option<Architecture> {
    match architecture {
        object::Architecture::I386 => Some(Architecture::X86),
        object::Architecture::X86_64 => Some(Architecture::X86_64),
        object::Architecture::Arm => Some(Architecture::Arm),
        object::Architecture::Aarch64 => Some(Architecture::Aarch64),
        object::Architecture::Riscv32 => Some(Architecture::RiscV32),
        object::Architecture::Riscv64 => Some(Architecture::RiscV64),
        object::Architecture::PowerPc => Some(Architecture::PowerPc),
        object::Architecture::PowerPc64 => Some(Architecture::PowerPc64),
        object::Architecture::S390x => Some(Architecture::S390x),
        object::Architecture::Mips => Some(Architecture::Mips),
        object::Architecture::Mips64 => Some(Architecture::Mips64),
        object::Architecture::Sparc64 => Some(Architecture::Sparc64),
        object::Architecture::Wasm32 => Some(Architecture::Wasm32),
        object::Architecture::Wasm64 => Some(Architecture::Wasm64),
        _ => None,
    }
}

fn artifact_kind_label(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Object => "object",
        ArtifactKind::StaticLibrary => "static_library",
        ArtifactKind::DynamicLibrary => "dynamic_library",
        ArtifactKind::ImportLibrary => "import_library",
        ArtifactKind::Framework => "framework",
    }
}

fn target_label(target: &TargetSpec) -> String {
    format!(
        "{}/{:?}/{:?}/{}-bit/{:?}",
        target.triple(),
        target.object_format(),
        target.architecture(),
        target.pointer_width(),
        target.endian()
    )
}

fn observed_label(target: &ObservedTarget) -> String {
    format!(
        "{:?}/{:?}/{}-bit/{:?}",
        target.object_format(),
        target.architecture(),
        target.pointer_width(),
        target.endian()
    )
}

fn facts_label(facts: TargetFacts) -> String {
    format!(
        "ELF/{:?}/{}-bit/{:?}",
        facts.architecture, facts.pointer_width, facts.endian
    )
}

fn symbol_limit_error(path: &Path, limit: usize) -> NativeError {
    NativeError::CorruptArtifact {
        path: path.to_path_buf(),
        detail: format!("artifact exceeds {limit} symbols"),
    }
}

fn corrupt_version(path: &Path) -> NativeError {
    NativeError::CorruptArtifact {
        path: path.to_path_buf(),
        detail: "malformed ELF symbol-version metadata".to_owned(),
    }
}

fn c_string(data: &[u8], offset: usize, max: usize) -> Option<&[u8]> {
    let rest = data.get(offset..)?;
    let end = rest
        .iter()
        .take(max.saturating_add(1))
        .position(|byte| *byte == 0)?;
    (end <= max && end > 0).then_some(&rest[..end])
}

fn slice_at(data: &[u8], offset: usize, len: usize) -> Option<&[u8]> {
    data.get(offset..offset.checked_add(len)?)
}

fn read_u16(data: &[u8], offset: usize, endian: ObjectEndian) -> NativeResult<u16> {
    let bytes: [u8; 2] = slice_at(data, offset, 2)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| NativeError::CorruptArtifact {
            path: PathBuf::from("<memory>"),
            detail: "bounded ELF u16 read failed".to_owned(),
        })?;
    Ok(match endian {
        ObjectEndian::Little => u16::from_le_bytes(bytes),
        ObjectEndian::Big => u16::from_be_bytes(bytes),
    })
}

fn read_u32(data: &[u8], offset: usize, endian: ObjectEndian) -> NativeResult<u32> {
    let bytes: [u8; 4] = slice_at(data, offset, 4)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| NativeError::CorruptArtifact {
            path: PathBuf::from("<memory>"),
            detail: "bounded ELF u32 read failed".to_owned(),
        })?;
    Ok(match endian {
        ObjectEndian::Little => u32::from_le_bytes(bytes),
        ObjectEndian::Big => u32::from_be_bytes(bytes),
    })
}

fn read_u64(data: &[u8], offset: usize, endian: ObjectEndian) -> NativeResult<u64> {
    let bytes: [u8; 8] = slice_at(data, offset, 8)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| NativeError::CorruptArtifact {
            path: PathBuf::from("<memory>"),
            detail: "bounded ELF u64 read failed".to_owned(),
        })?;
    Ok(match endian {
        ObjectEndian::Little => u64::from_le_bytes(bytes),
        ObjectEndian::Big => u64::from_be_bytes(bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_string_parser_rejects_missing_nul_empty_and_oversized_values() {
        assert_eq!(c_string(b"abc\0tail", 0, 3), Some(&b"abc"[..]));
        assert_eq!(c_string(b"abc", 0, 3), None);
        assert_eq!(c_string(b"\0", 0, 3), None);
        assert_eq!(c_string(b"abcd\0", 0, 3), None);
        assert_eq!(c_string(b"abc\0", usize::MAX, 3), None);
    }

    #[test]
    fn bounded_integer_parser_rejects_every_truncation_boundary() {
        for length in 0..8 {
            let bytes = vec![0_u8; length];
            assert!(read_u64(&bytes, 0, ObjectEndian::Little).is_err());
        }
        assert_eq!(
            read_u64(&[1, 0, 0, 0, 0, 0, 0, 0], 0, ObjectEndian::Little).unwrap(),
            1
        );
    }
}
