use std::path::PathBuf;

use parc::contract::{
    ChildId, ContentFingerprint, DeclarationId, SourceFingerprint, TargetFingerprint,
};
use thiserror::Error;

use super::{ArtifactFingerprint, ArtifactSymbolId, ProbeEvidenceId, ProviderId, SymbolKind};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ContractError {
    #[error("{field} must be nonempty and contain no NUL byte")]
    InvalidText { field: &'static str },
    #[error("diagnostic code {value:?} must match LINC-[ENPW]dddd")]
    InvalidDiagnosticCode { value: String },
    #[error("diagnostic code {code} requires {expected} severity, found {actual}")]
    DiagnosticSeverityMismatch {
        code: String,
        expected: &'static str,
        actual: &'static str,
    },
    #[error("probe rejection code {code} must use the LINC-E or LINC-P class")]
    InvalidProbeRejectionCode { code: String },
    #[error("{field} must be an absolute, lexically normalized native path: {path:?}")]
    InvalidPath { field: &'static str, path: PathBuf },
    #[error("{field} must be a nonempty native string without NUL or path separators")]
    InvalidNativeString { field: &'static str },
    #[error("wire native-string platform {found:?} cannot be decoded on {host:?}")]
    NativePathPlatform {
        found: &'static str,
        host: &'static str,
    },
    #[error("observed pointer width {bits} is not a supported byte-aligned width")]
    InvalidPointerWidth { bits: u16 },
    #[error("layout for declaration {declaration} has invalid size/alignment")]
    InvalidLayout { declaration: DeclarationId },
    #[error("layout for declaration {declaration} repeats child {child}")]
    DuplicateLayoutChild {
        declaration: DeclarationId,
        child: ChildId,
    },
    #[error(
        "ambiguous declaration evidence for {declaration} requires at least two distinct providers"
    )]
    InvalidAmbiguity { declaration: DeclarationId },
    #[error("link group ends before a matching group start at atom {index}")]
    UnexpectedGroupEnd { index: usize },
    #[error("resolved link plan has {depth} unclosed group(s)")]
    UnclosedGroups { depth: usize },
    #[error("link atom kind {atom_kind} does not match artifact kind {artifact_kind}")]
    ArtifactKindMismatch {
        atom_kind: &'static str,
        artifact_kind: &'static str,
    },
    #[error(
        "symbol inventory for provider {provider} has an invalid symbol at index {index}: {reason}"
    )]
    InvalidSymbol {
        provider: ProviderId,
        index: usize,
        reason: &'static str,
    },
    #[error("symbol inventory for provider {provider} repeats symbol identity {symbol:?}")]
    DuplicateSymbol {
        provider: ProviderId,
        symbol: String,
    },
    #[error(
        "symbol inventory for provider {provider} repeats artifact-local symbol id {symbol:?}"
    )]
    DuplicateArtifactSymbolId {
        provider: ProviderId,
        symbol: ArtifactSymbolId,
    },
    #[error("symbol inspection provenance is invalid: {reason}")]
    InvalidInspectionProvenance { reason: &'static str },
    #[error("inspection parser identities do not match provider {provider} artifact format")]
    InspectionParserMismatch { provider: ProviderId },
    #[error("ABI probe evidence must contain at least one subject")]
    EmptyProbeSubjects,
    #[error("ABI probe evidence repeats subject {subject}")]
    DuplicateProbeSubject { subject: String },
    #[error("ABI probe has {subjects} subjects but {outcomes} subject outcomes")]
    ProbeSubjectCountMismatch { subjects: usize, outcomes: usize },
    #[error("ABI probe result is inconsistent: {reason}")]
    InvalidProbeResult { reason: &'static str },
    #[error("probe environment is inconsistent: {reason}")]
    InvalidProbeEnvironment { reason: &'static str },
    #[error("stored probe environment fingerprint {stored} differs from derived {derived}")]
    ProbeEnvironmentFingerprintMismatch {
        stored: ContentFingerprint,
        derived: ContentFingerprint,
    },
    #[error("stored ABI probe id {stored} differs from derived id {derived}")]
    ProbeIdMismatch {
        stored: ProbeEvidenceId,
        derived: ProbeEvidenceId,
    },
    #[error("ABI probe evidence {probe} occurs more than once")]
    DuplicateProbeEvidence { probe: ProbeEvidenceId },
    #[error("ABI probe evidence {probe} is absent from the package")]
    MissingProbeEvidence { probe: ProbeEvidenceId },
    #[error("ABI probe evidence {probe} does not cover declaration {declaration} as {subject}")]
    ProbeSubjectMismatch {
        probe: ProbeEvidenceId,
        declaration: DeclarationId,
        subject: &'static str,
    },
    #[error(
        "ABI probe evidence {probe} outcome fingerprint does not bind layout declaration {declaration}"
    )]
    ProbeOutcomeFingerprintMismatch {
        probe: ProbeEvidenceId,
        declaration: DeclarationId,
    },
    #[error("ABI probe evidence {probe} compiler or ABI flags differ from the PARC target")]
    ProbeCompilerMismatch { probe: ProbeEvidenceId },
    #[error("ABI probe evidence {probe} uses an incompatible method for {subject}")]
    ProbeMethodMismatch {
        probe: ProbeEvidenceId,
        subject: &'static str,
    },
    #[error("provider {provider} occurs more than once")]
    DuplicateProvider { provider: ProviderId },
    #[error("stored provider id {stored} differs from derived provider id {derived}")]
    ProviderIdMismatch {
        stored: ProviderId,
        derived: ProviderId,
    },
    #[error("declaration {declaration} has repeated {evidence_kind} evidence")]
    DuplicateDeclarationEvidence {
        declaration: DeclarationId,
        evidence_kind: &'static str,
    },
    #[error("provider {provider} references absent parent provider {parent}")]
    MissingParentProvider {
        provider: ProviderId,
        parent: ProviderId,
    },
    #[error("dependency parent {parent} and child {child} do not cross-reference each other")]
    DependencyCrossReference {
        parent: ProviderId,
        child: ProviderId,
    },
    #[error("provider {provider} cannot depend on itself")]
    SelfParentProvider { provider: ProviderId },
    #[error("provider dependency graph contains a cycle through {provider}")]
    DependencyCycle { provider: ProviderId },
    #[error("provider {parent} has an unresolved transitive dependency")]
    UnresolvedDependency { parent: ProviderId },
    #[error("dependency provider {child} does not follow parent {parent} in the link plan")]
    DependencyPlanOrder {
        parent: ProviderId,
        child: ProviderId,
    },
    #[error("provider {provider} search resolution references invalid native input {index}")]
    InvalidResolutionInput { provider: ProviderId, index: u32 },
    #[error("provider {provider} is absent from symbol inventories")]
    MissingProvider { provider: ProviderId },
    #[error("provider {provider} has artifact fingerprint {actual}, expected {expected}")]
    ArtifactFingerprintMismatch {
        provider: ProviderId,
        expected: ArtifactFingerprint,
        actual: ArtifactFingerprint,
    },
    #[error("resolved link atom for provider {provider} differs from its inventory artifact")]
    PlanArtifactMismatch { provider: ProviderId },
    #[error("provider {provider} is not present in the resolved link plan")]
    ProviderNotInPlan { provider: ProviderId },
    #[error("declaration {declaration} has {count} visible providers in the resolved plan")]
    AmbiguousVisibleProviders {
        declaration: DeclarationId,
        count: usize,
    },
    #[error(
        "declaration {declaration} references absent symbol {symbol:?} from provider {provider}"
    )]
    MissingSymbolIdentity {
        declaration: DeclarationId,
        provider: ProviderId,
        symbol: ArtifactSymbolId,
    },
    #[error("declaration {declaration} references a symbol that is imported, local, or hidden")]
    SymbolNotVisible { declaration: DeclarationId },
    #[error("declaration {declaration} requires symbol kind {expected:?}, found {actual:?}")]
    SymbolKindMismatch {
        declaration: DeclarationId,
        expected: SymbolKind,
        actual: SymbolKind,
    },
    #[error("declaration {declaration} symbol evidence differs from its inventory record")]
    SymbolEvidenceMismatch { declaration: DeclarationId },
    #[error("declaration {declaration} has invalid symbol decoration evidence: {reason}")]
    InvalidSymbolDecoration {
        declaration: DeclarationId,
        reason: &'static str,
    },
    #[error("provider {provider} does not export symbol {symbol:?}")]
    MissingExportedSymbol {
        provider: ProviderId,
        symbol: String,
    },
    #[error(
        "declaration {declaration} expects link symbol {expected:?}, evidence names {actual:?}"
    )]
    SymbolNameMismatch {
        declaration: DeclarationId,
        expected: String,
        actual: String,
    },
    #[error("{evidence_kind} evidence target {actual} differs from package target {expected}")]
    EvidenceTargetMismatch {
        evidence_kind: &'static str,
        expected: TargetFingerprint,
        actual: TargetFingerprint,
    },
    #[error("{evidence_kind} evidence source {actual} differs from package source {expected}")]
    EvidenceSourceFingerprintMismatch {
        evidence_kind: &'static str,
        expected: SourceFingerprint,
        actual: SourceFingerprint,
    },
    #[error("source fingerprint {actual} differs from required source {expected}")]
    SourceFingerprintMismatch {
        expected: SourceFingerprint,
        actual: SourceFingerprint,
    },
    #[error("target fingerprint {actual} differs from required target {expected}")]
    TargetFingerprintMismatch {
        expected: TargetFingerprint,
        actual: TargetFingerprint,
    },
    #[error("observed target for provider {provider} does not match the requested target")]
    ObservedTargetMismatch { provider: ProviderId },
    #[error("declaration evidence references {declaration}, which is outside the complete source closure")]
    DeclarationOutsideClosure { declaration: DeclarationId },
    #[error("layout evidence references missing or incompatible declaration {declaration}")]
    InvalidLayoutDeclaration { declaration: DeclarationId },
    #[error("layout evidence for declaration {declaration} references foreign child {child}")]
    ForeignLayoutChild {
        declaration: DeclarationId,
        child: ChildId,
    },
    #[error("enum evidence for declaration {declaration} disagrees on child {child} value")]
    EnumValueMismatch {
        declaration: DeclarationId,
        child: ChildId,
    },
    #[error("selected declaration {declaration} requires resolved symbol evidence")]
    RequiredSymbolEvidence { declaration: DeclarationId },
    #[error("selected declaration {declaration} requires layout evidence")]
    RequiredLayoutEvidence { declaration: DeclarationId },
    #[error("selected declaration {declaration} requires a declaration-evidence record")]
    RequiredDeclarationEvidence { declaration: DeclarationId },
    #[error("selected declaration {declaration} has only inferred layout evidence")]
    InferredLayoutEvidence { declaration: DeclarationId },
    #[error("selected callable declaration {declaration} lacks confirmed ABI evidence")]
    RequiredCallableAbiEvidence { declaration: DeclarationId },
    #[error("declaration {declaration} has incoherent evidence dimensions: {reason}")]
    IncoherentDeclarationEvidence {
        declaration: DeclarationId,
        reason: &'static str,
    },
    #[error("analysis policy is inconsistent: {reason}")]
    InvalidPolicy { reason: &'static str },
    #[error("diagnostic context is inconsistent: {reason}")]
    InvalidDiagnosticContext { reason: &'static str },
    #[error("diagnostic collection contains an exact duplicate")]
    DuplicateDiagnostic,
    #[error("{collection} is not in canonical contract order")]
    NonCanonicalOrder { collection: &'static str },
    #[error("could not canonicalize link-analysis package: {message}")]
    Canonical { message: String },
}
