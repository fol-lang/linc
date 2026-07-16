use std::collections::BTreeMap;

use parc::contract::{
    ClosureRequirement, CompleteSourcePackage, DeclarationId, Linkage, RecordCompleteness,
    SourceDeclarationKind,
};

use crate::contract::{
    AbiProbeEvidence, CallableAbiAssessment, DeclarationEvidence, DeclarationEvidenceInput,
    DiagnosticEvidenceRef, LayoutAssessment, LayoutEvidence, LincDiagnostic, LinkAnalysisPackage,
    LinkAnalysisPackageInput, ProviderAssessment, SymbolAssessment, SymbolDecoration,
    ValidatedLinkAnalysis,
};

use super::{
    AbiShapeEvidence, NativeError, NativeResolver, NativeResult, StrictDeclarationRequest,
    StrictEvidenceValidator,
};

/// Owned strict-validation request for one linked source declaration.
///
/// Provider and symbol facts are never supplied by the caller: the analyzer
/// derives them from its inspected resolution. Callable evidence remains an
/// explicit typed input because different certified ABIs require different
/// probe strategies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeDeclarationRequest {
    declaration: DeclarationId,
    decoration: SymbolDecoration,
    callable_abi: CallableAbiAssessment,
    abi_shape: Option<AbiShapeEvidence>,
}

impl NativeDeclarationRequest {
    pub fn new(
        declaration: DeclarationId,
        decoration: SymbolDecoration,
        callable_abi: CallableAbiAssessment,
        abi_shape: Option<AbiShapeEvidence>,
    ) -> Self {
        Self {
            declaration,
            decoration,
            callable_abi,
            abi_shape,
        }
    }

    pub const fn declaration(&self) -> DeclarationId {
        self.declaration
    }

    pub const fn decoration(&self) -> &SymbolDecoration {
        &self.decoration
    }

    pub const fn callable_abi(&self) -> &CallableAbiAssessment {
        &self.callable_abi
    }

    pub const fn abi_shape(&self) -> Option<&AbiShapeEvidence> {
        self.abi_shape.as_ref()
    }
}

/// Explicit measured inputs consumed by [`NativeAnalyzer`].
///
/// These are durable contract types, not a second evidence schema. The final
/// `ValidatedLinkAnalysis` gate checks every fingerprint, probe subject,
/// layout, declaration, and policy cross-reference after native inspection.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NativeAnalysisInput {
    pub abi_probes: Vec<AbiProbeEvidence>,
    pub layouts: Vec<LayoutEvidence>,
    pub declarations: Vec<NativeDeclarationRequest>,
    pub diagnostics: Vec<LincDiagnostic>,
}

/// LINC-owned authoritative native-analysis orchestration.
///
/// Downstream consumers call this boundary rather than assembling
/// `LinkAnalysisPackageInput` themselves. Resolution, declaration validation,
/// closure evidence, package construction, and final source validation are one
/// fail-closed operation.
#[derive(Debug, Clone)]
pub struct NativeAnalyzer {
    resolver: NativeResolver,
    validator: StrictEvidenceValidator,
}

impl NativeAnalyzer {
    pub fn new(resolver: NativeResolver) -> Self {
        Self {
            resolver,
            validator: StrictEvidenceValidator,
        }
    }

    pub const fn resolver(&self) -> &NativeResolver {
        &self.resolver
    }

    pub fn analyze(
        &self,
        request: &crate::contract::AnalysisRequest<'_>,
        input: NativeAnalysisInput,
    ) -> NativeResult<ValidatedLinkAnalysis> {
        validate_explicit_evidence(request.source(), &input)?;
        let resolution = self.resolver.resolve(request)?;
        let mut requests = declaration_request_map(input.declarations)?;
        let mut declaration_evidence =
            Vec::with_capacity(request.source().declaration_closure().len());

        for entry in request.source().declaration_closure() {
            let declaration = request
                .source()
                .source()
                .declaration(entry.declaration())
                .ok_or_else(|| NativeError::InvalidPolicy {
                    detail: format!(
                        "complete source closure lost declaration {}",
                        entry.declaration()
                    ),
                })?;
            let linked = matches!(
                &declaration.kind,
                SourceDeclarationKind::Function(_) | SourceDeclarationKind::Variable(_)
            ) && declaration.linkage == Linkage::External;
            if linked {
                let explicit = requests.remove(&entry.declaration()).ok_or_else(|| {
                    NativeError::AbiMismatch {
                        declaration: entry.declaration(),
                        detail: "linked closure member has no strict declaration request"
                            .to_owned(),
                    }
                })?;
                declaration_evidence.push(self.validator.validate_declaration(
                    request.source(),
                    &resolution,
                    &input.abi_probes,
                    &input.layouts,
                    StrictDeclarationRequest {
                        declaration: explicit.declaration,
                        decoration: explicit.decoration,
                        layout: LayoutAssessment::NotRequired,
                        callable_abi: explicit.callable_abi,
                        abi_shape: explicit.abi_shape.as_ref(),
                    },
                )?);
            } else {
                declaration_evidence.push(unlinked_evidence(
                    request.source(),
                    entry.declaration(),
                    entry.requirement(),
                    &input.layouts,
                )?);
            }
        }
        if let Some(declaration) = requests.keys().next().copied() {
            return Err(NativeError::InvalidPolicy {
                detail: format!(
                    "strict declaration request {declaration} is not a linked member of the source closure"
                ),
            });
        }

        let (resolved_link_plan, inventories) = resolution.into_parts();
        let package = LinkAnalysisPackage::try_new(LinkAnalysisPackageInput {
            source_fingerprint: request.source().source().fingerprint(),
            target_fingerprint: request.source().source().target_fingerprint(),
            analysis_policy: request.policy().clone(),
            native_inputs: request.native_inputs().to_vec(),
            inventories,
            abi_probes: input.abi_probes,
            layouts: input.layouts,
            declaration_evidence,
            resolved_link_plan,
            diagnostics: input.diagnostics,
        })?;
        Ok(ValidatedLinkAnalysis::try_new(request.source(), package)?)
    }
}

impl Default for NativeAnalyzer {
    fn default() -> Self {
        Self::new(NativeResolver::default())
    }
}

fn declaration_request_map(
    declarations: Vec<NativeDeclarationRequest>,
) -> NativeResult<BTreeMap<DeclarationId, NativeDeclarationRequest>> {
    let mut requests = BTreeMap::new();
    for declaration in declarations {
        let id = declaration.declaration;
        if requests.insert(id, declaration).is_some() {
            return Err(NativeError::InvalidPolicy {
                detail: format!("duplicate strict declaration request {id}"),
            });
        }
    }
    Ok(requests)
}

fn validate_explicit_evidence(
    source: &CompleteSourcePackage,
    input: &NativeAnalysisInput,
) -> NativeResult<()> {
    let expected_source = source.source().fingerprint();
    let expected_target = source.source().target_fingerprint();
    for probe in &input.abi_probes {
        if probe.source_fingerprint() != expected_source
            || probe.target_fingerprint() != expected_target
        {
            return Err(NativeError::InvalidPolicy {
                detail: format!(
                    "probe {} has stale source or target fingerprints",
                    probe.id()
                ),
            });
        }
    }
    for layout in &input.layouts {
        if layout.source_fingerprint() != expected_source
            || layout.target_fingerprint() != expected_target
        {
            return Err(NativeError::AbiMismatch {
                declaration: layout.declaration(),
                detail: "layout has stale source or target fingerprints".to_owned(),
            });
        }
    }
    for diagnostic in &input.diagnostics {
        if diagnostic.context().target_fingerprint() != expected_target {
            return Err(NativeError::InvalidPolicy {
                detail: format!(
                    "diagnostic {} has a stale target fingerprint",
                    diagnostic.code()
                ),
            });
        }
        if let Some(DiagnosticEvidenceRef::Layout { declaration }) = diagnostic.context().evidence()
        {
            if !source
                .declaration_closure()
                .iter()
                .any(|entry| entry.declaration() == *declaration)
            {
                return Err(NativeError::InvalidPolicy {
                    detail: format!(
                        "diagnostic layout reference {declaration} is outside the source closure"
                    ),
                });
            }
        }
    }
    Ok(())
}

fn unlinked_evidence(
    source: &CompleteSourcePackage,
    declaration: DeclarationId,
    requirement: ClosureRequirement,
    layouts: &[LayoutEvidence],
) -> NativeResult<DeclarationEvidence> {
    let source_declaration =
        source
            .source()
            .declaration(declaration)
            .ok_or_else(|| NativeError::InvalidPolicy {
                detail: format!("closure declaration {declaration} is missing"),
            })?;
    let requires_layout = matches!(
        &source_declaration.kind,
        SourceDeclarationKind::Record(record)
            if requirement == ClosureRequirement::Definition
                && record.completeness == RecordCompleteness::Complete
    ) || matches!(
        &source_declaration.kind,
        SourceDeclarationKind::Enum(_) if requirement == ClosureRequirement::Definition
    );
    let layout = if requires_layout {
        let mut matching = layouts
            .iter()
            .filter(|layout| layout.declaration() == declaration);
        let evidence = matching.next().ok_or_else(|| NativeError::AbiMismatch {
            declaration,
            detail: "definition-required source type has no measured layout".to_owned(),
        })?;
        if matching.next().is_some() {
            return Err(NativeError::AbiMismatch {
                declaration,
                detail: "source type has duplicate measured layouts".to_owned(),
            });
        }
        LayoutAssessment::Available {
            confidence: evidence.confidence(),
            probe: evidence.probe(),
        }
    } else {
        LayoutAssessment::NotRequired
    };
    Ok(DeclarationEvidence::try_new(DeclarationEvidenceInput {
        declaration,
        source_fingerprint: source.source().fingerprint(),
        target_fingerprint: source.source().target_fingerprint(),
        provider: ProviderAssessment::NotRequired,
        symbol: SymbolAssessment::NotRequired,
        layout,
        callable_abi: CallableAbiAssessment::NotApplicable,
    })?)
}
