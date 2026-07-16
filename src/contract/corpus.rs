//! Embedded, package-safe access to the H1 preservation link-analysis corpus.

use std::str::FromStr as _;

use parc::contract::{CompleteSourcePackage, DeclarationId, Selection};
use thiserror::Error;

use super::{
    decode_link_analysis, ContractError, DecodeError, LinkAnalysisFingerprint, LinkAnalysisPackage,
    ValidatedLinkAnalysis,
};

/// The checked schema-v2 LINC package paired with PARC's complete preservation
/// source-package artifact.
pub const PRESERVATION_LINK_ANALYSIS_JSON: &[u8] =
    include_bytes!("../../contract-corpus/v2/preservation/link-analysis.json");

/// Frozen identity of [`PRESERVATION_LINK_ANALYSIS_JSON`].
pub const PRESERVATION_LINK_ANALYSIS_FINGERPRINT: &str =
    "lanalysis2_470f6007292587b5d92e1ecc5c92c8baa67caf96f39e6492eb6f6bfabe35db37";

const PRESERVATION_ROOTS: [&str; 3] = [
    "pdecl1_524bcccd395cfaad5d0697f01bc545663e82eaad03be1e515beeb81933f5b37d",
    "pdecl1_b81fc55bc5e16f7145cee441d41748a09a5e93c3ae37cc0d36d530034a05e6c5",
    "pdecl1_efa4c3e08588aadceced27ba10c722fc59563b99397009d83e59edea080ca726",
];

/// Errors possible while loading and checking the embedded preservation pair.
#[derive(Debug, Error)]
pub enum PreservationCorpusError {
    #[error("embedded preservation link analysis did not decode: {0}")]
    Decode(#[from] DecodeError),
    #[error("embedded preservation link analysis does not cover the supplied source closure: {0}")]
    Contract(#[from] ContractError),
}

/// The exact PARC root selection covered by the preservation link analysis.
///
/// This deliberately excludes the corpus' unresolved `parc_missing`
/// declaration. The selected closure still includes every transitive type
/// needed by these roots.
pub fn preservation_selection() -> Selection {
    Selection::only(
        PRESERVATION_ROOTS.map(|value| {
            DeclarationId::from_str(value).expect("frozen preservation declaration id")
        }),
    )
    .expect("frozen preservation selection is nonempty and distinct")
}

/// Decode and fully check the embedded schema-v2 LINC artifact.
pub fn decode_preservation_link_analysis() -> Result<LinkAnalysisPackage, DecodeError> {
    decode_link_analysis(PRESERVATION_LINK_ANALYSIS_JSON)
}

/// Decode the embedded artifact and prove that it exactly covers `source`.
pub fn validated_preservation_link_analysis(
    source: &CompleteSourcePackage,
) -> Result<ValidatedLinkAnalysis, PreservationCorpusError> {
    let package = decode_preservation_link_analysis()?;
    Ok(ValidatedLinkAnalysis::try_new(source, package)?)
}

/// Parse the frozen identity without requiring consumers to duplicate its
/// textual representation.
pub fn preservation_link_analysis_fingerprint() -> LinkAnalysisFingerprint {
    LinkAnalysisFingerprint::from_str(PRESERVATION_LINK_ANALYSIS_FINGERPRINT)
        .expect("frozen preservation link-analysis fingerprint")
}
