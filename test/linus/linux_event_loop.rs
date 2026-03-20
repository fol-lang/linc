use std::path::{Path, PathBuf};

use linc::{LincError, HeaderConfig, RawHeaderResult};

const REQUIRED_HEADERS: &[&str] = &[
    "/usr/include/sys/epoll.h",
    "/usr/include/sys/timerfd.h",
    "/usr/include/sys/signalfd.h",
];
const MULTIARCH_HEADERS: &[&str] = &[
    "/usr/include/x86_64-linux-gnu/sys/epoll.h",
    "/usr/include/x86_64-linux-gnu/sys/timerfd.h",
    "/usr/include/x86_64-linux-gnu/sys/signalfd.h",
];
const INCLUDE_DIR_CANDIDATES: &[&str] = &["/usr/include", "/usr/include/x86_64-linux-gnu"];
const PROBE_TYPES: &[&str] = &["struct epoll_event", "struct signalfd_siginfo"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxEventLoopEnvironment {
    pub headers: Vec<PathBuf>,
    pub include_dirs: Vec<PathBuf>,
}

pub fn linux_event_loop_environment() -> Result<LinuxEventLoopEnvironment, LincError> {
    let header_candidates = if REQUIRED_HEADERS.iter().all(|path| Path::new(path).exists()) {
        REQUIRED_HEADERS
    } else if MULTIARCH_HEADERS.iter().all(|path| Path::new(path).exists()) {
        MULTIARCH_HEADERS
    } else {
        return Err(LincError::InvalidConfig {
            reason: "linux event-loop example requires epoll, timerfd, and signalfd headers".into(),
        });
    };

    let headers = header_candidates.iter().map(PathBuf::from).collect();
    let include_dirs = INCLUDE_DIR_CANDIDATES
        .iter()
        .filter(|dir| Path::new(dir).exists())
        .map(PathBuf::from)
        .collect();

    Ok(LinuxEventLoopEnvironment {
        headers,
        include_dirs,
    })
}

pub fn linux_event_loop_header_config() -> Result<HeaderConfig, LincError> {
    let environment = linux_event_loop_environment()?;
    let mut cfg = HeaderConfig::new()
        .target_constraint("linux")
        .link_lib("c")
        .no_origin_filter();

    for header in &environment.headers {
        cfg = cfg.entry_header(header);
    }
    for include_dir in &environment.include_dirs {
        cfg = cfg.include_dir(include_dir);
    }
    for probe_type in PROBE_TYPES {
        cfg = cfg.probe_type_layout(*probe_type);
    }

    Ok(cfg)
}

pub fn analyze_linux_event_loop() -> Result<RawHeaderResult, LincError> {
    linux_event_loop_header_config()?.process()
}
