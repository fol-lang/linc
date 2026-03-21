use std::path::{Path, PathBuf};

use linc::{HeaderConfig, LincError, RawHeaderResult};

const EPOLL_HEADER_CANDIDATES: &[&str] = &[
    "/usr/include/sys/epoll.h",
    "/usr/include/x86_64-linux-gnu/sys/epoll.h",
];
const INCLUDE_DIR_CANDIDATES: &[&str] = &["/usr/include", "/usr/include/x86_64-linux-gnu"];
const EPOLL_PROBE_TYPES: &[&str] = &["struct epoll_event"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpollEnvironment {
    pub header: PathBuf,
    pub include_dirs: Vec<PathBuf>,
    pub is_fixture: bool,
}

pub fn epoll_environment() -> Result<EpollEnvironment, LincError> {
    let system_header = EPOLL_HEADER_CANDIDATES
        .iter()
        .find(|path| Path::new(path).exists())
        .map(PathBuf::from);
    let fixture_header =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/linus/epoll_fixture.h");
    let (header, is_fixture) = if let Some(header) = system_header {
        (header, false)
    } else if fixture_header.exists() {
        (fixture_header, true)
    } else {
        return Err(LincError::InvalidConfig {
            reason: "epoll example requires a sys/epoll.h header or repo fixture".into(),
        });
    };

    let include_dirs = INCLUDE_DIR_CANDIDATES
        .iter()
        .filter(|dir| Path::new(dir).exists())
        .map(PathBuf::from)
        .collect();

    Ok(EpollEnvironment {
        header,
        include_dirs,
        is_fixture,
    })
}

pub fn epoll_header_config() -> Result<HeaderConfig, LincError> {
    let environment = epoll_environment()?;
    let mut cfg = HeaderConfig::new()
        .target_constraint("linux")
        .link_lib("c")
        .no_origin_filter()
        .entry_header(&environment.header);

    for include_dir in &environment.include_dirs {
        cfg = cfg.include_dir(include_dir);
    }
    for probe_type in EPOLL_PROBE_TYPES {
        cfg = cfg.probe_type_layout(*probe_type);
    }

    Ok(cfg)
}

pub fn analyze_epoll() -> Result<RawHeaderResult, LincError> {
    super::common::process(&epoll_header_config()?)
}
