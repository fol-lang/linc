#[cfg(target_os = "linux")]
#[path = "../test/linus/socketcan.rs"]
mod socketcan;

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
