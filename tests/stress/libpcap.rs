use std::path::PathBuf;

use linc::raw_headers::{HeaderConfig, RawHeaderResult};
use linc::LincError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibpcapEnvironment {
    pub header: PathBuf,
    pub support_headers: Vec<PathBuf>,
    pub include_dirs: Vec<PathBuf>,
}

pub fn libpcap_environment() -> Result<LibpcapEnvironment, LincError> {
    let header = super::common::find_system_header("pcap/pcap.h")
        .or_else(|| super::common::find_system_header("pcap.h"))
        .ok_or_else(|| LincError::InvalidConfig {
            reason: "libpcap example requires pcap headers".into(),
        })?;

    let include_dirs = super::common::system_include_dirs();
    let support_headers = super::common::find_system_header("sys/types.h")
        .into_iter()
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
