#[cfg(target_os = "linux")]
#[path = "../test/linus/socketcan.rs"]
mod socketcan;

#[cfg(target_os = "linux")]
#[test]
fn socketcan_example_environment_is_explicit() {
    if !socketcan::socketcan_headers_available() {
        return;
    }

    let environment = socketcan::socketcan_environment().unwrap();
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
    assert_eq!(config.binding_surface().entry_headers.len(), environment.required_headers.len() + environment.optional_headers.len());
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
    if std::env::var_os("BIC_RUN_SYSTEM_SOCKETCAN").is_none() {
        return;
    }
    if !socketcan::socketcan_headers_available() {
        return;
    }

    let result = socketcan::analyze_socketcan().unwrap();
    let package = &result.package;

    assert!(package.layouts.iter().any(|layout| layout.name == "struct can_frame" && layout.size > 0));
    assert!(package.layouts.iter().any(|layout| layout.name == "struct sockaddr_can" && layout.size > 0));
    assert!(package.macros.iter().any(|macro_binding| macro_binding.name == "CAN_EFF_FLAG"));
    assert!(result.report.preprocessed_source.contains("struct can_frame"));
    socketcan::socketcan_runtime_smoke_check().unwrap();
}
