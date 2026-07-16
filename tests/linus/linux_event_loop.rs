use std::path::PathBuf;

use linc::raw_headers::{HeaderConfig, RawHeaderResult};
use linc::LincError;

const REQUIRED_HEADERS: &[&str] = &["sys/epoll.h", "sys/timerfd.h", "sys/signalfd.h"];
const PROBE_TYPES: &[&str] = &["struct epoll_event", "struct signalfd_siginfo"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxEventLoopEnvironment {
    pub headers: Vec<PathBuf>,
    pub include_dirs: Vec<PathBuf>,
}

pub fn linux_event_loop_environment() -> Result<LinuxEventLoopEnvironment, LincError> {
    let headers = REQUIRED_HEADERS
        .iter()
        .map(super::common::find_system_header)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| LincError::InvalidConfig {
            reason: "linux event-loop example requires epoll, timerfd, and signalfd headers".into(),
        })?;
    let include_dirs = super::common::system_include_dirs();

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
    super::common::process(&linux_event_loop_header_config()?)
}
