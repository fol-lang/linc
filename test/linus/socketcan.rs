use std::path::{Path, PathBuf};
use std::{io, os::raw::c_int};

use linc::{LincError, HeaderConfig, RawHeaderResult};

const SOCKETCAN_HEADERS: &[&str] = &["/usr/include/linux/can.h", "/usr/include/linux/can/raw.h"];
const OPTIONAL_HEADERS: &[&str] = &[
    "/usr/include/net/if.h",
    "/usr/include/x86_64-linux-gnu/sys/socket.h",
    "/usr/include/sys/socket.h",
];
const INCLUDE_DIR_CANDIDATES: &[&str] = &["/usr/include", "/usr/include/x86_64-linux-gnu"];
const SOCKETCAN_PROBE_TYPES: &[&str] =
    &["struct can_frame", "struct canfd_frame", "struct sockaddr_can"];
const AF_CAN: c_int = 29;
const SOCK_RAW: c_int = 3;
const CAN_RAW: c_int = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocketcanEnvironment {
    pub required_headers: Vec<PathBuf>,
    pub optional_headers: Vec<PathBuf>,
    pub include_dirs: Vec<PathBuf>,
}

unsafe extern "C" {
    fn socket(domain: c_int, kind: c_int, protocol: c_int) -> c_int;
    fn close(fd: c_int) -> c_int;
}

pub fn socketcan_headers_available() -> bool {
    SOCKETCAN_HEADERS.iter().all(|path| Path::new(path).exists())
}

pub fn socketcan_environment() -> Result<SocketcanEnvironment, LincError> {
    if !socketcan_headers_available() {
        return Err(LincError::InvalidConfig {
            reason: "socketcan example requires Linux SocketCAN headers".into(),
        });
    }

    let required_headers = SOCKETCAN_HEADERS.iter().map(PathBuf::from).collect();
    let optional_headers = OPTIONAL_HEADERS
        .iter()
        .filter(|path| Path::new(path).exists())
        .map(PathBuf::from)
        .collect();
    let include_dirs = INCLUDE_DIR_CANDIDATES
        .iter()
        .filter(|dir| Path::new(dir).exists())
        .map(PathBuf::from)
        .collect();

    Ok(SocketcanEnvironment {
        required_headers,
        optional_headers,
        include_dirs,
    })
}

pub fn socketcan_header_config() -> Result<HeaderConfig, LincError> {
    let environment = socketcan_environment()?;

    let mut cfg = HeaderConfig::new()
        .target_constraint("linux")
        .link_lib("c")
        .no_origin_filter();

    for path in &environment.required_headers {
        cfg = cfg.entry_header(path);
    }
    for path in &environment.optional_headers {
        cfg = cfg.entry_header(path);
    }
    for dir in &environment.include_dirs {
        cfg = cfg.include_dir(dir);
    }
    for probe_type in SOCKETCAN_PROBE_TYPES {
        cfg = cfg.probe_type_layout(*probe_type);
    }

    Ok(cfg)
}

pub fn analyze_socketcan() -> Result<RawHeaderResult, LincError> {
    socketcan_header_config()?.process()
}

pub fn socketcan_runtime_smoke_check() -> io::Result<()> {
    let fd = unsafe { socket(AF_CAN, SOCK_RAW, CAN_RAW) };
    if fd >= 0 {
        let _ = unsafe { close(fd) };
        return Ok(());
    }

    let err = io::Error::last_os_error();
    match err.raw_os_error() {
        Some(93 | 94 | 97) => Ok(()),
        _ => Err(err),
    }
}
