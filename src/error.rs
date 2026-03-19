use std::fmt;
use std::path::PathBuf;

/// Crate-wide typed error surface.
///
/// Current state:
///
/// - JSON/schema transport already uses `BicError`
/// - several operational APIs still return `Result<_, String>`
///
/// Intended taxonomy:
///
/// - scan/configuration failures
/// - preprocessing and parse failures
/// - probe execution failures
/// - artifact inspection failures
/// - serialization/schema failures
///
/// Validation findings are intentionally *not* modeled as `BicError`.
/// They are returned as structured report data instead.
#[derive(Debug)]
pub enum BicError {
    /// A scan-like operation was invoked without any entry headers.
    NoHeaders,
    /// ABI probe execution failed before layouts could be produced.
    ProbeFailed { reason: String },
    /// A compiler/preprocessor invocation failed before a usable translation unit was produced.
    PreprocessorFailed { command: String, stderr: String },
    /// Source parsing failed after preprocessing.
    ParseFailed { source: String },
    /// An I/O failure occurred while reading or writing an input/output boundary.
    Io(std::io::Error),
    /// Serialization or deserialization failed.
    Serialization(String),
    /// Artifact inspection failed for a specific path.
    SymbolRead { path: PathBuf, reason: String },
    /// An artifact format was recognized as unsupported for the attempted operation.
    UnsupportedFormat { path: PathBuf, format: String },
    /// The serialized package uses a schema newer than this build supports.
    SchemaVersion { found: u32, supported: u32 },
}

impl fmt::Display for BicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BicError::NoHeaders => write!(f, "no entry headers specified"),
            BicError::ProbeFailed { reason } => {
                write!(f, "probe failed: {}", reason)
            }
            BicError::PreprocessorFailed { command, stderr } => {
                write!(f, "preprocessor '{}' failed: {}", command, stderr)
            }
            BicError::ParseFailed { source } => {
                write!(f, "parse error: {}", source)
            }
            BicError::Io(e) => write!(f, "I/O error: {}", e),
            BicError::Serialization(msg) => write!(f, "serialization error: {}", msg),
            BicError::SymbolRead { path, reason } => {
                write!(f, "failed to read symbols from {}: {}", path.display(), reason)
            }
            BicError::UnsupportedFormat { path, format } => {
                write!(f, "unsupported format '{}' for {}", format, path.display())
            }
            BicError::SchemaVersion { found, supported } => {
                write!(
                    f,
                    "unsupported schema version {} (this BIC supports up to {})",
                    found, supported
                )
            }
        }
    }
}

impl std::error::Error for BicError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BicError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for BicError {
    fn from(e: std::io::Error) -> Self {
        BicError::Io(e)
    }
}

impl From<serde_json::Error> for BicError {
    fn from(e: serde_json::Error) -> Self {
        BicError::Serialization(e.to_string())
    }
}

impl From<BicError> for String {
    fn from(e: BicError) -> String {
        e.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_no_headers() {
        let e = BicError::NoHeaders;
        assert_eq!(e.to_string(), "no entry headers specified");
    }

    #[test]
    fn error_display_preprocessor() {
        let e = BicError::PreprocessorFailed {
            command: "gcc".into(),
            stderr: "file not found".into(),
        };
        assert!(e.to_string().contains("gcc"));
    }

    #[test]
    fn error_display_probe_failed() {
        let e = BicError::ProbeFailed {
            reason: "compiler missing".into(),
        };
        assert!(e.to_string().contains("compiler missing"));
    }

    #[test]
    fn error_display_schema() {
        let e = BicError::SchemaVersion { found: 99, supported: 1 };
        assert!(e.to_string().contains("99"));
    }

    #[test]
    fn error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let bic_err: BicError = io_err.into();
        assert!(bic_err.to_string().contains("gone"));
    }

    #[test]
    fn error_to_string() {
        let e = BicError::ParseFailed { source: "bad".into() };
        let s: String = e.into();
        assert!(s.contains("bad"));
    }
}
