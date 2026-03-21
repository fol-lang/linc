mod common;
#[path = "stress/libcurl.rs"]
mod libcurl;
#[path = "stress/libpcap.rs"]
mod libpcap;
#[path = "stress/openssl.rs"]
mod openssl;
#[path = "stress/plugin.rs"]
mod plugin;
#[path = "stress/zlib.rs"]
mod zlib;

#[test]
fn zlib_vendored_example_is_code_driven_and_consumable() {
    let environment = zlib::zlib_vendored_environment().unwrap();
    let config = zlib::zlib_vendored_header_config().unwrap();
    let result = zlib::analyze_zlib_vendored().unwrap();

    assert!(environment.include_dir.ends_with("zlib/header/include"));
    assert!(environment
        .entry_header
        .ends_with("zlib/header/include/zlib.h"));
    assert_eq!(config.binding_surface().entry_headers.len(), 1);
    assert_eq!(config.preprocessing().include_dirs.len(), 1);
    assert!(result.package.find_function("deflate").is_some());
    assert!(result.package.find_function("inflate").is_some());
    assert!(result
        .package
        .layouts
        .iter()
        .any(|layout| layout.name == "z_stream" && layout.size > 0));
}

#[test]
fn libpcap_example_is_code_driven_and_consumable() {
    let Ok(environment) = libpcap::libpcap_environment() else {
        return;
    };

    let config = libpcap::libpcap_header_config().unwrap();
    let result = libpcap::analyze_libpcap().unwrap();

    assert!(environment.header.ends_with("pcap.h") || environment.header.ends_with("pcap/pcap.h"));
    assert!(!environment.support_headers.is_empty());
    assert!(config
        .linking()
        .link_libraries
        .iter()
        .any(|library| library.name == "pcap"));
    assert!(result.package.find_function("pcap_open_live").is_some());
    assert!(result.package.find_function("pcap_loop").is_some());
    assert!(result.package.find_record("pcap_pkthdr").is_some());
    assert!(result.package.find_type_alias("pcap_handler").is_some());
}

#[test]
fn libcurl_example_is_code_driven_and_consumable() {
    let Ok(environment) = libcurl::libcurl_environment() else {
        return;
    };

    let config = libcurl::libcurl_header_config().unwrap();
    let result = libcurl::analyze_libcurl().unwrap();

    assert!(environment.header.ends_with("curl.h"));
    assert!(config
        .linking()
        .link_libraries
        .iter()
        .any(|library| library.name == "curl"));
    assert!(result.package.find_function("curl_easy_init").is_some());
    assert!(result.package.find_function("curl_easy_setopt").is_some());
    assert!(result.package.find_enum("curl_khtype").is_some());
    assert!(result
        .package
        .macros
        .iter()
        .any(|macro_binding| macro_binding.name == "CURL_VERSION_BITS"));
    assert!(result
        .package
        .layouts
        .iter()
        .any(|layout| layout.name == "struct curl_blob" && layout.size > 0));
}

#[test]
fn openssl_example_is_code_driven_and_consumable() {
    let Ok(environment) = openssl::openssl_environment() else {
        return;
    };

    let config = openssl::openssl_header_config().unwrap();
    let result = openssl::analyze_openssl().unwrap();

    assert!(environment.header.ends_with("ssl.h"));
    assert!(config
        .linking()
        .link_libraries
        .iter()
        .any(|library| library.name == "ssl"));
    assert!(config
        .linking()
        .link_libraries
        .iter()
        .any(|library| library.name == "crypto"));
    assert!(result.package.find_function("SSL_new").is_some());
    assert!(result.package.find_function("SSL_CTX_new").is_some());
    assert!(result.package.find_type_alias("SSL").is_some());
    assert!(result.package.find_type_alias("SSL_CTX").is_some());
    assert!(result
        .package
        .macros
        .iter()
        .any(|macro_binding| macro_binding.name == "OPENSSL_VERSION_NUMBER"));
    assert!(
        result.package.layouts.is_empty()
            || result
                .package
                .layouts
                .iter()
                .all(|layout| layout.name != "struct ssl_st")
    );
}

#[test]
fn plugin_abi_example_is_code_driven_and_consumable() {
    let environment = plugin::plugin_abi_environment().unwrap();
    let config = plugin::plugin_abi_header_config().unwrap();
    let result = plugin::analyze_plugin_abi().unwrap();

    assert!(environment.header.ends_with("tests/stress/plugin_abi.h"));
    assert!(config
        .linking()
        .link_libraries
        .iter()
        .any(|library| library.name == "dl"));
    assert!(result
        .package
        .find_function("bic_plugin_descriptor_v1")
        .is_some());
    assert!(result
        .package
        .find_record("bic_plugin_descriptor")
        .is_some());
    assert!(result
        .package
        .find_type_alias("bic_plugin_log_fn")
        .is_some());
    assert!(result
        .package
        .layouts
        .iter()
        .any(|layout| layout.name == "struct bic_plugin_descriptor" && layout.size > 0));
}
