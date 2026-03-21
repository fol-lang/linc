use std::path::{Path, PathBuf};

use linc::{HeaderConfig, LincError, RawHeaderResult};

const HEADER_CANDIDATES: &[&str] = &[
    "/usr/include/curl/curl.h",
    "/usr/include/x86_64-linux-gnu/curl/curl.h",
];
const INCLUDE_DIR_CANDIDATES: &[&str] = &[
    "/usr/include",
    "/usr/include/curl",
    "/usr/include/x86_64-linux-gnu",
];
const PROBE_TYPES: &[&str] = &["struct curl_blob"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibcurlEnvironment {
    pub header: PathBuf,
    pub include_dirs: Vec<PathBuf>,
}

pub fn libcurl_environment() -> Result<LibcurlEnvironment, LincError> {
    let header = HEADER_CANDIDATES
        .iter()
        .find(|path| Path::new(path).exists())
        .map(PathBuf::from)
        .ok_or_else(|| LincError::InvalidConfig {
            reason: "libcurl example requires curl headers".into(),
        })?;

    let include_dirs = INCLUDE_DIR_CANDIDATES
        .iter()
        .filter(|dir| Path::new(dir).exists())
        .map(PathBuf::from)
        .collect();

    Ok(LibcurlEnvironment {
        header,
        include_dirs,
    })
}

pub fn libcurl_header_config() -> Result<HeaderConfig, LincError> {
    let environment = libcurl_environment()?;
    let mut cfg = HeaderConfig::new()
        .entry_header(&environment.header)
        .link_lib("curl")
        .no_origin_filter();

    for include_dir in &environment.include_dirs {
        cfg = cfg.include_dir(include_dir);
    }
    for probe_type in PROBE_TYPES {
        cfg = cfg.probe_type_layout(*probe_type);
    }

    Ok(cfg)
}

pub fn analyze_libcurl() -> Result<RawHeaderResult, LincError> {
    super::common::process(&libcurl_header_config()?)
}
