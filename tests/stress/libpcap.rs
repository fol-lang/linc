use std::path::{Path, PathBuf};

use linc::{HeaderConfig, LincError, RawHeaderResult};

const HEADER_CANDIDATES: &[&str] = &["/usr/include/pcap/pcap.h", "/usr/include/pcap.h"];
const SUPPORT_HEADER_CANDIDATES: &[&str] = &[
    "/usr/include/sys/types.h",
    "/usr/include/x86_64-linux-gnu/sys/types.h",
];
const INCLUDE_DIR_CANDIDATES: &[&str] = &[
    "/usr/include",
    "/usr/include/pcap",
    "/usr/include/x86_64-linux-gnu",
];
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibpcapEnvironment {
    pub header: PathBuf,
    pub support_headers: Vec<PathBuf>,
    pub include_dirs: Vec<PathBuf>,
}

pub fn libpcap_environment() -> Result<LibpcapEnvironment, LincError> {
    let header = HEADER_CANDIDATES
        .iter()
        .find(|path| Path::new(path).exists())
        .map(PathBuf::from)
        .ok_or_else(|| LincError::InvalidConfig {
            reason: "libpcap example requires pcap headers".into(),
        })?;

    let include_dirs = INCLUDE_DIR_CANDIDATES
        .iter()
        .filter(|dir| Path::new(dir).exists())
        .map(PathBuf::from)
        .collect();
    let support_headers = SUPPORT_HEADER_CANDIDATES
        .iter()
        .filter(|path| Path::new(path).exists())
        .map(PathBuf::from)
        .collect();

    Ok(LibpcapEnvironment {
        header,
        support_headers,
        include_dirs,
    })
}

pub fn libpcap_header_config() -> Result<HeaderConfig, LincError> {
    let environment = libpcap_environment()?;
    let mut cfg = HeaderConfig::new().link_lib("pcap").no_origin_filter();

    for support_header in &environment.support_headers {
        cfg = cfg.entry_header(support_header);
    }
    cfg = cfg.entry_header(&environment.header);

    for include_dir in &environment.include_dirs {
        cfg = cfg.include_dir(include_dir);
    }

    Ok(cfg)
}

pub fn analyze_libpcap() -> Result<RawHeaderResult, LincError> {
    super::common::process(&libpcap_header_config()?)
}
