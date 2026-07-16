//! Strict schema-v2 transport codec.

use serde_json::Error as JsonError;
use thiserror::Error;

use super::{
    package::validate_internal,
    schema::{is_link_analysis_v2, LINK_ANALYSIS_SCHEMA_ID},
    wire::{
        analysis_fingerprint, LinkAnalysisEnvelope, LinkAnalysisPackageWire,
        RawLinkAnalysisEnvelope,
    },
    ContractError, LinkAnalysisFingerprint, LinkAnalysisPackage, SchemaHeader, LINK_ANALYSIS_KIND,
    LINK_ANALYSIS_SCHEMA_VERSION,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeLimits {
    pub max_bytes: usize,
    pub max_native_inputs: usize,
    pub max_inventories: usize,
    pub max_symbols: usize,
    pub max_abi_probes: usize,
    pub max_layouts: usize,
    pub max_declaration_evidence: usize,
    pub max_link_atoms: usize,
    pub max_diagnostics: usize,
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            max_bytes: 64 * 1024 * 1024,
            max_native_inputs: 1_000_000,
            max_inventories: 65_536,
            max_symbols: 4_000_000,
            max_abi_probes: 1_000_000,
            max_layouts: 1_000_000,
            max_declaration_evidence: 1_000_000,
            max_link_atoms: 1_000_000,
            max_diagnostics: 1_000_000,
        }
    }
}

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("link-analysis envelope is {actual} bytes; decoder limit is {maximum}")]
    ByteLimit { actual: usize, maximum: usize },
    #[error("link-analysis {resource} count {actual} exceeds decoder limit {maximum}")]
    ResourceLimit {
        resource: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("malformed link-analysis envelope: {0}")]
    Envelope(#[source] JsonError),
    #[error("unexpected artifact kind {found:?}; expected {LINK_ANALYSIS_KIND:?}")]
    Kind { found: String },
    #[error("unsupported schema id {found:?}; expected {LINK_ANALYSIS_SCHEMA_ID:?}")]
    SchemaId { found: String },
    #[error(
        "unsupported link-analysis schema version {found}; only version {LINK_ANALYSIS_SCHEMA_VERSION} is accepted"
    )]
    SchemaVersion { found: u32 },
    #[error("malformed schema-v2 link-analysis payload: {0}")]
    Payload(#[source] JsonError),
    #[error("unit variant {tag:?} contains fields beyond its discriminator")]
    UnitVariantShape { tag: String },
    #[error("payload schema header does not exactly match the envelope header")]
    PayloadSchema,
    #[error("link-analysis fingerprint differs between envelope and payload")]
    EnvelopeFingerprint,
    #[error("link-analysis contract violation: {0}")]
    Contract(#[source] ContractError),
    #[error("link-analysis fingerprint mismatch: stored {stored}, recomputed {recomputed}")]
    Fingerprint {
        stored: LinkAnalysisFingerprint,
        recomputed: LinkAnalysisFingerprint,
    },
}

#[derive(Debug, Error)]
pub enum EncodeError {
    #[error("link-analysis contract violation: {0}")]
    Contract(#[source] ContractError),
    #[error("link-analysis package does not carry the current schema-v2 header")]
    Schema,
    #[error("link-analysis fingerprint mismatch: stored {stored}, recomputed {recomputed}")]
    Fingerprint {
        stored: LinkAnalysisFingerprint,
        recomputed: LinkAnalysisFingerprint,
    },
    #[error("could not serialize schema-v2 link-analysis package: {0}")]
    Serialization(#[source] JsonError),
}

pub fn decode_link_analysis(bytes: &[u8]) -> Result<LinkAnalysisPackage, DecodeError> {
    decode_link_analysis_with_limits(bytes, DecodeLimits::default())
}

pub fn decode_link_analysis_with_limits(
    bytes: &[u8],
    limits: DecodeLimits,
) -> Result<LinkAnalysisPackage, DecodeError> {
    if bytes.len() > limits.max_bytes {
        return Err(DecodeError::ByteLimit {
            actual: bytes.len(),
            maximum: limits.max_bytes,
        });
    }

    // Keep payload bytes opaque until kind and schema have been accepted.
    let envelope: RawLinkAnalysisEnvelope =
        serde_json::from_slice(bytes).map_err(DecodeError::Envelope)?;
    validate_envelope_header(&envelope.kind, &envelope.schema)?;

    let preflight: serde_json::Value =
        serde_json::from_str(envelope.payload.get()).map_err(DecodeError::Payload)?;
    validate_unit_variant_shapes(&preflight)?;
    let wire: LinkAnalysisPackageWire =
        serde_json::from_value(preflight).map_err(DecodeError::Payload)?;
    if wire.schema != envelope.schema {
        return Err(DecodeError::PayloadSchema);
    }
    if wire.fingerprint != envelope.fingerprint {
        return Err(DecodeError::EnvelopeFingerprint);
    }
    wire.check_limits(limits)?;

    let package = wire.into_domain().map_err(DecodeError::Contract)?;
    let recomputed = analysis_fingerprint(&package).map_err(DecodeError::Contract)?;
    if package.fingerprint() != recomputed {
        return Err(DecodeError::Fingerprint {
            stored: package.fingerprint(),
            recomputed,
        });
    }
    Ok(package)
}

fn validate_unit_variant_shapes(value: &serde_json::Value) -> Result<(), DecodeError> {
    const KIND_UNITS: &[&str] = &[
        "explicit",
        "none",
        "group_start",
        "group_end",
        "probe_source",
        "output_artifact",
        "probe_executable",
    ];
    const STATE_UNITS: &[&str] = &[
        "unavailable",
        "not_executed",
        "not_required",
        "unresolved",
        "not_applicable",
        "unset",
    ];

    fn walk(
        value: &serde_json::Value,
        kind_units: &[&str],
        state_units: &[&str],
    ) -> Result<(), DecodeError> {
        match value {
            serde_json::Value::Object(object) => {
                for (discriminator, units) in [("kind", kind_units), ("state", state_units)] {
                    if let Some(tag) = object.get(discriminator).and_then(|value| value.as_str()) {
                        if units.contains(&tag) && object.len() != 1 {
                            return Err(DecodeError::UnitVariantShape {
                                tag: tag.to_owned(),
                            });
                        }
                    }
                }
                for nested in object.values() {
                    walk(nested, kind_units, state_units)?;
                }
            }
            serde_json::Value::Array(values) => {
                for nested in values {
                    walk(nested, kind_units, state_units)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    walk(value, KIND_UNITS, STATE_UNITS)?;
    if let Some(evidence) = value
        .get("declaration_evidence")
        .and_then(serde_json::Value::as_array)
    {
        for declaration in evidence {
            for field in ["layout", "callable_abi"] {
                let Some(object) = declaration
                    .get(field)
                    .and_then(serde_json::Value::as_object)
                else {
                    continue;
                };
                if object.get("state").and_then(|value| value.as_str()) == Some("missing")
                    && object.len() != 1
                {
                    return Err(DecodeError::UnitVariantShape {
                        tag: "missing".to_owned(),
                    });
                }
            }
        }
    }
    Ok(())
}

/// Produces the unique minified schema-v2 JSON representation.
pub fn encode_link_analysis(package: &LinkAnalysisPackage) -> Result<Vec<u8>, EncodeError> {
    if !is_link_analysis_v2(package.schema()) {
        return Err(EncodeError::Schema);
    }
    validate_internal(package).map_err(EncodeError::Contract)?;
    let recomputed = analysis_fingerprint(package).map_err(EncodeError::Contract)?;
    if package.fingerprint() != recomputed {
        return Err(EncodeError::Fingerprint {
            stored: package.fingerprint(),
            recomputed,
        });
    }
    serde_json::to_vec(&LinkAnalysisEnvelope {
        kind: LINK_ANALYSIS_KIND,
        schema: package.schema(),
        fingerprint: package.fingerprint(),
        payload: LinkAnalysisPackageWire::from_domain(package),
    })
    .map_err(EncodeError::Serialization)
}

fn validate_envelope_header(kind: &str, schema: &SchemaHeader) -> Result<(), DecodeError> {
    if kind != LINK_ANALYSIS_KIND {
        return Err(DecodeError::Kind {
            found: kind.to_owned(),
        });
    }
    if schema.id != LINK_ANALYSIS_SCHEMA_ID {
        return Err(DecodeError::SchemaId {
            found: schema.id.clone(),
        });
    }
    if schema.version != LINK_ANALYSIS_SCHEMA_VERSION {
        return Err(DecodeError::SchemaVersion {
            found: schema.version,
        });
    }
    Ok(())
}
