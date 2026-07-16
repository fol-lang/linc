use std::{
    collections::BTreeSet,
    ffi::{OsStr, OsString},
    fmt,
    path::{Component, Path, PathBuf},
    str::FromStr,
};

use parc::contract::{
    Architecture, ContentFingerprint, DeclarationId, Endian, Environment, ObjectFormat,
    OperatingSystem, SourceRange, TargetFingerprint, TargetSpec,
};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};

use super::{ArtifactFingerprint, ContractError, ProbeEvidenceId, ProviderId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeAbi {
    SysV32,
    SysV64,
    Aapcs,
    Aapcs64,
    Win32,
    Win64,
    RiscV32,
    RiscV64,
    PowerPc32SysV,
    PowerPc64ElfV1,
    PowerPc64ElfV2,
    S390x,
    MipsO32,
    MipsN32,
    MipsN64,
    Wasm32,
    Wasm64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkerFlavor {
    Gnu,
    Lld,
    Darwin,
    Msvc,
    WasmLd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrtFlavor {
    Glibc,
    Musl,
    Msvc,
    Darwin,
    None,
}

/// Target facts observed from native artifacts and the resolving toolchain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedTarget {
    target_fingerprint: TargetFingerprint,
    architecture: Architecture,
    operating_system: OperatingSystem,
    environment: Environment,
    object_format: ObjectFormat,
    endian: Endian,
    pointer_width: u16,
    abi: NativeAbi,
    linker: LinkerFlavor,
    crt: CrtFlavor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedTargetParts {
    pub target_fingerprint: TargetFingerprint,
    pub architecture: Architecture,
    pub operating_system: OperatingSystem,
    pub environment: Environment,
    pub object_format: ObjectFormat,
    pub endian: Endian,
    pub pointer_width: u16,
    pub abi: NativeAbi,
    pub linker: LinkerFlavor,
    pub crt: CrtFlavor,
}

impl ObservedTarget {
    pub fn try_new(parts: ObservedTargetParts) -> Result<Self, ContractError> {
        if !matches!(parts.pointer_width, 16 | 32 | 64 | 128) {
            return Err(ContractError::InvalidPointerWidth {
                bits: parts.pointer_width,
            });
        }
        Ok(Self {
            target_fingerprint: parts.target_fingerprint,
            architecture: parts.architecture,
            operating_system: parts.operating_system,
            environment: parts.environment,
            object_format: parts.object_format,
            endian: parts.endian,
            pointer_width: parts.pointer_width,
            abi: parts.abi,
            linker: parts.linker,
            crt: parts.crt,
        })
    }

    pub fn for_target(
        target: &TargetSpec,
        abi: NativeAbi,
        linker: LinkerFlavor,
        crt: CrtFlavor,
    ) -> Self {
        Self {
            target_fingerprint: target.fingerprint(),
            architecture: target.architecture(),
            operating_system: target.operating_system(),
            environment: target.environment(),
            object_format: target.object_format(),
            endian: target.endian(),
            pointer_width: target.pointer_width(),
            abi,
            linker,
            crt,
        }
    }

    pub fn matches_target(&self, target: &TargetSpec) -> bool {
        self.target_fingerprint == target.fingerprint()
            && self.architecture == target.architecture()
            && self.operating_system == target.operating_system()
            && self.environment == target.environment()
            && self.object_format == target.object_format()
            && self.endian == target.endian()
            && self.pointer_width == target.pointer_width()
            && abi_matches_target(self.abi, target)
            && linker_matches_target(self.linker, target)
            && crt_matches_target(self.crt, target)
    }

    pub const fn target_fingerprint(&self) -> TargetFingerprint {
        self.target_fingerprint
    }

    pub const fn architecture(&self) -> Architecture {
        self.architecture
    }

    pub const fn operating_system(&self) -> OperatingSystem {
        self.operating_system
    }

    pub const fn environment(&self) -> Environment {
        self.environment
    }

    pub const fn object_format(&self) -> ObjectFormat {
        self.object_format
    }

    pub const fn endian(&self) -> Endian {
        self.endian
    }

    pub const fn pointer_width(&self) -> u16 {
        self.pointer_width
    }

    pub const fn abi(&self) -> NativeAbi {
        self.abi
    }

    pub const fn linker(&self) -> LinkerFlavor {
        self.linker
    }

    pub const fn crt(&self) -> CrtFlavor {
        self.crt
    }

    pub(crate) fn parts(&self) -> ObservedTargetParts {
        ObservedTargetParts {
            target_fingerprint: self.target_fingerprint,
            architecture: self.architecture,
            operating_system: self.operating_system,
            environment: self.environment,
            object_format: self.object_format,
            endian: self.endian,
            pointer_width: self.pointer_width,
            abi: self.abi,
            linker: self.linker,
            crt: self.crt,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Object,
    StaticLibrary,
    DynamicLibrary,
    ImportLibrary,
    Framework,
}

impl ArtifactKind {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Object => "object",
            Self::StaticLibrary => "static_library",
            Self::DynamicLibrary => "dynamic_library",
            Self::ImportLibrary => "import_library",
            Self::Framework => "framework",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProviderResolution {
    Explicit,
    SearchPath { native_input_index: u32 },
    Dependency { parent: ProviderId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderProvenance {
    User,
    Toolchain,
    PackageMetadata,
    Transitive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedArtifactInput {
    pub artifact_fingerprint: ArtifactFingerprint,
    pub canonical_path: PathBuf,
    pub kind: ArtifactKind,
    pub resolution: ProviderResolution,
    pub provenance: ProviderProvenance,
    pub observed_target: ObservedTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedArtifact {
    provider_id: ProviderId,
    artifact_fingerprint: ArtifactFingerprint,
    canonical_path: PathBuf,
    kind: ArtifactKind,
    resolution: ProviderResolution,
    provenance: ProviderProvenance,
    observed_target: ObservedTarget,
}

impl ResolvedArtifact {
    pub fn try_new(input: ResolvedArtifactInput) -> Result<Self, ContractError> {
        let path = normalized_absolute_path("canonical_path", input.canonical_path)?;
        let (platform, units) = native_units(path.as_os_str());
        let provider_id = ProviderId::derive(
            input.artifact_fingerprint,
            input.kind.label().as_bytes(),
            platform,
            &units,
        );
        Ok(Self {
            provider_id,
            artifact_fingerprint: input.artifact_fingerprint,
            canonical_path: path,
            kind: input.kind,
            resolution: input.resolution,
            provenance: input.provenance,
            observed_target: input.observed_target,
        })
    }

    pub(crate) fn try_from_stored(
        stored_provider: ProviderId,
        input: ResolvedArtifactInput,
    ) -> Result<Self, ContractError> {
        let artifact = Self::try_new(input)?;
        if artifact.provider_id != stored_provider {
            return Err(ContractError::ProviderIdMismatch {
                stored: stored_provider,
                derived: artifact.provider_id,
            });
        }
        Ok(artifact)
    }

    pub const fn provider_id(&self) -> ProviderId {
        self.provider_id
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.artifact_fingerprint
    }

    pub fn canonical_path(&self) -> &Path {
        &self.canonical_path
    }

    pub const fn kind(&self) -> ArtifactKind {
        self.kind
    }

    pub fn resolution(&self) -> &ProviderResolution {
        &self.resolution
    }

    pub const fn provenance(&self) -> ProviderProvenance {
        self.provenance
    }

    pub fn observed_target(&self) -> &ObservedTarget {
        &self.observed_target
    }

    pub(crate) fn input(&self) -> ResolvedArtifactInput {
        ResolvedArtifactInput {
            artifact_fingerprint: self.artifact_fingerprint,
            canonical_path: self.canonical_path.clone(),
            kind: self.kind,
            resolution: self.resolution.clone(),
            provenance: self.provenance,
            observed_target: self.observed_target.clone(),
        }
    }
}

/// Unresolved native link input. Name requests and exact paths are distinct,
/// and order/repetition are always semantic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeInput {
    SearchNative(PathBuf),
    ObjectPath(PathBuf),
    StaticLibraryPath(PathBuf),
    DynamicLibraryPath(PathBuf),
    ImportLibraryPath(PathBuf),
    FrameworkPath(PathBuf),
    StaticLibraryName(OsString),
    DynamicLibraryName(OsString),
    ImportLibraryName(OsString),
    FrameworkName {
        name: OsString,
        search_path: Option<PathBuf>,
    },
    GroupStart,
    GroupEnd,
}

pub fn validate_native_inputs(inputs: &[NativeInput]) -> Result<(), ContractError> {
    let mut group_depth = 0_usize;
    for (index, input) in inputs.iter().enumerate() {
        match input {
            NativeInput::SearchNative(path)
            | NativeInput::ObjectPath(path)
            | NativeInput::StaticLibraryPath(path)
            | NativeInput::DynamicLibraryPath(path)
            | NativeInput::ImportLibraryPath(path)
            | NativeInput::FrameworkPath(path) => {
                normalized_absolute_path("native_input.path", path.clone())?;
            }
            NativeInput::StaticLibraryName(name)
            | NativeInput::DynamicLibraryName(name)
            | NativeInput::ImportLibraryName(name) => {
                validate_native_name("native_input.library_name", name)?;
            }
            NativeInput::FrameworkName { name, search_path } => {
                validate_native_name("native_input.framework.name", name)?;
                if let Some(path) = search_path {
                    normalized_absolute_path("native_input.framework.search_path", path.clone())?;
                }
            }
            NativeInput::GroupStart => group_depth += 1,
            NativeInput::GroupEnd => {
                if group_depth == 0 {
                    return Err(ContractError::UnexpectedGroupEnd { index });
                }
                group_depth -= 1;
            }
        }
    }
    if group_depth != 0 {
        return Err(ContractError::UnclosedGroups { depth: group_depth });
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactSymbolId {
    table_index: u32,
    symbol_index: u64,
}

impl ArtifactSymbolId {
    pub const fn new(table_index: u32, symbol_index: u64) -> Self {
        Self {
            table_index,
            symbol_index,
        }
    }

    pub const fn table_index(&self) -> u32 {
        self.table_index
    }

    pub const fn symbol_index(&self) -> u64 {
        self.symbol_index
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolDirection {
    Exported,
    Imported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Data,
    ThreadLocal,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolBinding {
    Global,
    Weak,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolVisibility {
    Default,
    Hidden,
    Protected,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SymbolDecoration {
    None,
    LeadingUnderscore,
    Stdcall { stack_bytes: u32 },
    Versioned { version: Vec<u8>, is_default: bool },
    Other { spelling: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolRecordInput {
    pub id: ArtifactSymbolId,
    pub name: String,
    pub raw_name: Vec<u8>,
    pub version: Option<Vec<u8>>,
    pub direction: SymbolDirection,
    pub kind: SymbolKind,
    pub binding: SymbolBinding,
    pub visibility: SymbolVisibility,
    pub decoration: SymbolDecoration,
    pub size: u64,
    pub address: Option<u64>,
    pub section: Option<Vec<u8>>,
    pub archive_member: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SymbolRecord {
    id: ArtifactSymbolId,
    name: String,
    raw_name: Vec<u8>,
    version: Option<Vec<u8>>,
    direction: SymbolDirection,
    kind: SymbolKind,
    binding: SymbolBinding,
    visibility: SymbolVisibility,
    decoration: SymbolDecoration,
    size: u64,
    address: Option<u64>,
    section: Option<Vec<u8>>,
    archive_member: Option<Vec<u8>>,
}

impl SymbolRecord {
    pub fn try_new(input: SymbolRecordInput) -> Result<Self, ContractError> {
        validate_text("symbol.name", &input.name)?;
        if input.raw_name.is_empty() || input.raw_name.contains(&0) {
            return Err(ContractError::InvalidText {
                field: "symbol.raw_name",
            });
        }
        Ok(Self {
            id: input.id,
            name: input.name,
            raw_name: input.raw_name,
            version: input.version,
            direction: input.direction,
            kind: input.kind,
            binding: input.binding,
            visibility: input.visibility,
            decoration: input.decoration,
            size: input.size,
            address: input.address,
            section: input.section,
            archive_member: input.archive_member,
        })
    }

    pub const fn id(&self) -> ArtifactSymbolId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn raw_name(&self) -> &[u8] {
        &self.raw_name
    }

    pub fn version(&self) -> Option<&[u8]> {
        self.version.as_deref()
    }

    pub const fn direction(&self) -> SymbolDirection {
        self.direction
    }

    pub const fn kind(&self) -> SymbolKind {
        self.kind
    }

    pub const fn binding(&self) -> SymbolBinding {
        self.binding
    }

    pub const fn visibility(&self) -> SymbolVisibility {
        self.visibility
    }

    pub fn decoration(&self) -> &SymbolDecoration {
        &self.decoration
    }

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub const fn address(&self) -> Option<u64> {
        self.address
    }

    pub fn section(&self) -> Option<&[u8]> {
        self.section.as_deref()
    }

    pub fn archive_member(&self) -> Option<&[u8]> {
        self.archive_member.as_deref()
    }

    pub fn is_visible_export(&self) -> bool {
        self.direction == SymbolDirection::Exported
            && matches!(self.binding, SymbolBinding::Global | SymbolBinding::Weak)
            && matches!(
                self.visibility,
                SymbolVisibility::Default | SymbolVisibility::Protected
            )
    }

    pub(crate) fn input(&self) -> SymbolRecordInput {
        SymbolRecordInput {
            id: self.id,
            name: self.name.clone(),
            raw_name: self.raw_name.clone(),
            version: self.version.clone(),
            direction: self.direction,
            kind: self.kind,
            binding: self.binding,
            visibility: self.visibility,
            decoration: self.decoration.clone(),
            size: self.size,
            address: self.address,
            section: self.section.clone(),
            archive_member: self.archive_member.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyEdge {
    requested: OsString,
    provider: Option<ProviderId>,
    provenance: DependencyProvenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyProvenance {
    DynamicTable,
    ImportTable,
    ArchiveDirective,
    LinkerOption,
    FrameworkMetadata,
}

impl DependencyEdge {
    pub fn try_new(
        requested: OsString,
        provider: Option<ProviderId>,
        provenance: DependencyProvenance,
    ) -> Result<Self, ContractError> {
        validate_native_string("dependency.requested", &requested)?;
        Ok(Self {
            requested,
            provider,
            provenance,
        })
    }

    pub fn requested(&self) -> &OsStr {
        &self.requested
    }

    pub const fn provider(&self) -> Option<ProviderId> {
        self.provider
    }

    pub const fn provenance(&self) -> DependencyProvenance {
        self.provenance
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectionToolKind {
    LincBuiltin,
    ObjectCrate,
    LlvmReadObj,
    GnuReadelf,
    GnuNm,
    Dumpbin,
    WasmTools,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectionParserKind {
    Archive,
    Elf,
    MachO,
    Coff,
    Pe,
    Wasm,
    Xcoff,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectionToolIdentity {
    kind: InspectionToolKind,
    version: String,
    implementation_fingerprint: ContentFingerprint,
}

impl InspectionToolIdentity {
    pub fn try_new(
        kind: InspectionToolKind,
        version: String,
        implementation_fingerprint: ContentFingerprint,
    ) -> Result<Self, ContractError> {
        validate_text("inspection.tool.version", &version)?;
        Ok(Self {
            kind,
            version,
            implementation_fingerprint,
        })
    }

    pub const fn kind(&self) -> InspectionToolKind {
        self.kind
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub const fn implementation_fingerprint(&self) -> ContentFingerprint {
        self.implementation_fingerprint
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectionParserIdentity {
    kind: InspectionParserKind,
    version: String,
    implementation_fingerprint: ContentFingerprint,
}

impl InspectionParserIdentity {
    pub fn try_new(
        kind: InspectionParserKind,
        version: String,
        implementation_fingerprint: ContentFingerprint,
    ) -> Result<Self, ContractError> {
        validate_text("inspection.parser.version", &version)?;
        Ok(Self {
            kind,
            version,
            implementation_fingerprint,
        })
    }

    pub const fn kind(&self) -> InspectionParserKind {
        self.kind
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub const fn implementation_fingerprint(&self) -> ContentFingerprint {
        self.implementation_fingerprint
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectionProvenance {
    tool: InspectionToolIdentity,
    parsers: Vec<InspectionParserIdentity>,
}

impl InspectionProvenance {
    pub fn try_new(
        tool: InspectionToolIdentity,
        mut parsers: Vec<InspectionParserIdentity>,
    ) -> Result<Self, ContractError> {
        parsers
            .sort_by(|left, right| inspection_parser_key(left).cmp(&inspection_parser_key(right)));
        if parsers.is_empty() {
            return Err(ContractError::InvalidInspectionProvenance {
                reason: "at least one parser identity is required",
            });
        }
        if parsers
            .windows(2)
            .any(|pair| inspection_parser_key(&pair[0]) == inspection_parser_key(&pair[1]))
        {
            return Err(ContractError::InvalidInspectionProvenance {
                reason: "parser identities must be distinct",
            });
        }
        Ok(Self { tool, parsers })
    }

    pub fn tool(&self) -> &InspectionToolIdentity {
        &self.tool
    }

    pub fn parsers(&self) -> &[InspectionParserIdentity] {
        &self.parsers
    }
}

fn inspection_parser_key(
    parser: &InspectionParserIdentity,
) -> (InspectionParserKind, &str, ContentFingerprint) {
    (
        parser.kind(),
        parser.version(),
        parser.implementation_fingerprint(),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolInventory {
    artifact: ResolvedArtifact,
    inspection: InspectionProvenance,
    symbols: Vec<SymbolRecord>,
    dependency_edges: Vec<DependencyEdge>,
}

impl SymbolInventory {
    pub fn try_new(
        artifact: ResolvedArtifact,
        inspection: InspectionProvenance,
        mut symbols: Vec<SymbolRecord>,
        dependency_edges: Vec<DependencyEdge>,
    ) -> Result<Self, ContractError> {
        symbols.sort_by_key(SymbolRecord::id);
        let mut seen = BTreeSet::new();
        for symbol in &symbols {
            if !seen.insert(symbol.id()) {
                return Err(ContractError::DuplicateArtifactSymbolId {
                    provider: artifact.provider_id(),
                    symbol: symbol.id(),
                });
            }
        }
        Ok(Self {
            artifact,
            inspection,
            symbols,
            dependency_edges,
        })
    }

    pub fn artifact(&self) -> &ResolvedArtifact {
        &self.artifact
    }

    pub fn symbols(&self) -> &[SymbolRecord] {
        &self.symbols
    }

    pub fn inspection(&self) -> &InspectionProvenance {
        &self.inspection
    }

    pub fn dependency_edges(&self) -> &[DependencyEdge] {
        &self.dependency_edges
    }

    pub fn symbol(&self, id: ArtifactSymbolId) -> Option<&SymbolRecord> {
        self.symbols
            .binary_search_by_key(&id, SymbolRecord::id)
            .ok()
            .map(|index| &self.symbols[index])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Note,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticStage {
    Intake,
    ArtifactResolution,
    SymbolInspection,
    LayoutProbe,
    Validation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LincCodeClass {
    Error,
    Note,
    Warning,
    Partial,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LincCode(String);

impl LincCode {
    pub fn try_new(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        let bytes = value.as_bytes();
        let valid = bytes.len() == 10
            && &bytes[..5] == b"LINC-"
            && matches!(bytes[5], b'E' | b'N' | b'P' | b'W')
            && bytes[6..].iter().all(u8::is_ascii_digit);
        if valid {
            Ok(Self(value))
        } else {
            Err(ContractError::InvalidDiagnosticCode { value })
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn class(&self) -> LincCodeClass {
        match self.0.as_bytes()[5] {
            b'E' => LincCodeClass::Error,
            b'N' => LincCodeClass::Note,
            b'W' => LincCodeClass::Warning,
            b'P' => LincCodeClass::Partial,
            _ => unreachable!("LincCode construction checks the class byte"),
        }
    }

    pub fn diagnostic_severity(&self) -> DiagnosticSeverity {
        match self.class() {
            LincCodeClass::Error => DiagnosticSeverity::Error,
            LincCodeClass::Note => DiagnosticSeverity::Note,
            LincCodeClass::Warning | LincCodeClass::Partial => DiagnosticSeverity::Warning,
        }
    }

    pub fn is_rejection(&self) -> bool {
        matches!(self.class(), LincCodeClass::Error | LincCodeClass::Partial)
    }
}

impl fmt::Display for LincCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for LincCode {
    type Err = ContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

impl Serialize for LincCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for LincCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DiagnosticEvidenceRef {
    Symbol {
        provider: ProviderId,
        symbol: ArtifactSymbolId,
    },
    Layout {
        declaration: DeclarationId,
    },
    Declaration {
        declaration: DeclarationId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LincDiagnosticContext {
    target_fingerprint: TargetFingerprint,
    source_range: Option<SourceRange>,
    native_input_index: Option<u32>,
    dependency_provider: Option<ProviderId>,
    probe: Option<ProbeEvidenceId>,
    evidence: Option<DiagnosticEvidenceRef>,
}

impl LincDiagnosticContext {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        target_fingerprint: TargetFingerprint,
        source_range: Option<SourceRange>,
        native_input_index: Option<u32>,
        dependency_provider: Option<ProviderId>,
        probe: Option<ProbeEvidenceId>,
        evidence: Option<DiagnosticEvidenceRef>,
    ) -> Self {
        Self {
            target_fingerprint,
            source_range,
            native_input_index,
            dependency_provider,
            probe,
            evidence,
        }
    }

    pub const fn target_fingerprint(&self) -> TargetFingerprint {
        self.target_fingerprint
    }

    pub const fn source_range(&self) -> Option<SourceRange> {
        self.source_range
    }

    pub const fn native_input_index(&self) -> Option<u32> {
        self.native_input_index
    }

    pub const fn dependency_provider(&self) -> Option<ProviderId> {
        self.dependency_provider
    }

    pub const fn probe(&self) -> Option<ProbeEvidenceId> {
        self.probe
    }

    pub const fn evidence(&self) -> Option<&DiagnosticEvidenceRef> {
        self.evidence.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LincDiagnosticInput {
    pub code: LincCode,
    pub severity: DiagnosticSeverity,
    pub stage: DiagnosticStage,
    pub message: String,
    pub declaration: Option<DeclarationId>,
    pub provider: Option<ProviderId>,
    pub context: LincDiagnosticContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LincDiagnostic {
    code: LincCode,
    severity: DiagnosticSeverity,
    stage: DiagnosticStage,
    message: String,
    declaration: Option<DeclarationId>,
    provider: Option<ProviderId>,
    context: LincDiagnosticContext,
}

impl LincDiagnostic {
    pub fn try_new(input: LincDiagnosticInput) -> Result<Self, ContractError> {
        validate_text("diagnostic.message", &input.message)?;
        let expected = input.code.diagnostic_severity();
        if input.severity != expected {
            return Err(ContractError::DiagnosticSeverityMismatch {
                code: input.code.to_string(),
                expected: diagnostic_severity_label(expected),
                actual: diagnostic_severity_label(input.severity),
            });
        }
        Ok(Self {
            code: input.code,
            severity: input.severity,
            stage: input.stage,
            message: input.message,
            declaration: input.declaration,
            provider: input.provider,
            context: input.context,
        })
    }

    pub fn code(&self) -> &LincCode {
        &self.code
    }

    pub const fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }

    pub const fn stage(&self) -> DiagnosticStage {
        self.stage
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn declaration(&self) -> Option<DeclarationId> {
        self.declaration
    }

    pub const fn provider(&self) -> Option<ProviderId> {
        self.provider
    }

    pub const fn context(&self) -> &LincDiagnosticContext {
        &self.context
    }
}

fn diagnostic_severity_label(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Note => "note",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Error => "error",
    }
}

pub(crate) fn validate_text(field: &'static str, value: &str) -> Result<(), ContractError> {
    if value.is_empty() || value.contains('\0') {
        Err(ContractError::InvalidText { field })
    } else {
        Ok(())
    }
}

pub(crate) fn validate_native_string(
    field: &'static str,
    value: &OsStr,
) -> Result<(), ContractError> {
    if value.is_empty() || native_has_nul(value) {
        Err(ContractError::InvalidNativeString { field })
    } else {
        Ok(())
    }
}

pub(crate) fn validate_native_name(
    field: &'static str,
    value: &OsStr,
) -> Result<(), ContractError> {
    validate_native_string(field, value)?;
    if native_has_separator(value) {
        return Err(ContractError::InvalidNativeString { field });
    }
    Ok(())
}

pub(crate) fn normalized_absolute_path(
    field: &'static str,
    path: PathBuf,
) -> Result<PathBuf, ContractError> {
    let rebuilt: PathBuf = path.components().collect();
    let valid = path.is_absolute()
        && !native_has_nul(path.as_os_str())
        && rebuilt.as_os_str() == path.as_os_str()
        && path
            .components()
            .all(|component| !matches!(component, Component::CurDir | Component::ParentDir));
    if valid {
        Ok(path)
    } else {
        Err(ContractError::InvalidPath { field, path })
    }
}

#[cfg(unix)]
pub(crate) fn native_units(value: &OsStr) -> (&'static [u8], Vec<u8>) {
    use std::os::unix::ffi::OsStrExt as _;
    (b"unix-bytes-v1", value.as_bytes().to_vec())
}

#[cfg(unix)]
pub(crate) fn native_has_nul(value: &OsStr) -> bool {
    use std::os::unix::ffi::OsStrExt as _;
    value.as_bytes().contains(&0)
}

#[cfg(unix)]
fn native_has_separator(value: &OsStr) -> bool {
    use std::os::unix::ffi::OsStrExt as _;
    value
        .as_bytes()
        .iter()
        .any(|byte| matches!(byte, b'/' | b'\\'))
}

#[cfg(windows)]
pub(crate) fn native_units(value: &OsStr) -> (&'static [u8], Vec<u8>) {
    use std::os::windows::ffi::OsStrExt as _;
    let units = value
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    (b"windows-utf16-v1", units)
}

#[cfg(windows)]
pub(crate) fn native_has_nul(value: &OsStr) -> bool {
    use std::os::windows::ffi::OsStrExt as _;
    value.encode_wide().any(|unit| unit == 0)
}

#[cfg(windows)]
fn native_has_separator(value: &OsStr) -> bool {
    use std::os::windows::ffi::OsStrExt as _;
    value
        .encode_wide()
        .any(|unit| unit == u16::from(b'/') || unit == u16::from(b'\\'))
}

fn abi_matches_target(abi: NativeAbi, target: &TargetSpec) -> bool {
    use Architecture as A;
    match abi {
        NativeAbi::SysV32 => matches!(target.architecture(), A::X86),
        NativeAbi::SysV64 => matches!(target.architecture(), A::X86_64),
        NativeAbi::Aapcs => matches!(target.architecture(), A::Arm),
        NativeAbi::Aapcs64 => matches!(target.architecture(), A::Aarch64),
        NativeAbi::Win32 => {
            target.operating_system() == OperatingSystem::Windows && target.pointer_width() == 32
        }
        NativeAbi::Win64 => {
            target.operating_system() == OperatingSystem::Windows && target.pointer_width() == 64
        }
        NativeAbi::RiscV32 => matches!(target.architecture(), A::RiscV32),
        NativeAbi::RiscV64 => matches!(target.architecture(), A::RiscV64),
        NativeAbi::PowerPc32SysV => matches!(target.architecture(), A::PowerPc),
        NativeAbi::PowerPc64ElfV1 | NativeAbi::PowerPc64ElfV2 => {
            matches!(target.architecture(), A::PowerPc64)
        }
        NativeAbi::S390x => matches!(target.architecture(), A::S390x),
        NativeAbi::MipsO32 => matches!(target.architecture(), A::Mips),
        NativeAbi::MipsN32 | NativeAbi::MipsN64 => matches!(target.architecture(), A::Mips64),
        NativeAbi::Wasm32 => matches!(target.architecture(), A::Wasm32),
        NativeAbi::Wasm64 => matches!(target.architecture(), A::Wasm64),
    }
}

fn linker_matches_target(linker: LinkerFlavor, target: &TargetSpec) -> bool {
    match linker {
        LinkerFlavor::Gnu => matches!(
            target.object_format(),
            ObjectFormat::Elf | ObjectFormat::Coff
        ),
        LinkerFlavor::Lld => true,
        LinkerFlavor::Darwin => target.object_format() == ObjectFormat::MachO,
        LinkerFlavor::Msvc => target.object_format() == ObjectFormat::Coff,
        LinkerFlavor::WasmLd => target.object_format() == ObjectFormat::Wasm,
    }
}

fn crt_matches_target(crt: CrtFlavor, target: &TargetSpec) -> bool {
    match crt {
        CrtFlavor::Glibc => matches!(
            target.environment(),
            Environment::Gnu | Environment::GnuAbi64
        ),
        CrtFlavor::Musl => target.environment() == Environment::Musl,
        CrtFlavor::Msvc => target.environment() == Environment::Msvc,
        CrtFlavor::Darwin => matches!(
            target.operating_system(),
            OperatingSystem::Darwin
                | OperatingSystem::MacOs
                | OperatingSystem::Ios
                | OperatingSystem::TvOs
                | OperatingSystem::WatchOs
        ),
        CrtFlavor::None => {
            target.operating_system() == OperatingSystem::None
                || target.object_format() == ObjectFormat::Wasm
        }
    }
}
