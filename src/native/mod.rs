//! Trustworthy native evidence implementation.
//!
//! This module is available only with `native-inspection`. It inspects native
//! artifacts and runs bounded probes, but its durable outputs are the types in
//! [`crate::contract`]; there is no second serialized evidence model.

mod analyze;
mod certify;
mod error;
mod inspect;
mod probe;
mod resolve;
mod sysv;
mod validate;

pub use analyze::{NativeAnalysisInput, NativeAnalyzer, NativeDeclarationRequest};
pub use certify::CertificationToolchain;
pub use error::{NativeError, NativeResult};
pub use inspect::{
    ArtifactInspection, InspectionLimits, NativeInspector, NeededLibrary, OBJECT_PARSER_VERSION,
};
pub use probe::{
    EnvironmentSetting, ProbeExpectation, ProbeProgram, ProbeRejection, ProbeRejectionKind,
    ProbeRequest, ProbeRunOutcome, ProbeRunner, RunnerSpec,
};
pub use resolve::{LibraryPreference, NativeResolution, NativeResolver, ResolverConfiguration};
pub use validate::{
    AbiDimension, AbiShapeEvidence, ReturnConvention, StrictDeclarationRequest,
    StrictEvidenceValidator, ValuePassing,
};
