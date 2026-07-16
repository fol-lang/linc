//! Checked native-link and ABI-evidence contracts for the
//! `PARC -> LINC -> GERC` pipeline.
//!
//! [`contract`] is the sole public API. It provides strict schema-v2 transport,
//! immutable link-analysis packages, complete-source validation, lossless
//! native paths and link plans, symbol inventories, layout and callable-ABI
//! evidence, probe provenance, policy, diagnostics, and the preservation
//! corpus shared with downstream consumers.

#![forbid(unsafe_code)]

pub mod contract;

/// Optional operational implementation which produces the canonical contract
/// types. The contract remains available without native parsing or execution
/// dependencies.
#[cfg(feature = "native-inspection")]
pub mod native;
