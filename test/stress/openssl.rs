use std::path::{Path, PathBuf};

use linc::{LincError, HeaderConfig, RawHeaderResult};

const HEADER_CANDIDATES: &[&str] = &["/usr/include/openssl/ssl.h", "/usr/include/x86_64-linux-gnu/openssl/ssl.h"];
const INCLUDE_DIR_CANDIDATES: &[&str] = &["/usr/include", "/usr/include/openssl", "/usr/include/x86_64-linux-gnu"];
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpensslEnvironment {
    pub header: PathBuf,
    pub include_dirs: Vec<PathBuf>,
}

pub fn openssl_environment() -> Result<OpensslEnvironment, LincError> {
    let header = HEADER_CANDIDATES
        .iter()
        .find(|path| Path::new(path).exists())
        .map(PathBuf::from)
        .ok_or_else(|| LincError::InvalidConfig {
            reason: "openssl example requires openssl headers".into(),
        })?;

    let include_dirs = INCLUDE_DIR_CANDIDATES
        .iter()
        .filter(|dir| Path::new(dir).exists())
        .map(PathBuf::from)
        .collect();

    Ok(OpensslEnvironment { header, include_dirs })
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
    openssl_header_config()?.process()
}
