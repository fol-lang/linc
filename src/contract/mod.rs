//! Parser-independent, checked link and ABI evidence contract.
//!
//! This is LINC's canonical public surface. Serialized values use the strict
//! schema-v2 codec and validated values bind exactly to a complete PARC source
//! closure.

mod codec;
pub mod corpus;
mod error;
mod evidence;
mod identity;
mod link;
mod model;
mod package;
mod request;
mod schema;
mod wire;

pub use codec::{
    decode_link_analysis, decode_link_analysis_with_limits, encode_link_analysis, DecodeError,
    DecodeLimits, EncodeError,
};
pub use error::ContractError;
pub use evidence::*;
pub use identity::{
    ArtifactFingerprint, IdentityParseError, LinkAnalysisFingerprint, ProbeEvidenceId, ProviderId,
};
pub use link::{LinkAtom, ResolvedLinkPlan};
pub use model::*;
pub use package::{LinkAnalysisPackage, LinkAnalysisPackageInput, ValidatedLinkAnalysis};
pub use parc::contract::{
    ChildId, DeclarationId, SchemaHeader, SourceFingerprint, TargetFingerprint,
};
pub use request::*;
pub use schema::{LINK_ANALYSIS_KIND, LINK_ANALYSIS_SCHEMA_ID, LINK_ANALYSIS_SCHEMA_VERSION};

#[cfg(test)]
mod tests;
