use std::path::{Path, PathBuf};

use linc::{LincError, HeaderConfig, RawHeaderResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZlibVendoredEnvironment {
    pub include_dir: PathBuf,
    pub entry_header: PathBuf,
}

pub fn zlib_vendored_environment() -> Result<ZlibVendoredEnvironment, LincError> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("full_apps")
        .join("external")
        .join("zlib")
        .join("header");
    let include_dir = root.join("include");
    let entry_header = include_dir.join("zlib.h");

    if !include_dir.exists() || !entry_header.exists() {
        return Err(LincError::InvalidConfig {
            reason: "vendored zlib example requires the test corpus headers".into(),
        });
    }

    Ok(ZlibVendoredEnvironment {
        include_dir,
        entry_header,
    })
}

pub fn zlib_vendored_header_config() -> Result<HeaderConfig, LincError> {
    let environment = zlib_vendored_environment()?;
    Ok(HeaderConfig::new()
        .entry_header(environment.entry_header)
        .include_dir(environment.include_dir)
        .no_origin_filter()
        .probe_type_layout("z_stream"))
}

pub fn analyze_zlib_vendored() -> Result<RawHeaderResult, LincError> {
    zlib_vendored_header_config()?.process()
}
