use std::{fmt, str::FromStr};

use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IdentityParseError {
    #[error("expected identity prefix {expected:?}")]
    Prefix { expected: &'static str },
    #[error("identity must contain exactly 64 lowercase hexadecimal digits")]
    Shape,
}

fn digest(domain: &str, fields: &[&[u8]]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hash_field(&mut hasher, domain.as_bytes());
    for field in fields {
        hash_field(&mut hasher, field);
    }
    *hasher.finalize().as_bytes()
}

fn hash_field(hasher: &mut blake3::Hasher, field: &[u8]) {
    hasher.update(&(field.len() as u64).to_le_bytes());
    hasher.update(field);
}

fn encode_hex(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 0x0f) as usize] as char);
    }
    result
}

fn parse_digest(value: &str, prefix: &'static str) -> Result<[u8; 32], IdentityParseError> {
    let Some(hex) = value.strip_prefix(prefix) else {
        return Err(IdentityParseError::Prefix { expected: prefix });
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(IdentityParseError::Shape);
    }

    let mut bytes = [0_u8; 32];
    for (index, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (decode_nibble(pair[0]) << 4) | decode_nibble(pair[1]);
    }
    Ok(bytes)
}

fn decode_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("identity shape was checked before decoding"),
    }
}

macro_rules! identity_type {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}{}", $prefix, encode_hex(&self.0))
            }
        }

        impl FromStr for $name {
            type Err = IdentityParseError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                parse_digest(value, $prefix).map(Self)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                value.parse().map_err(D::Error::custom)
            }
        }
    };
}

identity_type!(ArtifactFingerprint, "lartifact1_");
identity_type!(ProviderId, "lprovider1_");
identity_type!(ProbeEvidenceId, "lprobe1_");
identity_type!(LinkAnalysisFingerprint, "lanalysis2_");

impl ArtifactFingerprint {
    pub fn from_content(content: &[u8]) -> Self {
        Self::from_bytes(digest("follang.linc.artifact-fingerprint.v1", &[content]))
    }
}

impl ProviderId {
    pub(crate) fn derive(
        artifact: ArtifactFingerprint,
        kind: &[u8],
        path_platform: &[u8],
        canonical_path_units: &[u8],
    ) -> Self {
        Self::from_bytes(digest(
            "follang.linc.provider-id.v1",
            &[
                artifact.as_bytes(),
                kind,
                path_platform,
                canonical_path_units,
            ],
        ))
    }
}

impl ProbeEvidenceId {
    pub(crate) fn derive(fields: &[Vec<u8>]) -> Self {
        let fields = fields.iter().map(Vec::as_slice).collect::<Vec<_>>();
        Self::from_bytes(digest("follang.linc.probe-evidence-id.v1", &fields))
    }
}

impl LinkAnalysisFingerprint {
    pub(crate) fn derive(canonical_payload: &[u8]) -> Self {
        Self::from_bytes(digest(
            "follang.linc.link-analysis-fingerprint.v2",
            &[canonical_payload],
        ))
    }
}
