//! Private schema-v2 data-transfer objects.

use std::{
    ffi::{OsStr, OsString},
    path::PathBuf,
};

use parc::contract::{
    Architecture, ChildId, CompilerIdentity, ContentFingerprint, DeclarationId, Endian,
    Environment, ExactInteger, NormalizedCompilerArg, ObjectFormat, OperatingSystem, SchemaHeader,
    Signedness, SourceFingerprint, TargetFingerprint,
};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use super::{
    AbiProbeEvidence, AbiProbeEvidenceInput, AnalysisPolicy, AnalysisPolicyInput,
    ArtifactFingerprint, ArtifactKind, ArtifactSymbolId, CallableAbiAssessment, ContractError,
    CrtFlavor, DeclarationEvidence, DeclarationEvidenceInput, DependencyEdge, DependencyProvenance,
    DiagnosticSeverity, DiagnosticStage, EnumLayoutEvidence, EnumVariantEvidence,
    EvidenceAcceptancePolicy, EvidenceConfidence, EvidenceSource, FieldLayoutEvidence,
    InspectionParserIdentity, InspectionParserKind, InspectionProvenance, InspectionToolIdentity,
    InspectionToolKind, LayoutAssessment, LayoutEvidence, LincCode, LincDiagnostic,
    LincDiagnosticContext, LincDiagnosticInput, LinkAnalysisFingerprint, LinkAnalysisPackage,
    LinkAtom, LinkerFlavor, NativeAbi, NativeInput, ObservedTarget, ObservedTargetParts,
    ProbeCompilerArgument, ProbeEnvironmentEntry, ProbeEnvironmentIdentity, ProbeEnvironmentPolicy,
    ProbeEnvironmentValue, ProbeEvidenceId, ProbeExecutionPolicy, ProbeMethod, ProbePolicy,
    ProbeProcessResult, ProbeResourceLimits, ProbeRunnerArgument, ProbeRunnerEvidence,
    ProbeSubject, ProbeSubjectOutcome, ProviderAssessment, ProviderId, ProviderProvenance,
    ProviderResolution, RecordLayoutEvidence, ResolutionPolicy, ResolvedArtifact,
    ResolvedArtifactInput, ResolvedLinkPlan, RunnerCommand, RunnerPolicy, SymbolAssessment,
    SymbolBinding, SymbolDecoration, SymbolDirection, SymbolInventory, SymbolKind, SymbolRecord,
    SymbolRecordInput, SymbolVisibility, WeakSymbolPolicy,
};
use crate::contract::package::LinkAnalysisPackageParts;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawLinkAnalysisEnvelope {
    pub kind: String,
    pub schema: SchemaHeader,
    pub fingerprint: LinkAnalysisFingerprint,
    pub payload: Box<RawValue>,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LinkAnalysisEnvelope<'a> {
    pub kind: &'static str,
    pub schema: &'a SchemaHeader,
    pub fingerprint: LinkAnalysisFingerprint,
    pub payload: LinkAnalysisPackageWire,
}

/// Lossless host-native `OsString` representation. A decoder never guesses a
/// conversion for a foreign platform's path units.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "platform", rename_all = "snake_case", deny_unknown_fields)]
enum NativeStringWire {
    UnixBytes { bytes: Vec<u8> },
    WindowsUtf16 { units: Vec<u16> },
}

impl NativeStringWire {
    #[cfg(unix)]
    fn from_os_str(value: &OsStr) -> Self {
        use std::os::unix::ffi::OsStrExt as _;
        Self::UnixBytes {
            bytes: value.as_bytes().to_vec(),
        }
    }

    #[cfg(windows)]
    fn from_os_str(value: &OsStr) -> Self {
        use std::os::windows::ffi::OsStrExt as _;
        Self::WindowsUtf16 {
            units: value.encode_wide().collect(),
        }
    }

    #[cfg(unix)]
    fn into_os_string(self) -> Result<OsString, ContractError> {
        use std::os::unix::ffi::OsStringExt as _;
        match self {
            Self::UnixBytes { bytes } => Ok(OsString::from_vec(bytes)),
            Self::WindowsUtf16 { .. } => Err(ContractError::NativePathPlatform {
                found: "windows_utf16",
                host: "unix_bytes",
            }),
        }
    }

    #[cfg(windows)]
    fn into_os_string(self) -> Result<OsString, ContractError> {
        use std::os::windows::ffi::OsStringExt as _;
        match self {
            Self::WindowsUtf16 { units } => Ok(OsString::from_wide(&units)),
            Self::UnixBytes { .. } => Err(ContractError::NativePathPlatform {
                found: "unix_bytes",
                host: "windows_utf16",
            }),
        }
    }

    fn from_path(path: &std::path::Path) -> Self {
        Self::from_os_str(path.as_os_str())
    }

    fn into_path(self) -> Result<PathBuf, ContractError> {
        self.into_os_string().map(PathBuf::from)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum NativeInputWire {
    SearchNative(NativeStringWire),
    ObjectPath(NativeStringWire),
    StaticLibraryPath(NativeStringWire),
    DynamicLibraryPath(NativeStringWire),
    ImportLibraryPath(NativeStringWire),
    FrameworkPath(NativeStringWire),
    StaticLibraryName(NativeStringWire),
    DynamicLibraryName(NativeStringWire),
    ImportLibraryName(NativeStringWire),
    FrameworkName {
        name: NativeStringWire,
        search_path: Option<NativeStringWire>,
    },
    GroupStart,
    GroupEnd,
}

impl NativeInputWire {
    fn from_domain(input: &NativeInput) -> Self {
        match input {
            NativeInput::SearchNative(path) => {
                Self::SearchNative(NativeStringWire::from_path(path))
            }
            NativeInput::ObjectPath(path) => Self::ObjectPath(NativeStringWire::from_path(path)),
            NativeInput::StaticLibraryPath(path) => {
                Self::StaticLibraryPath(NativeStringWire::from_path(path))
            }
            NativeInput::DynamicLibraryPath(path) => {
                Self::DynamicLibraryPath(NativeStringWire::from_path(path))
            }
            NativeInput::ImportLibraryPath(path) => {
                Self::ImportLibraryPath(NativeStringWire::from_path(path))
            }
            NativeInput::FrameworkPath(path) => {
                Self::FrameworkPath(NativeStringWire::from_path(path))
            }
            NativeInput::StaticLibraryName(name) => {
                Self::StaticLibraryName(NativeStringWire::from_os_str(name))
            }
            NativeInput::DynamicLibraryName(name) => {
                Self::DynamicLibraryName(NativeStringWire::from_os_str(name))
            }
            NativeInput::ImportLibraryName(name) => {
                Self::ImportLibraryName(NativeStringWire::from_os_str(name))
            }
            NativeInput::FrameworkName { name, search_path } => Self::FrameworkName {
                name: NativeStringWire::from_os_str(name),
                search_path: search_path.as_deref().map(NativeStringWire::from_path),
            },
            NativeInput::GroupStart => Self::GroupStart,
            NativeInput::GroupEnd => Self::GroupEnd,
        }
    }

    fn into_domain(self) -> Result<NativeInput, ContractError> {
        match self {
            Self::SearchNative(path) => path.into_path().map(NativeInput::SearchNative),
            Self::ObjectPath(path) => path.into_path().map(NativeInput::ObjectPath),
            Self::StaticLibraryPath(path) => path.into_path().map(NativeInput::StaticLibraryPath),
            Self::DynamicLibraryPath(path) => path.into_path().map(NativeInput::DynamicLibraryPath),
            Self::ImportLibraryPath(path) => path.into_path().map(NativeInput::ImportLibraryPath),
            Self::FrameworkPath(path) => path.into_path().map(NativeInput::FrameworkPath),
            Self::StaticLibraryName(name) => {
                name.into_os_string().map(NativeInput::StaticLibraryName)
            }
            Self::DynamicLibraryName(name) => {
                name.into_os_string().map(NativeInput::DynamicLibraryName)
            }
            Self::ImportLibraryName(name) => {
                name.into_os_string().map(NativeInput::ImportLibraryName)
            }
            Self::FrameworkName { name, search_path } => Ok(NativeInput::FrameworkName {
                name: name.into_os_string()?,
                search_path: search_path.map(NativeStringWire::into_path).transpose()?,
            }),
            Self::GroupStart => Ok(NativeInput::GroupStart),
            Self::GroupEnd => Ok(NativeInput::GroupEnd),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeEnvironmentEntryWire {
    name: String,
    value: ProbeEnvironmentValue,
}

impl ProbeEnvironmentEntryWire {
    fn from_domain(entry: &ProbeEnvironmentEntry) -> Self {
        Self {
            name: entry.name().to_owned(),
            value: entry.value(),
        }
    }

    fn into_domain(self) -> Result<ProbeEnvironmentEntry, ContractError> {
        ProbeEnvironmentEntry::try_new(self.name, self.value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeEnvironmentIdentityWire {
    policy: ProbeEnvironmentPolicy,
    entries: Vec<ProbeEnvironmentEntryWire>,
    fingerprint: ContentFingerprint,
}

impl ProbeEnvironmentIdentityWire {
    fn from_domain(identity: &ProbeEnvironmentIdentity) -> Self {
        Self {
            policy: identity.policy(),
            entries: identity
                .entries()
                .iter()
                .map(ProbeEnvironmentEntryWire::from_domain)
                .collect(),
            fingerprint: identity.fingerprint(),
        }
    }

    fn into_domain(self) -> Result<ProbeEnvironmentIdentity, ContractError> {
        if !self
            .entries
            .windows(2)
            .all(|pair| pair[0].name < pair[1].name)
        {
            return Err(ContractError::NonCanonicalOrder {
                collection: "probe environment entries",
            });
        }
        ProbeEnvironmentIdentity::try_from_stored(
            self.policy,
            self.entries
                .into_iter()
                .map(ProbeEnvironmentEntryWire::into_domain)
                .collect::<Result<_, _>>()?,
            self.fingerprint,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeExecutionPolicyWire {
    temporary_parent: NativeStringWire,
    environment: ProbeEnvironmentIdentityWire,
    limits: ProbeResourceLimits,
}

impl ProbeExecutionPolicyWire {
    fn from_domain(policy: &ProbeExecutionPolicy) -> Self {
        Self {
            temporary_parent: NativeStringWire::from_path(policy.temporary_parent()),
            environment: ProbeEnvironmentIdentityWire::from_domain(policy.environment()),
            limits: policy.limits(),
        }
    }

    fn into_domain(self) -> Result<ProbeExecutionPolicy, ContractError> {
        ProbeExecutionPolicy::from_checked_parts(
            self.temporary_parent.into_path()?,
            self.environment.into_domain()?,
            self.limits,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum RunnerPolicyWire {
    Unavailable,
    Explicit {
        program: NativeStringWire,
        executable_fingerprint: ArtifactFingerprint,
        arguments: Vec<ProbeRunnerArgumentWire>,
    },
}

impl RunnerPolicyWire {
    fn from_domain(policy: &RunnerPolicy) -> Self {
        match policy {
            RunnerPolicy::Unavailable => Self::Unavailable,
            RunnerPolicy::Explicit(command) => Self::Explicit {
                program: NativeStringWire::from_path(command.program()),
                executable_fingerprint: command.executable_fingerprint(),
                arguments: command
                    .arguments()
                    .iter()
                    .map(ProbeRunnerArgumentWire::from_domain)
                    .collect(),
            },
        }
    }

    fn into_domain(self) -> Result<RunnerPolicy, ContractError> {
        match self {
            Self::Unavailable => Ok(RunnerPolicy::Unavailable),
            Self::Explicit {
                program,
                executable_fingerprint,
                arguments,
            } => RunnerCommand::try_new(
                program.into_path()?,
                executable_fingerprint,
                arguments
                    .into_iter()
                    .map(ProbeRunnerArgumentWire::into_domain)
                    .collect::<Result<_, _>>()?,
            )
            .map(RunnerPolicy::Explicit),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AnalysisPolicyWire {
    resolution: ResolutionPolicy,
    probe: ProbePolicy,
    runner: RunnerPolicyWire,
    layout_evidence: EvidenceAcceptancePolicy,
    callable_abi_evidence: EvidenceAcceptancePolicy,
    weak_symbols: WeakSymbolPolicy,
    probe_execution: ProbeExecutionPolicyWire,
}

impl AnalysisPolicyWire {
    fn from_domain(policy: &AnalysisPolicy) -> Self {
        Self {
            resolution: policy.resolution(),
            probe: policy.probe(),
            runner: RunnerPolicyWire::from_domain(policy.runner()),
            layout_evidence: policy.layout_evidence(),
            callable_abi_evidence: policy.callable_abi_evidence(),
            weak_symbols: policy.weak_symbols(),
            probe_execution: ProbeExecutionPolicyWire::from_domain(policy.probe_execution()),
        }
    }

    fn into_domain(self) -> Result<AnalysisPolicy, ContractError> {
        AnalysisPolicy::try_new(AnalysisPolicyInput {
            resolution: self.resolution,
            probe: self.probe,
            runner: self.runner.into_domain()?,
            layout_evidence: self.layout_evidence,
            callable_abi_evidence: self.callable_abi_evidence,
            weak_symbols: self.weak_symbols,
            probe_execution: self.probe_execution.into_domain()?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservedTargetWire {
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

impl ObservedTargetWire {
    fn from_domain(target: &ObservedTarget) -> Self {
        let parts = target.parts();
        Self {
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
        }
    }

    fn into_domain(self) -> Result<ObservedTarget, ContractError> {
        ObservedTarget::try_new(ObservedTargetParts {
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
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolvedArtifactWire {
    provider_id: ProviderId,
    artifact_fingerprint: ArtifactFingerprint,
    canonical_path: NativeStringWire,
    kind: ArtifactKind,
    resolution: ProviderResolution,
    provenance: ProviderProvenance,
    observed_target: ObservedTargetWire,
}

impl ResolvedArtifactWire {
    fn from_domain(artifact: &ResolvedArtifact) -> Self {
        let input = artifact.input();
        Self {
            provider_id: artifact.provider_id(),
            artifact_fingerprint: input.artifact_fingerprint,
            canonical_path: NativeStringWire::from_path(&input.canonical_path),
            kind: input.kind,
            resolution: input.resolution,
            provenance: input.provenance,
            observed_target: ObservedTargetWire::from_domain(&input.observed_target),
        }
    }

    fn into_domain(self) -> Result<ResolvedArtifact, ContractError> {
        ResolvedArtifact::try_from_stored(
            self.provider_id,
            ResolvedArtifactInput {
                artifact_fingerprint: self.artifact_fingerprint,
                canonical_path: self.canonical_path.into_path()?,
                kind: self.kind,
                resolution: self.resolution,
                provenance: self.provenance,
                observed_target: self.observed_target.into_domain()?,
            },
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SymbolRecordWire {
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

impl SymbolRecordWire {
    fn from_domain(symbol: &SymbolRecord) -> Self {
        let input = symbol.input();
        Self {
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
        }
    }

    fn into_domain(self) -> Result<SymbolRecord, ContractError> {
        SymbolRecord::try_new(SymbolRecordInput {
            id: self.id,
            name: self.name,
            raw_name: self.raw_name,
            version: self.version,
            direction: self.direction,
            kind: self.kind,
            binding: self.binding,
            visibility: self.visibility,
            decoration: self.decoration,
            size: self.size,
            address: self.address,
            section: self.section,
            archive_member: self.archive_member,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DependencyEdgeWire {
    requested: NativeStringWire,
    provider: Option<ProviderId>,
    provenance: DependencyProvenance,
}

impl DependencyEdgeWire {
    fn from_domain(edge: &DependencyEdge) -> Self {
        Self {
            requested: NativeStringWire::from_os_str(edge.requested()),
            provider: edge.provider(),
            provenance: edge.provenance(),
        }
    }

    fn into_domain(self) -> Result<DependencyEdge, ContractError> {
        DependencyEdge::try_new(
            self.requested.into_os_string()?,
            self.provider,
            self.provenance,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InspectionToolIdentityWire {
    kind: InspectionToolKind,
    version: String,
    implementation_fingerprint: ContentFingerprint,
}

impl InspectionToolIdentityWire {
    fn from_domain(identity: &InspectionToolIdentity) -> Self {
        Self {
            kind: identity.kind(),
            version: identity.version().to_owned(),
            implementation_fingerprint: identity.implementation_fingerprint(),
        }
    }

    fn into_domain(self) -> Result<InspectionToolIdentity, ContractError> {
        InspectionToolIdentity::try_new(self.kind, self.version, self.implementation_fingerprint)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InspectionParserIdentityWire {
    kind: InspectionParserKind,
    version: String,
    implementation_fingerprint: ContentFingerprint,
}

impl InspectionParserIdentityWire {
    fn from_domain(identity: &InspectionParserIdentity) -> Self {
        Self {
            kind: identity.kind(),
            version: identity.version().to_owned(),
            implementation_fingerprint: identity.implementation_fingerprint(),
        }
    }

    fn into_domain(self) -> Result<InspectionParserIdentity, ContractError> {
        InspectionParserIdentity::try_new(self.kind, self.version, self.implementation_fingerprint)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InspectionProvenanceWire {
    tool: InspectionToolIdentityWire,
    parsers: Vec<InspectionParserIdentityWire>,
}

impl InspectionProvenanceWire {
    fn from_domain(provenance: &InspectionProvenance) -> Self {
        Self {
            tool: InspectionToolIdentityWire::from_domain(provenance.tool()),
            parsers: provenance
                .parsers()
                .iter()
                .map(InspectionParserIdentityWire::from_domain)
                .collect(),
        }
    }

    fn into_domain(self) -> Result<InspectionProvenance, ContractError> {
        let original_keys = self
            .parsers
            .iter()
            .map(|parser| {
                (
                    parser.kind,
                    parser.version.as_str(),
                    parser.implementation_fingerprint,
                )
            })
            .collect::<Vec<_>>();
        if !original_keys.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(ContractError::NonCanonicalOrder {
                collection: "inspection parsers",
            });
        }
        InspectionProvenance::try_new(
            self.tool.into_domain()?,
            self.parsers
                .into_iter()
                .map(InspectionParserIdentityWire::into_domain)
                .collect::<Result<_, _>>()?,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SymbolInventoryWire {
    artifact: ResolvedArtifactWire,
    inspection: InspectionProvenanceWire,
    symbols: Vec<SymbolRecordWire>,
    dependency_edges: Vec<DependencyEdgeWire>,
}

impl SymbolInventoryWire {
    fn from_domain(inventory: &SymbolInventory) -> Self {
        Self {
            artifact: ResolvedArtifactWire::from_domain(inventory.artifact()),
            inspection: InspectionProvenanceWire::from_domain(inventory.inspection()),
            symbols: inventory
                .symbols()
                .iter()
                .map(SymbolRecordWire::from_domain)
                .collect(),
            dependency_edges: inventory
                .dependency_edges()
                .iter()
                .map(DependencyEdgeWire::from_domain)
                .collect(),
        }
    }

    fn into_domain(self) -> Result<SymbolInventory, ContractError> {
        require_strict_order(&self.symbols, |symbol| symbol.id, "inventory symbols")?;
        SymbolInventory::try_new(
            self.artifact.into_domain()?,
            self.inspection.into_domain()?,
            self.symbols
                .into_iter()
                .map(SymbolRecordWire::into_domain)
                .collect::<Result<_, _>>()?,
            self.dependency_edges
                .into_iter()
                .map(DependencyEdgeWire::into_domain)
                .collect::<Result<_, _>>()?,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum ProbeCompilerArgumentWire {
    Literal(NativeStringWire),
    ProbeSource,
    OutputArtifact,
}

impl ProbeCompilerArgumentWire {
    fn from_domain(argument: &ProbeCompilerArgument) -> Self {
        match argument {
            ProbeCompilerArgument::Literal(value) => {
                Self::Literal(NativeStringWire::from_os_str(value))
            }
            ProbeCompilerArgument::ProbeSource => Self::ProbeSource,
            ProbeCompilerArgument::OutputArtifact => Self::OutputArtifact,
        }
    }

    fn into_domain(self) -> Result<ProbeCompilerArgument, ContractError> {
        match self {
            Self::Literal(value) => value.into_os_string().map(ProbeCompilerArgument::Literal),
            Self::ProbeSource => Ok(ProbeCompilerArgument::ProbeSource),
            Self::OutputArtifact => Ok(ProbeCompilerArgument::OutputArtifact),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum ProbeRunnerArgumentWire {
    Literal(NativeStringWire),
    ProbeExecutable,
}

impl ProbeRunnerArgumentWire {
    fn from_domain(argument: &ProbeRunnerArgument) -> Self {
        match argument {
            ProbeRunnerArgument::Literal(value) => {
                Self::Literal(NativeStringWire::from_os_str(value))
            }
            ProbeRunnerArgument::ProbeExecutable => Self::ProbeExecutable,
        }
    }

    fn into_domain(self) -> Result<ProbeRunnerArgument, ContractError> {
        match self {
            Self::Literal(value) => value.into_os_string().map(ProbeRunnerArgument::Literal),
            Self::ProbeExecutable => Ok(ProbeRunnerArgument::ProbeExecutable),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum ProbeRunnerEvidenceWire {
    NotExecuted,
    Executed {
        executable_path: NativeStringWire,
        executable_fingerprint: ArtifactFingerprint,
        arguments: Vec<ProbeRunnerArgumentWire>,
    },
}

impl ProbeRunnerEvidenceWire {
    fn from_domain(runner: &ProbeRunnerEvidence) -> Self {
        match runner {
            ProbeRunnerEvidence::NotExecuted => Self::NotExecuted,
            ProbeRunnerEvidence::Executed {
                executable_path,
                executable_fingerprint,
                arguments,
            } => Self::Executed {
                executable_path: NativeStringWire::from_path(executable_path),
                executable_fingerprint: *executable_fingerprint,
                arguments: arguments
                    .iter()
                    .map(ProbeRunnerArgumentWire::from_domain)
                    .collect(),
            },
        }
    }

    fn into_domain(self) -> Result<ProbeRunnerEvidence, ContractError> {
        match self {
            Self::NotExecuted => Ok(ProbeRunnerEvidence::NotExecuted),
            Self::Executed {
                executable_path,
                executable_fingerprint,
                arguments,
            } => Ok(ProbeRunnerEvidence::Executed {
                executable_path: executable_path.into_path()?,
                executable_fingerprint,
                arguments: arguments
                    .into_iter()
                    .map(ProbeRunnerArgumentWire::into_domain)
                    .collect::<Result<_, _>>()?,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AbiProbeEvidenceWire {
    id: ProbeEvidenceId,
    source_fingerprint: SourceFingerprint,
    target_fingerprint: TargetFingerprint,
    compiler: CompilerIdentity,
    compiler_executable: NativeStringWire,
    compiler_arguments: Vec<ProbeCompilerArgumentWire>,
    abi_flags: Vec<NormalizedCompilerArg>,
    probe_source_fingerprint: ContentFingerprint,
    subjects: Vec<ProbeSubject>,
    method: ProbeMethod,
    execution_policy: ProbeExecutionPolicyWire,
    compile_result: ProbeProcessResult,
    runner: ProbeRunnerEvidenceWire,
    execution_result: Option<ProbeProcessResult>,
    subject_outcomes: Vec<ProbeSubjectOutcome>,
}

impl AbiProbeEvidenceWire {
    fn from_domain(evidence: &AbiProbeEvidence) -> Self {
        let input = evidence.input();
        Self {
            id: evidence.id(),
            source_fingerprint: input.source_fingerprint,
            target_fingerprint: input.target_fingerprint,
            compiler: input.compiler,
            compiler_executable: NativeStringWire::from_path(&input.compiler_executable),
            compiler_arguments: input
                .compiler_arguments
                .iter()
                .map(ProbeCompilerArgumentWire::from_domain)
                .collect(),
            abi_flags: input.abi_flags,
            probe_source_fingerprint: input.probe_source_fingerprint,
            subjects: input.subjects,
            method: input.method,
            execution_policy: ProbeExecutionPolicyWire::from_domain(&input.execution_policy),
            compile_result: input.compile_result,
            runner: ProbeRunnerEvidenceWire::from_domain(&input.runner),
            execution_result: input.execution_result,
            subject_outcomes: input.subject_outcomes,
        }
    }

    fn into_domain(self) -> Result<AbiProbeEvidence, ContractError> {
        require_strict_order(&self.subjects, |subject| *subject, "ABI probe subjects")?;
        require_strict_order(
            &self.subject_outcomes,
            ProbeSubjectOutcome::subject,
            "ABI probe subject outcomes",
        )?;
        AbiProbeEvidence::try_from_stored(
            self.id,
            AbiProbeEvidenceInput {
                source_fingerprint: self.source_fingerprint,
                target_fingerprint: self.target_fingerprint,
                compiler: self.compiler,
                compiler_executable: self.compiler_executable.into_path()?,
                compiler_arguments: self
                    .compiler_arguments
                    .into_iter()
                    .map(ProbeCompilerArgumentWire::into_domain)
                    .collect::<Result<_, _>>()?,
                abi_flags: self.abi_flags,
                probe_source_fingerprint: self.probe_source_fingerprint,
                subjects: self.subjects,
                method: self.method,
                execution_policy: self.execution_policy.into_domain()?,
                compile_result: self.compile_result,
                runner: self.runner.into_domain()?,
                execution_result: self.execution_result,
                subject_outcomes: self.subject_outcomes,
            },
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FieldLayoutEvidenceWire {
    child: ChildId,
    offset_bits: u64,
    size_bits: Option<u64>,
    alignment_bits: Option<u32>,
}

impl FieldLayoutEvidenceWire {
    fn from_domain(evidence: &FieldLayoutEvidence) -> Self {
        Self {
            child: evidence.child(),
            offset_bits: evidence.offset_bits(),
            size_bits: evidence.size_bits(),
            alignment_bits: evidence.alignment_bits(),
        }
    }

    fn into_domain(self) -> Result<FieldLayoutEvidence, ContractError> {
        FieldLayoutEvidence::try_new(
            self.child,
            self.offset_bits,
            self.size_bits,
            self.alignment_bits,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnumVariantEvidenceWire {
    child: ChildId,
    value: ExactInteger,
}

impl EnumVariantEvidenceWire {
    fn from_domain(evidence: &EnumVariantEvidence) -> Self {
        Self {
            child: evidence.child(),
            value: *evidence.value(),
        }
    }

    fn into_domain(self) -> EnumVariantEvidence {
        EnumVariantEvidence::new(self.child, self.value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordLayoutEvidenceWire {
    declaration: DeclarationId,
    source_fingerprint: SourceFingerprint,
    target_fingerprint: TargetFingerprint,
    size_bits: u64,
    alignment_bits: u32,
    fields: Vec<FieldLayoutEvidenceWire>,
    probe: ProbeEvidenceId,
    source: EvidenceSource,
    confidence: EvidenceConfidence,
}

impl RecordLayoutEvidenceWire {
    fn from_domain(evidence: &RecordLayoutEvidence) -> Self {
        Self {
            declaration: evidence.declaration(),
            source_fingerprint: evidence.source_fingerprint(),
            target_fingerprint: evidence.target_fingerprint(),
            size_bits: evidence.size_bits(),
            alignment_bits: evidence.alignment_bits(),
            fields: evidence
                .fields()
                .iter()
                .map(FieldLayoutEvidenceWire::from_domain)
                .collect(),
            probe: evidence.probe(),
            source: evidence.source(),
            confidence: evidence.confidence(),
        }
    }

    fn into_domain(self) -> Result<RecordLayoutEvidence, ContractError> {
        require_strict_order(&self.fields, |field| field.child, "record layout fields")?;
        RecordLayoutEvidence::try_new(
            self.declaration,
            self.source_fingerprint,
            self.target_fingerprint,
            self.size_bits,
            self.alignment_bits,
            self.fields
                .into_iter()
                .map(FieldLayoutEvidenceWire::into_domain)
                .collect::<Result<_, _>>()?,
            self.probe,
            self.source,
            self.confidence,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnumLayoutEvidenceWire {
    declaration: DeclarationId,
    source_fingerprint: SourceFingerprint,
    target_fingerprint: TargetFingerprint,
    storage_bits: u64,
    alignment_bits: u32,
    signedness: Signedness,
    variants: Vec<EnumVariantEvidenceWire>,
    probe: ProbeEvidenceId,
    source: EvidenceSource,
    confidence: EvidenceConfidence,
}

impl EnumLayoutEvidenceWire {
    fn from_domain(evidence: &EnumLayoutEvidence) -> Self {
        Self {
            declaration: evidence.declaration(),
            source_fingerprint: evidence.source_fingerprint(),
            target_fingerprint: evidence.target_fingerprint(),
            storage_bits: evidence.storage_bits(),
            alignment_bits: evidence.alignment_bits(),
            signedness: evidence.signedness(),
            variants: evidence
                .variants()
                .iter()
                .map(EnumVariantEvidenceWire::from_domain)
                .collect(),
            probe: evidence.probe(),
            source: evidence.source(),
            confidence: evidence.confidence(),
        }
    }

    fn into_domain(self) -> Result<EnumLayoutEvidence, ContractError> {
        require_strict_order(
            &self.variants,
            |variant| variant.child,
            "enum layout variants",
        )?;
        EnumLayoutEvidence::try_new(
            self.declaration,
            self.source_fingerprint,
            self.target_fingerprint,
            self.storage_bits,
            self.alignment_bits,
            self.signedness,
            self.variants
                .into_iter()
                .map(EnumVariantEvidenceWire::into_domain)
                .collect(),
            self.probe,
            self.source,
            self.confidence,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "evidence",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum LayoutEvidenceWire {
    Record(RecordLayoutEvidenceWire),
    Enum(EnumLayoutEvidenceWire),
}

impl LayoutEvidenceWire {
    fn from_domain(evidence: &LayoutEvidence) -> Self {
        match evidence {
            LayoutEvidence::Record(record) => {
                Self::Record(RecordLayoutEvidenceWire::from_domain(record))
            }
            LayoutEvidence::Enum(enumeration) => {
                Self::Enum(EnumLayoutEvidenceWire::from_domain(enumeration))
            }
        }
    }

    fn into_domain(self) -> Result<LayoutEvidence, ContractError> {
        match self {
            Self::Record(record) => record.into_domain().map(LayoutEvidence::Record),
            Self::Enum(enumeration) => enumeration.into_domain().map(LayoutEvidence::Enum),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeclarationEvidenceWire {
    declaration: DeclarationId,
    source_fingerprint: SourceFingerprint,
    target_fingerprint: TargetFingerprint,
    provider: ProviderAssessment,
    symbol: SymbolAssessment,
    layout: LayoutAssessment,
    callable_abi: CallableAbiAssessment,
}

impl DeclarationEvidenceWire {
    fn from_domain(evidence: &DeclarationEvidence) -> Self {
        Self {
            declaration: evidence.declaration(),
            source_fingerprint: evidence.source_fingerprint(),
            target_fingerprint: evidence.target_fingerprint(),
            provider: evidence.provider().clone(),
            symbol: evidence.symbol().clone(),
            layout: evidence.layout().clone(),
            callable_abi: evidence.callable_abi().clone(),
        }
    }

    fn into_domain(self) -> Result<DeclarationEvidence, ContractError> {
        DeclarationEvidence::try_from_wire(DeclarationEvidenceInput {
            declaration: self.declaration,
            source_fingerprint: self.source_fingerprint,
            target_fingerprint: self.target_fingerprint,
            provider: self.provider,
            symbol: self.symbol,
            layout: self.layout,
            callable_abi: self.callable_abi,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LincDiagnosticWire {
    code: LincCode,
    severity: DiagnosticSeverity,
    stage: DiagnosticStage,
    message: String,
    declaration: Option<DeclarationId>,
    provider: Option<ProviderId>,
    context: LincDiagnosticContext,
}

impl LincDiagnosticWire {
    fn from_domain(diagnostic: &LincDiagnostic) -> Self {
        Self {
            code: diagnostic.code().to_owned(),
            severity: diagnostic.severity(),
            stage: diagnostic.stage(),
            message: diagnostic.message().to_owned(),
            declaration: diagnostic.declaration(),
            provider: diagnostic.provider(),
            context: diagnostic.context().clone(),
        }
    }

    fn into_domain(self) -> Result<LincDiagnostic, ContractError> {
        LincDiagnostic::try_new(LincDiagnosticInput {
            code: self.code,
            severity: self.severity,
            stage: self.stage,
            message: self.message,
            declaration: self.declaration,
            provider: self.provider,
            context: self.context,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum LinkAtomWire {
    SearchNative(NativeStringWire),
    Object(ResolvedArtifactWire),
    StaticLibrary(ResolvedArtifactWire),
    DynamicLibrary(ResolvedArtifactWire),
    ImportLibrary(ResolvedArtifactWire),
    Framework {
        name: NativeStringWire,
        search_path: NativeStringWire,
        artifact: ResolvedArtifactWire,
    },
    GroupStart,
    GroupEnd,
}

impl LinkAtomWire {
    fn from_domain(atom: &LinkAtom) -> Self {
        match atom {
            LinkAtom::SearchNative(path) => Self::SearchNative(NativeStringWire::from_path(path)),
            LinkAtom::Object(artifact) => Self::Object(ResolvedArtifactWire::from_domain(artifact)),
            LinkAtom::StaticLibrary(artifact) => {
                Self::StaticLibrary(ResolvedArtifactWire::from_domain(artifact))
            }
            LinkAtom::DynamicLibrary(artifact) => {
                Self::DynamicLibrary(ResolvedArtifactWire::from_domain(artifact))
            }
            LinkAtom::ImportLibrary(artifact) => {
                Self::ImportLibrary(ResolvedArtifactWire::from_domain(artifact))
            }
            LinkAtom::Framework {
                name,
                search_path,
                artifact,
            } => Self::Framework {
                name: NativeStringWire::from_os_str(name),
                search_path: NativeStringWire::from_path(search_path),
                artifact: ResolvedArtifactWire::from_domain(artifact),
            },
            LinkAtom::GroupStart => Self::GroupStart,
            LinkAtom::GroupEnd => Self::GroupEnd,
        }
    }

    fn into_domain(self) -> Result<LinkAtom, ContractError> {
        match self {
            Self::SearchNative(path) => Ok(LinkAtom::SearchNative(path.into_path()?)),
            Self::Object(artifact) => artifact.into_domain().map(LinkAtom::Object),
            Self::StaticLibrary(artifact) => artifact.into_domain().map(LinkAtom::StaticLibrary),
            Self::DynamicLibrary(artifact) => artifact.into_domain().map(LinkAtom::DynamicLibrary),
            Self::ImportLibrary(artifact) => artifact.into_domain().map(LinkAtom::ImportLibrary),
            Self::Framework {
                name,
                search_path,
                artifact,
            } => Ok(LinkAtom::Framework {
                name: name.into_os_string()?,
                search_path: search_path.into_path()?,
                artifact: artifact.into_domain()?,
            }),
            Self::GroupStart => Ok(LinkAtom::GroupStart),
            Self::GroupEnd => Ok(LinkAtom::GroupEnd),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
struct ResolvedLinkPlanWire(Vec<LinkAtomWire>);

impl ResolvedLinkPlanWire {
    fn from_domain(plan: &ResolvedLinkPlan) -> Self {
        Self(plan.atoms().iter().map(LinkAtomWire::from_domain).collect())
    }

    fn into_domain(self) -> Result<ResolvedLinkPlan, ContractError> {
        ResolvedLinkPlan::try_new(
            self.0
                .into_iter()
                .map(LinkAtomWire::into_domain)
                .collect::<Result<_, _>>()?,
        )
    }

    fn len(&self) -> usize {
        self.0.len()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LinkAnalysisPackageWire {
    pub schema: SchemaHeader,
    pub fingerprint: LinkAnalysisFingerprint,
    pub source_fingerprint: SourceFingerprint,
    pub target_fingerprint: TargetFingerprint,
    analysis_policy: AnalysisPolicyWire,
    native_inputs: Vec<NativeInputWire>,
    inventories: Vec<SymbolInventoryWire>,
    abi_probes: Vec<AbiProbeEvidenceWire>,
    layouts: Vec<LayoutEvidenceWire>,
    declaration_evidence: Vec<DeclarationEvidenceWire>,
    resolved_link_plan: ResolvedLinkPlanWire,
    diagnostics: Vec<LincDiagnosticWire>,
}

impl LinkAnalysisPackageWire {
    pub(crate) fn from_domain(package: &LinkAnalysisPackage) -> Self {
        Self {
            schema: package.schema().clone(),
            fingerprint: package.fingerprint(),
            source_fingerprint: package.source_fingerprint(),
            target_fingerprint: package.target_fingerprint(),
            analysis_policy: AnalysisPolicyWire::from_domain(package.analysis_policy()),
            native_inputs: package
                .native_inputs()
                .iter()
                .map(NativeInputWire::from_domain)
                .collect(),
            inventories: package
                .inventories()
                .iter()
                .map(SymbolInventoryWire::from_domain)
                .collect(),
            abi_probes: package
                .abi_probes()
                .iter()
                .map(AbiProbeEvidenceWire::from_domain)
                .collect(),
            layouts: package
                .layouts()
                .iter()
                .map(LayoutEvidenceWire::from_domain)
                .collect(),
            declaration_evidence: package
                .declaration_evidence()
                .iter()
                .map(DeclarationEvidenceWire::from_domain)
                .collect(),
            resolved_link_plan: ResolvedLinkPlanWire::from_domain(package.resolved_link_plan()),
            diagnostics: package
                .diagnostics()
                .iter()
                .map(LincDiagnosticWire::from_domain)
                .collect(),
        }
    }

    pub(crate) fn into_domain(self) -> Result<LinkAnalysisPackage, ContractError> {
        LinkAnalysisPackage::from_parts(LinkAnalysisPackageParts {
            schema: self.schema,
            fingerprint: self.fingerprint,
            source_fingerprint: self.source_fingerprint,
            target_fingerprint: self.target_fingerprint,
            analysis_policy: self.analysis_policy.into_domain()?,
            native_inputs: self
                .native_inputs
                .into_iter()
                .map(NativeInputWire::into_domain)
                .collect::<Result<_, _>>()?,
            inventories: self
                .inventories
                .into_iter()
                .map(SymbolInventoryWire::into_domain)
                .collect::<Result<_, _>>()?,
            abi_probes: self
                .abi_probes
                .into_iter()
                .map(AbiProbeEvidenceWire::into_domain)
                .collect::<Result<_, _>>()?,
            layouts: self
                .layouts
                .into_iter()
                .map(LayoutEvidenceWire::into_domain)
                .collect::<Result<_, _>>()?,
            declaration_evidence: self
                .declaration_evidence
                .into_iter()
                .map(DeclarationEvidenceWire::into_domain)
                .collect::<Result<_, _>>()?,
            resolved_link_plan: self.resolved_link_plan.into_domain()?,
            diagnostics: self
                .diagnostics
                .into_iter()
                .map(LincDiagnosticWire::into_domain)
                .collect::<Result<_, _>>()?,
        })
    }

    pub(crate) fn check_limits(
        &self,
        limits: super::DecodeLimits,
    ) -> Result<(), super::DecodeError> {
        let symbols = self
            .inventories
            .iter()
            .map(|inventory| inventory.symbols.len())
            .sum::<usize>();
        let counts = [
            (
                "native inputs",
                self.native_inputs.len(),
                limits.max_native_inputs,
            ),
            (
                "inventories",
                self.inventories.len(),
                limits.max_inventories,
            ),
            ("symbols", symbols, limits.max_symbols),
            ("ABI probes", self.abi_probes.len(), limits.max_abi_probes),
            ("layouts", self.layouts.len(), limits.max_layouts),
            (
                "declaration evidence",
                self.declaration_evidence.len(),
                limits.max_declaration_evidence,
            ),
            (
                "link atoms",
                self.resolved_link_plan.len(),
                limits.max_link_atoms,
            ),
            (
                "diagnostics",
                self.diagnostics.len(),
                limits.max_diagnostics,
            ),
        ];
        for (resource, actual, maximum) in counts {
            if actual > maximum {
                return Err(super::DecodeError::ResourceLimit {
                    resource,
                    actual,
                    maximum,
                });
            }
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct FingerprintPayload {
    schema: SchemaHeader,
    source_fingerprint: SourceFingerprint,
    target_fingerprint: TargetFingerprint,
    analysis_policy: AnalysisPolicyWire,
    native_inputs: Vec<NativeInputWire>,
    inventories: Vec<SymbolInventoryWire>,
    abi_probes: Vec<AbiProbeEvidenceWire>,
    layouts: Vec<LayoutEvidenceWire>,
    declaration_evidence: Vec<DeclarationEvidenceWire>,
    resolved_link_plan: ResolvedLinkPlanWire,
    diagnostics: Vec<LincDiagnosticWire>,
}

pub(crate) fn canonical_payload_bytes(
    package: &LinkAnalysisPackage,
) -> Result<Vec<u8>, ContractError> {
    serde_json::to_vec(&FingerprintPayload {
        schema: package.schema().clone(),
        source_fingerprint: package.source_fingerprint(),
        target_fingerprint: package.target_fingerprint(),
        analysis_policy: AnalysisPolicyWire::from_domain(package.analysis_policy()),
        native_inputs: package
            .native_inputs()
            .iter()
            .map(NativeInputWire::from_domain)
            .collect(),
        inventories: package
            .inventories()
            .iter()
            .map(SymbolInventoryWire::from_domain)
            .collect(),
        abi_probes: package
            .abi_probes()
            .iter()
            .map(AbiProbeEvidenceWire::from_domain)
            .collect(),
        layouts: package
            .layouts()
            .iter()
            .map(LayoutEvidenceWire::from_domain)
            .collect(),
        declaration_evidence: package
            .declaration_evidence()
            .iter()
            .map(DeclarationEvidenceWire::from_domain)
            .collect(),
        resolved_link_plan: ResolvedLinkPlanWire::from_domain(package.resolved_link_plan()),
        diagnostics: package
            .diagnostics()
            .iter()
            .map(LincDiagnosticWire::from_domain)
            .collect(),
    })
    .map_err(|error| ContractError::Canonical {
        message: error.to_string(),
    })
}

pub(crate) fn analysis_fingerprint(
    package: &LinkAnalysisPackage,
) -> Result<LinkAnalysisFingerprint, ContractError> {
    canonical_payload_bytes(package).map(|bytes| LinkAnalysisFingerprint::derive(&bytes))
}

fn require_strict_order<T, K: Ord>(
    values: &[T],
    key: impl Fn(&T) -> K,
    collection: &'static str,
) -> Result<(), ContractError> {
    if values.windows(2).all(|pair| key(&pair[0]) < key(&pair[1])) {
        Ok(())
    } else {
        Err(ContractError::NonCanonicalOrder { collection })
    }
}
