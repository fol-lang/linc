use std::path::{Path, PathBuf};
use std::{io, os::raw::c_int};

use bic::{BicError, HeaderConfig, RawHeaderResult};

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

unsafe extern "C" {
    fn socket(domain: c_int, kind: c_int, protocol: c_int) -> c_int;
    fn close(fd: c_int) -> c_int;
}

pub fn socketcan_headers_available() -> bool {
    SOCKETCAN_HEADERS.iter().all(|path| Path::new(path).exists())
}

pub fn socketcan_header_config() -> Result<HeaderConfig, BicError> {
    if !socketcan_headers_available() {
        return Err(BicError::InvalidConfig {
            reason: "socketcan example requires Linux SocketCAN headers".into(),
        });
    }

    let mut cfg = HeaderConfig::new()
        .target_constraint("linux")
        .link_lib("c")
        .no_origin_filter();

    for path in SOCKETCAN_HEADERS {
        cfg = cfg.entry_header(PathBuf::from(path));
    }
    for path in OPTIONAL_HEADERS {
        if Path::new(path).exists() {
            cfg = cfg.entry_header(PathBuf::from(path));
        }
    }
    for dir in INCLUDE_DIR_CANDIDATES {
        if Path::new(dir).exists() {
            cfg = cfg.include_dir(PathBuf::from(dir));
        }
    }
    for probe_type in SOCKETCAN_PROBE_TYPES {
        cfg = cfg.probe_type_layout(*probe_type);
    }

    Ok(cfg)
}

pub fn analyze_socketcan() -> Result<RawHeaderResult, BicError> {
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
