use std::{io, path::PathBuf};

use thiserror::Error;

use crate::contract::ContractError;
use crate::contract::DeclarationId;

pub type NativeResult<T> = Result<T, NativeError>;

/// Operational failures emitted by the native implementation.
///
/// Every variant maps to a stable LINC diagnostic code so callers never need
/// to classify failures by matching human-readable text.
#[derive(Debug, Error)]
pub enum NativeError {
    #[error("{operation} failed for {path}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("artifact {path} exceeds the {limit}-byte inspection limit")]
    ArtifactTooLarge { path: PathBuf, limit: u64 },
    #[error("artifact {path} is empty")]
    EmptyArtifact { path: PathBuf },
    #[error("artifact {path} is truncated or corrupt: {detail}")]
    CorruptArtifact { path: PathBuf, detail: String },
    #[error("unsupported native artifact at {path}: {detail}")]
    UnsupportedArtifact { path: PathBuf, detail: String },
    #[error("artifact kind mismatch for {path}: expected {expected}, observed {observed}")]
    ArtifactKindMismatch {
        path: PathBuf,
        expected: &'static str,
        observed: &'static str,
    },
    #[error("artifact target mismatch for {path}: requested {requested}, observed {observed}")]
    TargetMismatch {
        path: PathBuf,
        requested: String,
        observed: String,
    },
    #[error("invalid symbol in {path}: {detail}")]
    InvalidSymbol { path: PathBuf, detail: String },
    #[error("native provider {requested:?} was not found")]
    MissingProvider { requested: std::ffi::OsString },
    #[error("native provider {requested:?} is ambiguous: {candidates:?}")]
    AmbiguousProvider {
        requested: std::ffi::OsString,
        candidates: Vec<PathBuf>,
    },
    #[error("native provider dependency cycle includes {path}")]
    DependencyCycle { path: PathBuf },
    #[error("unsupported native input for this certified lane: {detail}")]
    UnsupportedInput { detail: String },
    #[error("invalid native execution policy: {detail}")]
    InvalidPolicy { detail: String },
    #[error("tool identity check failed for {path}: {detail}")]
    ToolIdentity { path: PathBuf, detail: String },
    #[error("probe process timed out after {millis} ms")]
    ProbeTimeout { millis: u64 },
    #[error("probe process exceeded the {limit}-byte output limit")]
    ProbeOutputLimit { limit: u64 },
    #[error("probe process exited unsuccessfully: {detail}")]
    ProbeNonzero { detail: String },
    #[error("probe output did not satisfy the evidence protocol: {detail}")]
    ProbeParserGap { detail: String },
    #[error("probe process left inherited output streams open")]
    ProbeUnsafeStreams,
    #[error("cross-target execution requires an explicit runner: {target}")]
    MissingCrossRunner { target: String },
    #[error("runner evidence is invalid: {detail}")]
    InvalidRunner { detail: String },
    #[error("symbol evidence for declaration {declaration} was rejected: {detail}")]
    SymbolRejected {
        declaration: DeclarationId,
        detail: String,
    },
    #[error("ABI evidence for declaration {declaration} was rejected: {detail}")]
    AbiMismatch {
        declaration: DeclarationId,
        detail: String,
    },
    #[error("contract construction rejected native evidence: {0}")]
    Contract(#[from] ContractError),
}

impl NativeError {
    /// Stable diagnostic identity for this failure class.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Io { .. } => "LINC-E3001",
            Self::ArtifactTooLarge { .. } => "LINC-E3002",
            Self::EmptyArtifact { .. } => "LINC-E3003",
            Self::CorruptArtifact { .. } => "LINC-E3004",
            Self::UnsupportedArtifact { .. } => "LINC-E3005",
            Self::ArtifactKindMismatch { .. } => "LINC-E3006",
            Self::TargetMismatch { .. } => "LINC-E3007",
            Self::InvalidSymbol { .. } => "LINC-E3008",
            Self::MissingProvider { .. } => "LINC-E3010",
            Self::AmbiguousProvider { .. } => "LINC-E3011",
            Self::DependencyCycle { .. } => "LINC-E3012",
            Self::UnsupportedInput { .. } => "LINC-E3013",
            Self::InvalidPolicy { .. } => "LINC-E3014",
            Self::ToolIdentity { .. } => "LINC-E3020",
            Self::ProbeTimeout { .. } => "LINC-E3030",
            Self::ProbeOutputLimit { .. } => "LINC-E3031",
            Self::ProbeNonzero { .. } => "LINC-E3032",
            Self::ProbeParserGap { .. } => "LINC-E3033",
            Self::ProbeUnsafeStreams => "LINC-E3037",
            Self::MissingCrossRunner { .. } => "LINC-E3034",
            Self::InvalidRunner { .. } => "LINC-E3035",
            Self::SymbolRejected { .. } => "LINC-E3040",
            Self::AbiMismatch { .. } => "LINC-E3041",
            Self::Contract(_) => "LINC-E3099",
        }
    }
}

pub(crate) fn io_error(
    operation: &'static str,
    path: impl Into<PathBuf>,
    source: io::Error,
) -> NativeError {
    NativeError::Io {
        operation,
        path: path.into(),
        source,
    }
}
