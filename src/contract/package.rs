use std::collections::{BTreeMap, BTreeSet};

use parc::contract::{
    Architecture, CallingConvention, ClosureRequirement, CompleteSourcePackage, DeclarationId,
    EnumValue, Linkage, ObjectFormat, OperatingSystem, RecordCompleteness, SchemaHeader,
    SourceDeclarationKind, SourceFingerprint, TargetFingerprint,
};

use super::{
    schema::link_analysis_schema_v2, validate_native_inputs, AbiProbeEvidence, AnalysisPolicy,
    ArtifactKind, CallableAbiAssessment, ContractError, DeclarationEvidence, DiagnosticEvidenceRef,
    EnumVariantEvidence, EvidenceAcceptancePolicy, EvidenceSource, InspectionParserKind,
    LayoutAssessment, LayoutEvidence, LincDiagnostic, LinkAnalysisFingerprint, NativeAbi,
    NativeInput, ObservedTarget, ProbeEvidenceId, ProbeMethod, ProbePolicy, ProbeSubject,
    ProviderAssessment, ProviderId, ProviderResolution, ResolutionPolicy, ResolvedLinkPlan,
    RunnerPolicy, SymbolAssessment, SymbolBinding, SymbolDecoration, SymbolInventory, SymbolKind,
    WeakSymbolPolicy,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkAnalysisPackageInput {
    pub source_fingerprint: SourceFingerprint,
    pub target_fingerprint: TargetFingerprint,
    pub analysis_policy: AnalysisPolicy,
    /// Effective unresolved native-input sequence. Order and repetition are
    /// semantic and are therefore never canonicalized.
    pub native_inputs: Vec<NativeInput>,
    pub inventories: Vec<SymbolInventory>,
    pub abi_probes: Vec<AbiProbeEvidence>,
    pub layouts: Vec<LayoutEvidence>,
    pub declaration_evidence: Vec<DeclarationEvidence>,
    pub resolved_link_plan: ResolvedLinkPlan,
    pub diagnostics: Vec<LincDiagnostic>,
}

/// Immutable, schema-v2 native evidence package.
///
/// Serialized values must enter through [`super::decode_link_analysis`]; this
/// type deliberately does not implement `Deserialize`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkAnalysisPackage {
    schema: SchemaHeader,
    fingerprint: LinkAnalysisFingerprint,
    source_fingerprint: SourceFingerprint,
    target_fingerprint: TargetFingerprint,
    analysis_policy: AnalysisPolicy,
    native_inputs: Vec<NativeInput>,
    inventories: Vec<SymbolInventory>,
    abi_probes: Vec<AbiProbeEvidence>,
    layouts: Vec<LayoutEvidence>,
    declaration_evidence: Vec<DeclarationEvidence>,
    resolved_link_plan: ResolvedLinkPlan,
    diagnostics: Vec<LincDiagnostic>,
}

pub(crate) struct LinkAnalysisPackageParts {
    pub schema: SchemaHeader,
    pub fingerprint: LinkAnalysisFingerprint,
    pub source_fingerprint: SourceFingerprint,
    pub target_fingerprint: TargetFingerprint,
    pub analysis_policy: AnalysisPolicy,
    pub native_inputs: Vec<NativeInput>,
    pub inventories: Vec<SymbolInventory>,
    pub abi_probes: Vec<AbiProbeEvidence>,
    pub layouts: Vec<LayoutEvidence>,
    pub declaration_evidence: Vec<DeclarationEvidence>,
    pub resolved_link_plan: ResolvedLinkPlan,
    pub diagnostics: Vec<LincDiagnostic>,
}

impl LinkAnalysisPackage {
    pub fn try_new(mut input: LinkAnalysisPackageInput) -> Result<Self, ContractError> {
        canonicalize_collections(&mut input)?;
        let mut package = Self {
            schema: link_analysis_schema_v2(),
            fingerprint: LinkAnalysisFingerprint::derive(b"unfingerprinted-link-analysis"),
            source_fingerprint: input.source_fingerprint,
            target_fingerprint: input.target_fingerprint,
            analysis_policy: input.analysis_policy,
            native_inputs: input.native_inputs,
            inventories: input.inventories,
            abi_probes: input.abi_probes,
            layouts: input.layouts,
            declaration_evidence: input.declaration_evidence,
            resolved_link_plan: input.resolved_link_plan,
            diagnostics: input.diagnostics,
        };
        validate_internal(&package)?;
        package.fingerprint = super::wire::analysis_fingerprint(&package)?;
        Ok(package)
    }

    pub(crate) fn from_parts(parts: LinkAnalysisPackageParts) -> Result<Self, ContractError> {
        let package = Self {
            schema: parts.schema,
            fingerprint: parts.fingerprint,
            source_fingerprint: parts.source_fingerprint,
            target_fingerprint: parts.target_fingerprint,
            analysis_policy: parts.analysis_policy,
            native_inputs: parts.native_inputs,
            inventories: parts.inventories,
            abi_probes: parts.abi_probes,
            layouts: parts.layouts,
            declaration_evidence: parts.declaration_evidence,
            resolved_link_plan: parts.resolved_link_plan,
            diagnostics: parts.diagnostics,
        };
        validate_canonical_order(&package)?;
        validate_internal(&package)?;
        Ok(package)
    }

    pub fn schema(&self) -> &SchemaHeader {
        &self.schema
    }

    pub const fn fingerprint(&self) -> LinkAnalysisFingerprint {
        self.fingerprint
    }

    pub const fn source_fingerprint(&self) -> SourceFingerprint {
        self.source_fingerprint
    }

    pub const fn target_fingerprint(&self) -> TargetFingerprint {
        self.target_fingerprint
    }

    pub const fn analysis_policy(&self) -> &AnalysisPolicy {
        &self.analysis_policy
    }

    pub fn native_inputs(&self) -> &[NativeInput] {
        &self.native_inputs
    }

    pub fn inventories(&self) -> &[SymbolInventory] {
        &self.inventories
    }

    pub fn abi_probes(&self) -> &[AbiProbeEvidence] {
        &self.abi_probes
    }

    pub fn layouts(&self) -> &[LayoutEvidence] {
        &self.layouts
    }

    pub fn declaration_evidence(&self) -> &[DeclarationEvidence] {
        &self.declaration_evidence
    }

    pub fn resolved_link_plan(&self) -> &ResolvedLinkPlan {
        &self.resolved_link_plan
    }

    pub fn diagnostics(&self) -> &[LincDiagnostic] {
        &self.diagnostics
    }
}

/// Proof that a LINC package exactly covers a complete PARC source closure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedLinkAnalysis(LinkAnalysisPackage);

impl ValidatedLinkAnalysis {
    pub fn try_new(
        source: &CompleteSourcePackage,
        package: LinkAnalysisPackage,
    ) -> Result<Self, ContractError> {
        validate_against_source(source, &package)?;
        Ok(Self(package))
    }

    pub fn package(&self) -> &LinkAnalysisPackage {
        &self.0
    }

    pub fn into_package(self) -> LinkAnalysisPackage {
        self.0
    }
}

pub(crate) fn validate_internal(package: &LinkAnalysisPackage) -> Result<(), ContractError> {
    package.analysis_policy.validate()?;
    validate_native_inputs(&package.native_inputs)?;
    if package.analysis_policy.resolution() == ResolutionPolicy::ExactPathsOnly
        && package.native_inputs.iter().any(|input| {
            matches!(
                input,
                NativeInput::SearchNative(_)
                    | NativeInput::StaticLibraryName(_)
                    | NativeInput::DynamicLibraryName(_)
                    | NativeInput::ImportLibraryName(_)
                    | NativeInput::FrameworkName { .. }
            )
        })
    {
        return Err(ContractError::InvalidPolicy {
            reason: "exact-path resolution cannot snapshot search paths or name requests",
        });
    }
    let providers = inventory_map(package)?;
    let probes = probe_map(package)?;

    for inventory in &package.inventories {
        let artifact = inventory.artifact();
        validate_inspection_provenance(inventory)?;
        match artifact.resolution() {
            ProviderResolution::Explicit => {}
            ProviderResolution::SearchPath { native_input_index } => {
                validate_resolution_input(
                    artifact.provider_id(),
                    artifact.kind(),
                    *native_input_index,
                    &package.native_inputs,
                )?;
            }
            ProviderResolution::Dependency { parent } => {
                validate_dependency_provider(artifact.provider_id(), *parent, &providers)?;
                let parent_inventory = providers
                    .get(parent)
                    .expect("dependency parent existence was checked");
                if !parent_inventory
                    .dependency_edges()
                    .iter()
                    .any(|edge| edge.provider() == Some(artifact.provider_id()))
                {
                    return Err(ContractError::DependencyCrossReference {
                        parent: *parent,
                        child: artifact.provider_id(),
                    });
                }
            }
        }
        for edge in inventory.dependency_edges() {
            if let Some(child) = edge.provider() {
                validate_dependency_provider(artifact.provider_id(), child, &providers)?;
                // An artifact can satisfy multiple DT_NEEDED edges (a diamond)
                // or also be named directly. Its resolution records one
                // primary discovery route, while every parent relationship is
                // retained by these ordered edges. The primary dependency
                // route is checked in the artifact-resolution branch above.
            }
        }
    }
    validate_dependency_graph(&providers)?;

    let mut layout_declarations = BTreeSet::new();
    for layout in &package.layouts {
        let declaration = layout.declaration();
        if !layout_declarations.insert(declaration) {
            return Err(ContractError::DuplicateDeclarationEvidence {
                declaration,
                evidence_kind: "layout",
            });
        }
        validate_evidence_fingerprints(
            "layout",
            layout.source_fingerprint(),
            layout.target_fingerprint(),
            package,
        )?;
        if !probes.contains_key(&layout.probe()) {
            return Err(ContractError::MissingProbeEvidence {
                probe: layout.probe(),
            });
        }
    }
    let layouts: BTreeMap<_, _> = package
        .layouts
        .iter()
        .map(|layout| (layout.declaration(), layout))
        .collect();

    let mut declaration_ids = BTreeSet::new();
    for evidence in &package.declaration_evidence {
        let declaration = evidence.declaration();
        if !declaration_ids.insert(declaration) {
            return Err(ContractError::DuplicateDeclarationEvidence {
                declaration,
                evidence_kind: "declaration",
            });
        }
        validate_evidence_fingerprints(
            "declaration",
            evidence.source_fingerprint(),
            evidence.target_fingerprint(),
            package,
        )?;
        validate_declaration_references(evidence, &providers)?;
        validate_declaration_probe_references(evidence, &layouts, &probes)?;
    }

    for atom in package.resolved_link_plan.atoms() {
        let Some(artifact) = atom.artifact() else {
            continue;
        };
        let provider = artifact.provider_id();
        let inventory = providers
            .get(&provider)
            .ok_or(ContractError::MissingProvider { provider })?;
        if inventory.artifact() != artifact {
            return Err(ContractError::PlanArtifactMismatch { provider });
        }
    }

    for diagnostic in &package.diagnostics {
        if diagnostic.context().target_fingerprint() != package.target_fingerprint {
            return Err(ContractError::EvidenceTargetMismatch {
                evidence_kind: "diagnostic",
                expected: package.target_fingerprint,
                actual: diagnostic.context().target_fingerprint(),
            });
        }
        if diagnostic
            .context()
            .native_input_index()
            .is_some_and(|index| {
                usize::try_from(index)
                    .ok()
                    .is_none_or(|index| index >= package.native_inputs.len())
            })
        {
            return Err(ContractError::InvalidDiagnosticContext {
                reason: "native-input index is outside the package snapshot",
            });
        }
        if let Some(provider) = diagnostic.provider() {
            if !providers.contains_key(&provider) {
                return Err(ContractError::MissingProvider { provider });
            }
        }
        if let Some(provider) = diagnostic.context().dependency_provider() {
            if !providers.contains_key(&provider) {
                return Err(ContractError::MissingProvider { provider });
            }
        }
        if let Some(probe) = diagnostic.context().probe() {
            if !probes.contains_key(&probe) {
                return Err(ContractError::MissingProbeEvidence { probe });
            }
        }
        validate_diagnostic_evidence_reference(diagnostic, &providers, &layouts)?;
    }
    Ok(())
}

fn inventory_map(
    package: &LinkAnalysisPackage,
) -> Result<BTreeMap<ProviderId, &SymbolInventory>, ContractError> {
    let mut providers = BTreeMap::new();
    for inventory in &package.inventories {
        let artifact = inventory.artifact();
        let provider = artifact.provider_id();
        if providers.insert(provider, inventory).is_some() {
            return Err(ContractError::DuplicateProvider { provider });
        }
        let actual = artifact.observed_target().target_fingerprint();
        if actual != package.target_fingerprint {
            return Err(ContractError::EvidenceTargetMismatch {
                evidence_kind: "symbol inventory",
                expected: package.target_fingerprint,
                actual,
            });
        }
    }
    Ok(providers)
}

fn probe_map(
    package: &LinkAnalysisPackage,
) -> Result<BTreeMap<ProbeEvidenceId, &AbiProbeEvidence>, ContractError> {
    let mut probes = BTreeMap::new();
    for probe in &package.abi_probes {
        if probes.insert(probe.id(), probe).is_some() {
            return Err(ContractError::DuplicateProbeEvidence { probe: probe.id() });
        }
        validate_evidence_fingerprints(
            "ABI probe",
            probe.source_fingerprint(),
            probe.target_fingerprint(),
            package,
        )?;
    }
    Ok(probes)
}

fn validate_resolution_input(
    provider: ProviderId,
    kind: ArtifactKind,
    index: u32,
    native_inputs: &[NativeInput],
) -> Result<(), ContractError> {
    let Some(input) = usize::try_from(index)
        .ok()
        .and_then(|index| native_inputs.get(index))
    else {
        return Err(ContractError::InvalidResolutionInput { provider, index });
    };
    let matches_kind = matches!(
        (kind, input),
        (
            ArtifactKind::StaticLibrary,
            NativeInput::StaticLibraryName(_)
        ) | (
            ArtifactKind::DynamicLibrary,
            NativeInput::DynamicLibraryName(_)
        ) | (
            ArtifactKind::ImportLibrary,
            NativeInput::ImportLibraryName(_)
        ) | (ArtifactKind::Framework, NativeInput::FrameworkName { .. })
    );
    if matches_kind {
        Ok(())
    } else {
        Err(ContractError::InvalidResolutionInput { provider, index })
    }
}

fn validate_dependency_provider(
    provider: ProviderId,
    parent: ProviderId,
    providers: &BTreeMap<ProviderId, &SymbolInventory>,
) -> Result<(), ContractError> {
    if parent == provider {
        return Err(ContractError::SelfParentProvider { provider });
    }
    if !providers.contains_key(&parent) {
        return Err(ContractError::MissingParentProvider { provider, parent });
    }
    Ok(())
}

fn validate_inspection_provenance(inventory: &SymbolInventory) -> Result<(), ContractError> {
    let artifact = inventory.artifact();
    let parsers: BTreeSet<_> = inventory
        .inspection()
        .parsers()
        .iter()
        .map(|parser| parser.kind())
        .collect();
    let format_parser = match (artifact.observed_target().object_format(), artifact.kind()) {
        (ObjectFormat::Elf, _) => InspectionParserKind::Elf,
        (ObjectFormat::MachO, _) => InspectionParserKind::MachO,
        (ObjectFormat::Coff, ArtifactKind::DynamicLibrary) => InspectionParserKind::Pe,
        (ObjectFormat::Coff, _) => InspectionParserKind::Coff,
        (ObjectFormat::Wasm, _) => InspectionParserKind::Wasm,
        (ObjectFormat::Xcoff, _) => InspectionParserKind::Xcoff,
    };
    let archive_required = matches!(
        artifact.kind(),
        ArtifactKind::StaticLibrary | ArtifactKind::ImportLibrary
    );
    let framework_mismatch = artifact.kind() == ArtifactKind::Framework
        && artifact.observed_target().object_format() != ObjectFormat::MachO;
    if !parsers.contains(&format_parser)
        || (archive_required && !parsers.contains(&InspectionParserKind::Archive))
        || framework_mismatch
    {
        return Err(ContractError::InspectionParserMismatch {
            provider: artifact.provider_id(),
        });
    }
    Ok(())
}

fn validate_dependency_graph(
    providers: &BTreeMap<ProviderId, &SymbolInventory>,
) -> Result<(), ContractError> {
    let mut state = BTreeMap::<ProviderId, u8>::new();
    for provider in providers.keys().copied() {
        visit_dependency_provider(provider, providers, &mut state)?;
    }
    Ok(())
}

fn visit_dependency_provider(
    provider: ProviderId,
    providers: &BTreeMap<ProviderId, &SymbolInventory>,
    state: &mut BTreeMap<ProviderId, u8>,
) -> Result<(), ContractError> {
    match state.get(&provider).copied() {
        Some(2) => return Ok(()),
        Some(1) => return Err(ContractError::DependencyCycle { provider }),
        _ => {}
    }
    state.insert(provider, 1);
    let inventory = providers
        .get(&provider)
        .expect("dependency traversal starts from a checked provider");
    for child in inventory
        .dependency_edges()
        .iter()
        .filter_map(super::DependencyEdge::provider)
    {
        visit_dependency_provider(child, providers, state)?;
    }
    state.insert(provider, 2);
    Ok(())
}

fn validate_evidence_fingerprints(
    evidence_kind: &'static str,
    source: SourceFingerprint,
    target: TargetFingerprint,
    package: &LinkAnalysisPackage,
) -> Result<(), ContractError> {
    if source != package.source_fingerprint {
        return Err(ContractError::EvidenceSourceFingerprintMismatch {
            evidence_kind,
            expected: package.source_fingerprint,
            actual: source,
        });
    }
    if target != package.target_fingerprint {
        return Err(ContractError::EvidenceTargetMismatch {
            evidence_kind,
            expected: package.target_fingerprint,
            actual: target,
        });
    }
    Ok(())
}

fn validate_declaration_references(
    evidence: &DeclarationEvidence,
    providers: &BTreeMap<ProviderId, &SymbolInventory>,
) -> Result<(), ContractError> {
    match evidence.provider() {
        ProviderAssessment::Resolved {
            provider,
            artifact_fingerprint,
        } => {
            let inventory = providers
                .get(provider)
                .ok_or(ContractError::MissingProvider {
                    provider: *provider,
                })?;
            let actual = inventory.artifact().artifact_fingerprint();
            if actual != *artifact_fingerprint {
                return Err(ContractError::ArtifactFingerprintMismatch {
                    provider: *provider,
                    expected: *artifact_fingerprint,
                    actual,
                });
            }
        }
        ProviderAssessment::Ambiguous {
            providers: candidates,
        } => {
            for provider in candidates {
                if !providers.contains_key(provider) {
                    return Err(ContractError::MissingProvider {
                        provider: *provider,
                    });
                }
            }
        }
        ProviderAssessment::NotRequired
        | ProviderAssessment::Unresolved
        | ProviderAssessment::Rejected { .. } => {}
    }

    if let SymbolAssessment::Exact {
        symbol,
        actual_name,
        kind,
        decoration,
        ..
    } = evidence.symbol()
    {
        let inventory =
            providers
                .get(&symbol.provider())
                .ok_or(ContractError::MissingProvider {
                    provider: symbol.provider(),
                })?;
        let record =
            inventory
                .symbol(symbol.symbol())
                .ok_or(ContractError::MissingSymbolIdentity {
                    declaration: evidence.declaration(),
                    provider: symbol.provider(),
                    symbol: symbol.symbol(),
                })?;
        if record.name() != actual_name
            || record.kind() != *kind
            || record.decoration() != decoration
        {
            return Err(ContractError::SymbolEvidenceMismatch {
                declaration: evidence.declaration(),
            });
        }
        match evidence.provider() {
            ProviderAssessment::Resolved { provider, .. } if *provider == symbol.provider() => {}
            _ => {
                return Err(ContractError::SymbolEvidenceMismatch {
                    declaration: evidence.declaration(),
                });
            }
        }
    }

    match evidence.symbol() {
        SymbolAssessment::Ambiguous { candidates } => {
            for candidate in candidates {
                validate_symbol_reference(evidence.declaration(), *candidate, providers)?;
            }
        }
        SymbolAssessment::WrongKind { symbol, .. } => {
            validate_symbol_reference(evidence.declaration(), *symbol, providers)?;
        }
        SymbolAssessment::NotRequired
        | SymbolAssessment::Exact { .. }
        | SymbolAssessment::Missing { .. }
        | SymbolAssessment::Rejected { .. } => {}
    }
    Ok(())
}

fn validate_symbol_reference(
    declaration: DeclarationId,
    symbol: super::SymbolReference,
    providers: &BTreeMap<ProviderId, &SymbolInventory>,
) -> Result<(), ContractError> {
    let inventory = providers
        .get(&symbol.provider())
        .ok_or(ContractError::MissingProvider {
            provider: symbol.provider(),
        })?;
    if inventory.symbol(symbol.symbol()).is_none() {
        return Err(ContractError::MissingSymbolIdentity {
            declaration,
            provider: symbol.provider(),
            symbol: symbol.symbol(),
        });
    }
    Ok(())
}

fn validate_declaration_probe_references(
    evidence: &DeclarationEvidence,
    layouts: &BTreeMap<DeclarationId, &LayoutEvidence>,
    probes: &BTreeMap<ProbeEvidenceId, &AbiProbeEvidence>,
) -> Result<(), ContractError> {
    if let LayoutAssessment::Available { confidence, probe } = evidence.layout() {
        let layout =
            layouts
                .get(&evidence.declaration())
                .ok_or(ContractError::RequiredLayoutEvidence {
                    declaration: evidence.declaration(),
                })?;
        if layout.probe() != *probe || layout.confidence() != *confidence {
            return Err(ContractError::IncoherentDeclarationEvidence {
                declaration: evidence.declaration(),
                reason: "layout assessment differs from the referenced layout record",
            });
        }
        if !probes.contains_key(probe) {
            return Err(ContractError::MissingProbeEvidence { probe: *probe });
        }
    }
    if let CallableAbiAssessment::Confirmed { probe, .. } = evidence.callable_abi() {
        if !probes.contains_key(probe) {
            return Err(ContractError::MissingProbeEvidence { probe: *probe });
        }
    }
    Ok(())
}

fn validate_diagnostic_evidence_reference(
    diagnostic: &LincDiagnostic,
    providers: &BTreeMap<ProviderId, &SymbolInventory>,
    layouts: &BTreeMap<DeclarationId, &LayoutEvidence>,
) -> Result<(), ContractError> {
    match diagnostic.context().evidence() {
        Some(DiagnosticEvidenceRef::Symbol { provider, symbol }) => {
            let inventory = providers
                .get(provider)
                .ok_or(ContractError::MissingProvider {
                    provider: *provider,
                })?;
            if inventory.symbol(*symbol).is_none() {
                return Err(ContractError::InvalidDiagnosticContext {
                    reason: "symbol evidence identity is absent from its provider inventory",
                });
            }
        }
        Some(DiagnosticEvidenceRef::Layout { declaration }) => {
            if !layouts.contains_key(declaration) {
                return Err(ContractError::InvalidDiagnosticContext {
                    reason: "layout evidence identity is absent from the package",
                });
            }
        }
        Some(DiagnosticEvidenceRef::Declaration { declaration }) => {
            if diagnostic.declaration() != Some(*declaration) {
                return Err(ContractError::InvalidDiagnosticContext {
                    reason: "declaration evidence identity differs from diagnostic declaration",
                });
            }
        }
        None => {}
    }
    Ok(())
}

fn validate_against_source(
    source: &CompleteSourcePackage,
    package: &LinkAnalysisPackage,
) -> Result<(), ContractError> {
    let expected_source = source.source().fingerprint();
    if package.source_fingerprint != expected_source {
        return Err(ContractError::SourceFingerprintMismatch {
            expected: expected_source,
            actual: package.source_fingerprint,
        });
    }
    let expected_target = source.source().target_fingerprint();
    if package.target_fingerprint != expected_target {
        return Err(ContractError::TargetFingerprintMismatch {
            expected: expected_target,
            actual: package.target_fingerprint,
        });
    }
    if package.analysis_policy.weak_symbols() != WeakSymbolPolicy::Reject
        || package.analysis_policy.layout_evidence() != EvidenceAcceptancePolicy::MeasuredOnly
        || package.analysis_policy.callable_abi_evidence() != EvidenceAcceptancePolicy::MeasuredOnly
    {
        return Err(ContractError::InvalidPolicy {
            reason: "validated analysis requires reject-weak and measured-only evidence policy",
        });
    }
    let probes = probe_map(package)?;
    if !probes.is_empty() && package.analysis_policy.probe() == ProbePolicy::Disabled {
        return Err(ContractError::InvalidPolicy {
            reason: "disabled probe policy cannot carry ABI probe evidence",
        });
    }
    for probe in probes.values() {
        if probe.compiler() != source.source().target().compiler()
            || probe.abi_flags() != source.source().target().abi_flags()
        {
            return Err(ContractError::ProbeCompilerMismatch { probe: probe.id() });
        }
        if probe.execution_policy() != package.analysis_policy.probe_execution() {
            return Err(ContractError::InvalidPolicy {
                reason: "ABI probe execution policy differs from package analysis policy",
            });
        }
        validate_probe_runner_policy(probe, package.analysis_policy())?;
    }
    for inventory in &package.inventories {
        let artifact = inventory.artifact();
        if !artifact
            .observed_target()
            .matches_target(source.source().target())
        {
            return Err(ContractError::ObservedTargetMismatch {
                provider: artifact.provider_id(),
            });
        }
    }
    for layout in &package.layouts {
        if !layout.confidence().is_strictly_measured()
            || !matches!(
                layout.source(),
                EvidenceSource::CompilerProbe | EvidenceSource::Corroborated
            )
        {
            return Err(ContractError::InferredLayoutEvidence {
                declaration: layout.declaration(),
            });
        }
    }

    let closure: BTreeMap<_, _> = source
        .declaration_closure()
        .iter()
        .map(|entry| (entry.declaration(), entry.requirement()))
        .collect();
    for evidence in &package.declaration_evidence {
        if !closure.contains_key(&evidence.declaration()) {
            return Err(ContractError::DeclarationOutsideClosure {
                declaration: evidence.declaration(),
            });
        }
    }
    for layout in &package.layouts {
        if !closure.contains_key(&layout.declaration()) {
            return Err(ContractError::DeclarationOutsideClosure {
                declaration: layout.declaration(),
            });
        }
        validate_layout_owner(source, layout)?;
    }

    let evidence_by_declaration: BTreeMap<_, _> = package
        .declaration_evidence
        .iter()
        .map(|evidence| (evidence.declaration(), evidence))
        .collect();
    let layouts_by_declaration: BTreeMap<_, _> = package
        .layouts
        .iter()
        .map(|layout| (layout.declaration(), layout))
        .collect();
    let providers = inventory_map(package)?;
    let plan_providers: BTreeSet<_> = package
        .resolved_link_plan
        .atoms()
        .iter()
        .filter_map(|atom| atom.artifact().map(|artifact| artifact.provider_id()))
        .collect();
    let plan_provider_positions = package
        .resolved_link_plan
        .atoms()
        .iter()
        .enumerate()
        .filter_map(|(index, atom)| {
            atom.artifact()
                .map(|artifact| (artifact.provider_id(), index))
        })
        .fold(BTreeMap::new(), |mut positions, (provider, index)| {
            positions.entry(provider).or_insert(index);
            positions
        });
    validate_strict_dependencies(package, &plan_provider_positions)?;
    validate_diagnostics_against_source(source, package, &closure, &evidence_by_declaration)?;

    validate_probe_subjects_against_source(source, &closure, &probes)?;

    for (declaration_id, requirement) in closure {
        let declaration = source.source().declaration(declaration_id).ok_or(
            ContractError::DeclarationOutsideClosure {
                declaration: declaration_id,
            },
        )?;
        let evidence = evidence_by_declaration
            .get(&declaration_id)
            .copied()
            .ok_or(ContractError::RequiredDeclarationEvidence {
                declaration: declaration_id,
            })?;
        match &declaration.kind {
            SourceDeclarationKind::Function(function)
                if declaration.linkage == Linkage::External =>
            {
                validate_no_layout_dimension(declaration_id, evidence, &layouts_by_declaration)?;
                validate_linked_declaration(
                    declaration_id,
                    &function.link_name,
                    SymbolKind::Function,
                    Some(&function.calling_convention),
                    evidence,
                    &providers,
                    &plan_providers,
                )?;
                validate_callable_abi(
                    declaration_id,
                    &function.calling_convention,
                    evidence.callable_abi(),
                    &probes,
                )?;
            }
            SourceDeclarationKind::Variable(variable)
                if declaration.linkage == Linkage::External =>
            {
                validate_no_layout_dimension(declaration_id, evidence, &layouts_by_declaration)?;
                require_callable_not_applicable(declaration_id, evidence)?;
                let kind = if variable.thread_local {
                    SymbolKind::ThreadLocal
                } else {
                    SymbolKind::Data
                };
                validate_linked_declaration(
                    declaration_id,
                    &variable.link_name,
                    kind,
                    None,
                    evidence,
                    &providers,
                    &plan_providers,
                )?;
            }
            SourceDeclarationKind::Record(record)
                if requirement == ClosureRequirement::Definition
                    && record.completeness == RecordCompleteness::Complete =>
            {
                require_provider_and_symbol_not_required(declaration_id, evidence)?;
                require_callable_not_applicable(declaration_id, evidence)?;
                validate_required_layout(
                    declaration_id,
                    evidence,
                    &layouts_by_declaration,
                    &probes,
                    ProbeSubject::RecordLayout {
                        declaration: declaration_id,
                    },
                )?;
            }
            SourceDeclarationKind::Enum(_) if requirement == ClosureRequirement::Definition => {
                require_provider_and_symbol_not_required(declaration_id, evidence)?;
                require_callable_not_applicable(declaration_id, evidence)?;
                validate_required_layout(
                    declaration_id,
                    evidence,
                    &layouts_by_declaration,
                    &probes,
                    ProbeSubject::EnumRepresentation {
                        declaration: declaration_id,
                    },
                )?;
            }
            SourceDeclarationKind::Function(_)
            | SourceDeclarationKind::Record(_)
            | SourceDeclarationKind::Enum(_)
            | SourceDeclarationKind::TypeAlias(_)
            | SourceDeclarationKind::Variable(_)
            | SourceDeclarationKind::Unsupported(_) => {
                require_provider_and_symbol_not_required(declaration_id, evidence)?;
                validate_no_layout_dimension(declaration_id, evidence, &layouts_by_declaration)?;
                require_callable_not_applicable(declaration_id, evidence)?;
            }
        }
    }
    Ok(())
}

fn validate_diagnostics_against_source(
    source: &CompleteSourcePackage,
    package: &LinkAnalysisPackage,
    closure: &BTreeMap<DeclarationId, ClosureRequirement>,
    evidence: &BTreeMap<DeclarationId, &DeclarationEvidence>,
) -> Result<(), ContractError> {
    for diagnostic in &package.diagnostics {
        if let Some(declaration) = diagnostic.declaration() {
            if !closure.contains_key(&declaration) {
                return Err(ContractError::DeclarationOutsideClosure { declaration });
            }
        }
        if let Some(range) = diagnostic.context().source_range() {
            let file = source
                .source()
                .files()
                .iter()
                .find(|file| file.id == range.file)
                .ok_or(ContractError::InvalidDiagnosticContext {
                    reason: "source range references a file outside the PARC package",
                })?;
            if range.start >= range.end || range.end > file.byte_len {
                return Err(ContractError::InvalidDiagnosticContext {
                    reason: "source range is empty or outside the PARC file",
                });
            }
        }
        match diagnostic.context().evidence() {
            Some(DiagnosticEvidenceRef::Declaration { declaration }) => {
                if !evidence.contains_key(declaration) {
                    return Err(ContractError::InvalidDiagnosticContext {
                        reason: "declaration evidence identity is absent from the package",
                    });
                }
            }
            Some(DiagnosticEvidenceRef::Layout { declaration }) => {
                if diagnostic.declaration() != Some(*declaration) {
                    return Err(ContractError::InvalidDiagnosticContext {
                        reason: "layout evidence and diagnostic declarations differ",
                    });
                }
            }
            Some(DiagnosticEvidenceRef::Symbol { provider, .. }) => {
                if diagnostic.provider() != Some(*provider) {
                    return Err(ContractError::InvalidDiagnosticContext {
                        reason: "symbol evidence and diagnostic providers differ",
                    });
                }
            }
            None => {}
        }
    }
    Ok(())
}

fn validate_strict_dependencies(
    package: &LinkAnalysisPackage,
    plan_positions: &BTreeMap<ProviderId, usize>,
) -> Result<(), ContractError> {
    for inventory in &package.inventories {
        let parent = inventory.artifact().provider_id();
        for edge in inventory.dependency_edges() {
            let child = edge
                .provider()
                .ok_or(ContractError::UnresolvedDependency { parent })?;
            let parent_position = plan_positions
                .get(&parent)
                .ok_or(ContractError::ProviderNotInPlan { provider: parent })?;
            let child_position = plan_positions
                .get(&child)
                .ok_or(ContractError::ProviderNotInPlan { provider: child })?;
            if parent_position >= child_position {
                return Err(ContractError::DependencyPlanOrder { parent, child });
            }
        }
    }
    Ok(())
}

fn validate_linked_declaration(
    declaration: DeclarationId,
    expected_name: &str,
    expected_kind: SymbolKind,
    expected_calling_convention: Option<&CallingConvention>,
    evidence: &DeclarationEvidence,
    providers: &BTreeMap<ProviderId, &SymbolInventory>,
    plan_providers: &BTreeSet<ProviderId>,
) -> Result<(), ContractError> {
    let ProviderAssessment::Resolved {
        provider,
        artifact_fingerprint,
    } = evidence.provider()
    else {
        return Err(ContractError::RequiredSymbolEvidence { declaration });
    };
    if !plan_providers.contains(provider) {
        return Err(ContractError::ProviderNotInPlan {
            provider: *provider,
        });
    }
    let inventory = providers
        .get(provider)
        .ok_or(ContractError::MissingProvider {
            provider: *provider,
        })?;
    if inventory.artifact().artifact_fingerprint() != *artifact_fingerprint {
        return Err(ContractError::ArtifactFingerprintMismatch {
            provider: *provider,
            expected: *artifact_fingerprint,
            actual: inventory.artifact().artifact_fingerprint(),
        });
    }

    let SymbolAssessment::Exact {
        symbol,
        expected_name: stated_expected,
        actual_name,
        kind,
        decoration,
    } = evidence.symbol()
    else {
        return Err(ContractError::RequiredSymbolEvidence { declaration });
    };
    if stated_expected != expected_name || symbol.provider() != *provider {
        return Err(ContractError::SymbolNameMismatch {
            declaration,
            expected: expected_name.to_owned(),
            actual: actual_name.clone(),
        });
    }
    let confirmed_calling_convention = match evidence.callable_abi() {
        CallableAbiAssessment::Confirmed {
            calling_convention, ..
        } => Some(calling_convention),
        _ => None,
    };
    let canonical_actual = canonical_symbol_spelling(
        expected_name,
        decoration,
        inventory.artifact().observed_target(),
        expected_calling_convention,
        confirmed_calling_convention,
    )
    .map_err(|reason| ContractError::InvalidSymbolDecoration {
        declaration,
        reason,
    })?;
    if actual_name != &canonical_actual {
        return Err(ContractError::SymbolNameMismatch {
            declaration,
            expected: canonical_actual,
            actual: actual_name.clone(),
        });
    }
    let record = inventory
        .symbol(symbol.symbol())
        .ok_or(ContractError::MissingSymbolIdentity {
            declaration,
            provider: *provider,
            symbol: symbol.symbol(),
        })?;
    if !record.is_visible_export() || record.binding() != SymbolBinding::Global {
        return Err(ContractError::SymbolNotVisible { declaration });
    }
    if record.kind() != expected_kind || *kind != expected_kind {
        return Err(ContractError::SymbolKindMismatch {
            declaration,
            expected: expected_kind,
            actual: record.kind(),
        });
    }
    if record.name() != canonical_actual || record.decoration() != decoration {
        return Err(ContractError::SymbolEvidenceMismatch { declaration });
    }

    let visible_providers = plan_providers
        .iter()
        .filter_map(|candidate| providers.get(candidate))
        .map(|inventory| {
            inventory
                .symbols()
                .iter()
                .filter(|symbol| {
                    symbol.name() == canonical_actual
                        && symbol.kind() == expected_kind
                        && symbol.decoration() == decoration
                        && symbol.is_visible_export()
                })
                .count()
        })
        .sum::<usize>();
    if visible_providers != 1 {
        return Err(ContractError::AmbiguousVisibleProviders {
            declaration,
            count: visible_providers,
        });
    }
    Ok(())
}

pub(crate) fn canonical_symbol_spelling(
    expected_name: &str,
    decoration: &SymbolDecoration,
    target: &ObservedTarget,
    expected_calling_convention: Option<&CallingConvention>,
    confirmed_calling_convention: Option<&CallingConvention>,
) -> Result<String, &'static str> {
    match decoration {
        SymbolDecoration::None => Ok(expected_name.to_owned()),
        SymbolDecoration::LeadingUnderscore => {
            if expected_calling_convention != confirmed_calling_convention {
                return Err(
                    "leading-underscore evidence requires the confirmed callable convention",
                );
            }
            let convention_supported = expected_calling_convention.is_none_or(|convention| {
                matches!(
                    convention,
                    CallingConvention::C
                        | CallingConvention::Cdecl
                        | CallingConvention::SysV64
                        | CallingConvention::Aapcs
                )
            });
            let apple_macho = target.object_format() == ObjectFormat::MachO
                && matches!(
                    target.operating_system(),
                    OperatingSystem::Darwin
                        | OperatingSystem::MacOs
                        | OperatingSystem::Ios
                        | OperatingSystem::TvOs
                        | OperatingSystem::WatchOs
                )
                && matches!(
                    target.architecture(),
                    Architecture::X86
                        | Architecture::X86_64
                        | Architecture::Arm
                        | Architecture::Aarch64
                );
            let windows_x86_c = target.object_format() == ObjectFormat::Coff
                && target.operating_system() == OperatingSystem::Windows
                && target.architecture() == Architecture::X86
                && target.pointer_width() == 32
                && target.abi() == NativeAbi::Win32
                && expected_calling_convention.is_none_or(|convention| {
                    matches!(convention, CallingConvention::C | CallingConvention::Cdecl)
                });
            if convention_supported && (apple_macho || windows_x86_c) {
                Ok(format!("_{expected_name}"))
            } else {
                Err("leading-underscore spelling is not certified for this target and convention")
            }
        }
        SymbolDecoration::Stdcall { .. } => {
            let windows_x86_stdcall = target.object_format() == ObjectFormat::Coff
                && target.operating_system() == OperatingSystem::Windows
                && target.architecture() == Architecture::X86
                && target.pointer_width() == 32
                && target.abi() == NativeAbi::Win32
                && expected_calling_convention == Some(&CallingConvention::Stdcall)
                && confirmed_calling_convention == Some(&CallingConvention::Stdcall);
            if windows_x86_stdcall {
                Err("stdcall spelling requires independently proven ABI-rounded stack bytes")
            } else {
                Err("stdcall spelling requires Windows x86 and confirmed stdcall ABI")
            }
        }
        SymbolDecoration::Versioned { version, .. } => {
            if target.object_format() != ObjectFormat::Elf {
                return Err("symbol versions are certified only for ELF providers");
            }
            if version.is_empty() || version.contains(&0) {
                return Err("ELF symbol version is empty or contains NUL");
            }
            Ok(expected_name.to_owned())
        }
        SymbolDecoration::Other { .. } => {
            Err("unmodeled symbol decoration is not accepted by the strict H1 path")
        }
    }
}

fn validate_callable_abi(
    declaration: DeclarationId,
    expected: &CallingConvention,
    assessment: &CallableAbiAssessment,
    probes: &BTreeMap<ProbeEvidenceId, &AbiProbeEvidence>,
) -> Result<(), ContractError> {
    let CallableAbiAssessment::Confirmed {
        calling_convention,
        confidence,
        probe,
    } = assessment
    else {
        return Err(ContractError::RequiredCallableAbiEvidence { declaration });
    };
    if calling_convention != expected || !confidence.is_strictly_measured() {
        return Err(ContractError::RequiredCallableAbiEvidence { declaration });
    }
    let evidence = probes
        .get(probe)
        .ok_or(ContractError::MissingProbeEvidence { probe: *probe })?;
    if !evidence.supports(ProbeSubject::CallableAbi { declaration })
        || !evidence.verified(ProbeSubject::CallableAbi { declaration })
    {
        return Err(ContractError::ProbeSubjectMismatch {
            probe: *probe,
            declaration,
            subject: "callable ABI",
        });
    }
    if evidence.method() == ProbeMethod::CompilerLayoutDump {
        return Err(ContractError::ProbeMethodMismatch {
            probe: *probe,
            subject: "callable ABI",
        });
    }
    Ok(())
}

fn validate_required_layout(
    declaration: DeclarationId,
    evidence: &DeclarationEvidence,
    layouts: &BTreeMap<DeclarationId, &LayoutEvidence>,
    probes: &BTreeMap<ProbeEvidenceId, &AbiProbeEvidence>,
    subject: ProbeSubject,
) -> Result<(), ContractError> {
    let subject_label = match subject {
        ProbeSubject::RecordLayout { .. } => "record layout",
        ProbeSubject::EnumRepresentation { .. } => "enum representation",
        ProbeSubject::CallableAbi { .. } => "callable ABI",
    };
    let LayoutAssessment::Available { confidence, probe } = evidence.layout() else {
        return Err(ContractError::RequiredLayoutEvidence { declaration });
    };
    let layout = layouts
        .get(&declaration)
        .ok_or(ContractError::RequiredLayoutEvidence { declaration })?;
    if !confidence.is_strictly_measured()
        || !layout.confidence().is_strictly_measured()
        || *confidence != layout.confidence()
        || *probe != layout.probe()
    {
        return Err(ContractError::InferredLayoutEvidence { declaration });
    }
    let evidence = probes
        .get(probe)
        .ok_or(ContractError::MissingProbeEvidence { probe: *probe })?;
    if !evidence.supports(subject) || !evidence.verified(subject) {
        return Err(ContractError::ProbeSubjectMismatch {
            probe: *probe,
            declaration,
            subject: subject_label,
        });
    }
    let actual = evidence
        .subject_outcomes()
        .iter()
        .find(|outcome| outcome.subject() == subject)
        .and_then(|outcome| match outcome.status() {
            super::ProbeSubjectStatus::Verified {
                evidence_fingerprint,
            } => Some(*evidence_fingerprint),
            super::ProbeSubjectStatus::Rejected { .. } => None,
        })
        .ok_or(ContractError::ProbeSubjectMismatch {
            probe: *probe,
            declaration,
            subject: subject_label,
        })?;
    let expected = layout.fingerprint()?;
    if actual != expected {
        return Err(ContractError::ProbeOutcomeFingerprintMismatch {
            probe: *probe,
            declaration,
        });
    }
    Ok(())
}

fn require_provider_and_symbol_not_required(
    declaration: DeclarationId,
    evidence: &DeclarationEvidence,
) -> Result<(), ContractError> {
    if !matches!(evidence.provider(), ProviderAssessment::NotRequired)
        || !matches!(evidence.symbol(), SymbolAssessment::NotRequired)
    {
        return Err(ContractError::IncoherentDeclarationEvidence {
            declaration,
            reason: "non-linked declaration must not carry provider or symbol evidence",
        });
    }
    Ok(())
}

fn require_callable_not_applicable(
    declaration: DeclarationId,
    evidence: &DeclarationEvidence,
) -> Result<(), ContractError> {
    if !matches!(
        evidence.callable_abi(),
        CallableAbiAssessment::NotApplicable
    ) {
        return Err(ContractError::IncoherentDeclarationEvidence {
            declaration,
            reason: "non-callable declaration must mark callable ABI not applicable",
        });
    }
    Ok(())
}

fn validate_no_layout_dimension(
    declaration: DeclarationId,
    evidence: &DeclarationEvidence,
    layouts: &BTreeMap<DeclarationId, &LayoutEvidence>,
) -> Result<(), ContractError> {
    if !matches!(evidence.layout(), LayoutAssessment::NotRequired)
        || layouts.contains_key(&declaration)
    {
        return Err(ContractError::IncoherentDeclarationEvidence {
            declaration,
            reason: "declaration does not require concrete layout evidence",
        });
    }
    Ok(())
}

fn validate_probe_subjects_against_source(
    source: &CompleteSourcePackage,
    closure: &BTreeMap<DeclarationId, ClosureRequirement>,
    probes: &BTreeMap<ProbeEvidenceId, &AbiProbeEvidence>,
) -> Result<(), ContractError> {
    for probe in probes.values() {
        for subject in probe.subjects() {
            let declaration_id = subject.declaration();
            if !closure.contains_key(&declaration_id) {
                return Err(ContractError::DeclarationOutsideClosure {
                    declaration: declaration_id,
                });
            }
            let declaration = source.source().declaration(declaration_id).ok_or(
                ContractError::DeclarationOutsideClosure {
                    declaration: declaration_id,
                },
            )?;
            let compatible = matches!(
                (subject, &declaration.kind),
                (
                    ProbeSubject::RecordLayout { .. },
                    SourceDeclarationKind::Record(_)
                ) | (
                    ProbeSubject::EnumRepresentation { .. },
                    SourceDeclarationKind::Enum(_)
                ) | (
                    ProbeSubject::CallableAbi { .. },
                    SourceDeclarationKind::Function(_)
                )
            );
            if !compatible {
                return Err(ContractError::ProbeSubjectMismatch {
                    probe: probe.id(),
                    declaration: declaration_id,
                    subject: match subject {
                        ProbeSubject::RecordLayout { .. } => "record layout",
                        ProbeSubject::EnumRepresentation { .. } => "enum representation",
                        ProbeSubject::CallableAbi { .. } => "callable ABI",
                    },
                });
            }
        }
    }
    Ok(())
}

fn validate_probe_runner_policy(
    probe: &AbiProbeEvidence,
    policy: &AnalysisPolicy,
) -> Result<(), ContractError> {
    match (
        probe.method(),
        probe.runner(),
        policy.probe(),
        policy.runner(),
    ) {
        (
            ProbeMethod::ExecutedHarness,
            super::ProbeRunnerEvidence::Executed {
                executable_path,
                executable_fingerprint,
                arguments,
            },
            ProbePolicy::CompileAndRun,
            RunnerPolicy::Explicit(command),
        ) if executable_path == command.program()
            && *executable_fingerprint == command.executable_fingerprint()
            && arguments == command.arguments() =>
        {
            Ok(())
        }
        (
            ProbeMethod::CompilerLayoutDump | ProbeMethod::CompileTimeAssertion,
            super::ProbeRunnerEvidence::NotExecuted,
            ProbePolicy::CompileOnly | ProbePolicy::CompileAndRun,
            _,
        ) => Ok(()),
        _ => Err(ContractError::InvalidPolicy {
            reason: "ABI probe runner evidence differs from package analysis policy",
        }),
    }
}

fn validate_layout_owner(
    source: &CompleteSourcePackage,
    layout: &LayoutEvidence,
) -> Result<(), ContractError> {
    let declaration_id = layout.declaration();
    let declaration = source.source().declaration(declaration_id).ok_or(
        ContractError::InvalidLayoutDeclaration {
            declaration: declaration_id,
        },
    )?;
    match (&declaration.kind, layout) {
        (SourceDeclarationKind::Record(record), LayoutEvidence::Record(evidence)) => {
            let expected: BTreeSet<_> = record.fields.iter().map(|field| field.id).collect();
            let actual: BTreeSet<_> = evidence
                .fields()
                .iter()
                .map(|field| field.child())
                .collect();
            if let Some(child) = actual.difference(&expected).next() {
                return Err(ContractError::ForeignLayoutChild {
                    declaration: declaration_id,
                    child: *child,
                });
            }
            if expected != actual {
                return Err(ContractError::RequiredLayoutEvidence {
                    declaration: declaration_id,
                });
            }
        }
        (SourceDeclarationKind::Enum(enumeration), LayoutEvidence::Enum(evidence)) => {
            let expected: BTreeSet<_> = enumeration
                .variants
                .iter()
                .map(|variant| variant.id)
                .collect();
            let actual: BTreeSet<_> = evidence
                .variants()
                .iter()
                .map(EnumVariantEvidence::child)
                .collect();
            if let Some(child) = actual.difference(&expected).next() {
                return Err(ContractError::ForeignLayoutChild {
                    declaration: declaration_id,
                    child: *child,
                });
            }
            if expected != actual {
                return Err(ContractError::RequiredLayoutEvidence {
                    declaration: declaration_id,
                });
            }
            for variant_evidence in evidence.variants() {
                let variant = enumeration
                    .variants
                    .iter()
                    .find(|variant| variant.id == variant_evidence.child())
                    .ok_or(ContractError::ForeignLayoutChild {
                        declaration: declaration_id,
                        child: variant_evidence.child(),
                    })?;
                if let EnumValue::Evaluated { value } = &variant.value {
                    if value != variant_evidence.value() {
                        return Err(ContractError::EnumValueMismatch {
                            declaration: declaration_id,
                            child: variant.id,
                        });
                    }
                }
            }
        }
        _ => {
            return Err(ContractError::InvalidLayoutDeclaration {
                declaration: declaration_id,
            });
        }
    }
    Ok(())
}

fn canonicalize_collections(input: &mut LinkAnalysisPackageInput) -> Result<(), ContractError> {
    input
        .inventories
        .sort_by_key(|inventory| inventory.artifact().provider_id());
    input.abi_probes.sort_by_key(AbiProbeEvidence::id);
    input.layouts.sort_by_key(LayoutEvidence::declaration);
    input
        .declaration_evidence
        .sort_by_key(DeclarationEvidence::declaration);
    input.diagnostics.sort_by(diagnostic_order);
    if input
        .diagnostics
        .windows(2)
        .any(|pair| diagnostic_order(&pair[0], &pair[1]).is_eq())
    {
        return Err(ContractError::DuplicateDiagnostic);
    }
    Ok(())
}

fn validate_canonical_order(package: &LinkAnalysisPackage) -> Result<(), ContractError> {
    require_sorted(
        &package.inventories,
        |inventory| inventory.artifact().provider_id(),
        "inventories",
    )?;
    require_sorted(&package.abi_probes, AbiProbeEvidence::id, "ABI probes")?;
    require_sorted(&package.layouts, LayoutEvidence::declaration, "layouts")?;
    require_sorted(
        &package.declaration_evidence,
        DeclarationEvidence::declaration,
        "declaration evidence",
    )?;
    if !package
        .diagnostics
        .windows(2)
        .all(|pair| diagnostic_order(&pair[0], &pair[1]).is_lt())
    {
        return Err(ContractError::NonCanonicalOrder {
            collection: "diagnostics",
        });
    }
    Ok(())
}

fn require_sorted<T, K: Ord>(
    values: &[T],
    key: impl Fn(&T) -> K,
    collection: &'static str,
) -> Result<(), ContractError> {
    if values.windows(2).all(|pair| key(&pair[0]) <= key(&pair[1])) {
        Ok(())
    } else {
        Err(ContractError::NonCanonicalOrder { collection })
    }
}

fn diagnostic_order(left: &LincDiagnostic, right: &LincDiagnostic) -> std::cmp::Ordering {
    (
        left.stage(),
        left.severity(),
        left.code(),
        left.declaration(),
        left.provider(),
        left.context(),
        left.message(),
    )
        .cmp(&(
            right.stage(),
            right.severity(),
            right.code(),
            right.declaration(),
            right.provider(),
            right.context(),
            right.message(),
        ))
}
