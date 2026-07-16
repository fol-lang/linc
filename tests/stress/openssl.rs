use std::path::PathBuf;

use linc::raw_headers::{HeaderConfig, RawHeaderResult};
use linc::LincError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpensslEnvironment {
    pub header: PathBuf,
    pub include_dirs: Vec<PathBuf>,
}

pub fn openssl_environment() -> Result<OpensslEnvironment, LincError> {
    let header = super::common::find_system_header("openssl/ssl.h").ok_or_else(|| {
        LincError::InvalidConfig {
            reason: "openssl example requires openssl headers".into(),
        }
    })?;

    let include_dirs = super::common::system_include_dirs();

    Ok(OpensslEnvironment {
        header,
        include_dirs,
    })
}

pub fn openssl_header_config() -> Result<HeaderConfig, LincError> {
    let environment = openssl_environment()?;
    let mut cfg = HeaderConfig::new()
        .entry_header(&environment.header)
        .link_lib("ssl")
        .link_lib("crypto")
        .no_origin_filter();

    for include_dir in &environment.include_dirs {
        cfg = cfg.include_dir(include_dir);
    }

    Ok(cfg)
}

pub fn analyze_openssl() -> Result<RawHeaderResult, LincError> {
    super::common::process(&openssl_header_config()?)
}
