mod common;
#[cfg(target_os = "linux")]
#[path = "linus/epoll.rs"]
mod epoll;

#[cfg(target_os = "linux")]
#[path = "linus/linux_event_loop.rs"]
mod linux_event_loop;

#[cfg(target_os = "linux")]
#[path = "linus/socketcan.rs"]
mod socketcan;

#[cfg(target_os = "linux")]
#[test]
#[ignore = "system prerequisite: Linux event-loop development headers"]
fn linux_event_loop_example_combines_multiple_system_headers() {
    eprintln!("RUN: Linux event-loop system evidence");
    let environment = linux_event_loop::linux_event_loop_environment()
        .expect("FAIL: epoll, timerfd, and signalfd headers are required");

    let config = linux_event_loop::linux_event_loop_header_config().unwrap();
    let result = linux_event_loop::analyze_linux_event_loop().unwrap();

    assert_eq!(
        config.binding_surface().entry_headers.len(),
        environment.headers.len()
    );
    assert!(environment
        .headers
        .iter()
        .any(|path| path.ends_with("sys/epoll.h")));
    assert!(environment
        .headers
        .iter()
        .any(|path| path.ends_with("sys/timerfd.h")));
    assert!(environment
        .headers
        .iter()
        .any(|path| path.ends_with("sys/signalfd.h")));
    assert!(result
        .report
        .preprocessed_source
        .contains("signalfd_siginfo"));
    assert!(result.report.preprocessed_source.contains("epoll_event"));
    assert!(result
        .package
        .layouts
        .iter()
        .any(|layout| layout.name == "struct epoll_event" && layout.size > 0));
    assert!(result
        .package
        .layouts
        .iter()
        .any(|layout| layout.name == "struct signalfd_siginfo" && layout.size > 0));
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "system prerequisite: Linux event-loop development headers"]
fn linux_event_loop_example_is_deterministic_when_available() {
    eprintln!("RUN: Linux event-loop determinism evidence");
    linux_event_loop::linux_event_loop_environment()
        .expect("FAIL: epoll, timerfd, and signalfd headers are required");

    let make = || {
        let result =
            linux_event_loop::analyze_linux_event_loop().expect("linux event-loop analysis");
        serde_json::to_string(&result.package).expect("linux event-loop package json")
    };

    assert_eq!(make(), make());
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "system prerequisite: host C compiler"]
fn epoll_example_is_code_driven_and_consumable() {
    eprintln!("RUN: host C compiler epoll analysis evidence");
    let environment = epoll::epoll_environment()
        .expect("repo epoll fixture or system epoll header must be available");

    let config = epoll::epoll_header_config().unwrap();
    let result = epoll::analyze_epoll().unwrap();

    assert!(
        environment.header.ends_with("sys/epoll.h")
            || environment.header.ends_with("epoll_fixture.h")
    );
    assert!(config
        .linking()
        .link_libraries
        .iter()
        .any(|library| library.name == "c"));
    assert!(config
        .probing()
        .probe_types
        .iter()
        .any(|probe_type| probe_type == "struct epoll_event"));
    if environment.is_fixture {
        assert!(result
            .report
            .preprocessed_source
            .contains("BIC_EPOLL_FIXTURE_H"));
    }
    assert!(result
        .package
        .layouts
        .iter()
        .any(|layout| layout.name == "struct epoll_event" && layout.size > 0));
    assert!(result.report.preprocessed_source.contains("epoll_event"));
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "system prerequisite: Linux SocketCAN development headers"]
fn socketcan_example_environment_is_explicit() {
    eprintln!("RUN: SocketCAN environment evidence");
    let environment =
        socketcan::socketcan_environment().expect("FAIL: Linux SocketCAN headers are required");
    let config = socketcan::socketcan_header_config().unwrap();

    assert!(environment
        .required_headers
        .iter()
        .any(|path| path.ends_with("linux/can.h")));
    assert!(environment
        .required_headers
        .iter()
        .any(|path| path.ends_with("linux/can/raw.h")));
    assert!(!environment.include_dirs.is_empty());
    assert_eq!(
        config.binding_surface().entry_headers.len(),
        environment.required_headers.len() + environment.optional_headers.len()
    );
    assert!(config
        .linking()
        .link_libraries
        .iter()
        .any(|library| library.name == "c"));
    assert!(config
        .linking()
        .platform_constraints
        .iter()
        .any(|constraint| constraint == "linux"));
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "system Linux header example"]
fn socketcan_example_is_code_driven_and_consumable() {
    eprintln!("RUN: SocketCAN compile and runtime evidence");
    socketcan::socketcan_environment().expect("FAIL: Linux SocketCAN headers are required");

    let result = socketcan::analyze_socketcan().unwrap();
    let package = &result.package;

    assert!(package
        .layouts
        .iter()
        .any(|layout| layout.name == "struct can_frame" && layout.size > 0));
    assert!(package
        .layouts
        .iter()
        .any(|layout| layout.name == "struct sockaddr_can" && layout.size > 0));
    assert!(package
        .macros
        .iter()
        .any(|macro_binding| macro_binding.name == "CAN_EFF_FLAG"));
    assert!(result
        .report
        .preprocessed_source
        .contains("struct can_frame"));
    socketcan::socketcan_runtime_smoke_check().unwrap();
}
