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

use std::path::Path;

#[test]
#[ignore = "system prerequisite: host C compiler"]
fn zlib_vendored_example_is_code_driven_and_consumable() {
    eprintln!("RUN: host C compiler vendored-zlib analysis evidence");
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
#[ignore = "system prerequisite: libpcap development headers"]
fn libpcap_example_is_code_driven_and_consumable() {
    eprintln!("RUN: libpcap system evidence");
    let environment =
        libpcap::libpcap_environment().expect("FAIL: libpcap development headers are required");

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
#[ignore = "system prerequisite: libcurl development headers"]
fn libcurl_example_is_code_driven_and_consumable() {
    eprintln!("RUN: libcurl system evidence");
    let environment =
        libcurl::libcurl_environment().expect("FAIL: libcurl development headers are required");

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
#[ignore = "system prerequisite: OpenSSL development headers"]
fn openssl_example_is_code_driven_and_consumable() {
    eprintln!("RUN: OpenSSL system evidence");
    let environment =
        openssl::openssl_environment().expect("FAIL: OpenSSL development headers are required");

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
#[ignore = "system prerequisite: OpenSSL development headers"]
fn openssl_example_is_deterministic_when_available() {
    eprintln!("RUN: OpenSSL determinism evidence");
    openssl::openssl_environment().expect("FAIL: OpenSSL development headers are required");

    let make = || {
        let result = openssl::analyze_openssl().expect("openssl analysis");
        serde_json::to_string(&result.package).expect("openssl package json")
    };

    assert_eq!(make(), make());
}

#[test]
#[ignore = "system prerequisite: OpenSSL development headers"]
fn openssl_example_resolves_expected_link_surface_when_available() {
    eprintln!("RUN: OpenSSL link-surface evidence");
    openssl::openssl_environment().expect("FAIL: OpenSSL development headers are required");

    let result = openssl::analyze_openssl().expect("openssl analysis");
    let plan = linc::resolve_link_plan(&result.package);

    assert!(plan.inputs.iter().any(
        |input| matches!(input, linc::ir::LinkInput::Library(library) if library.name == "ssl")
    ));
    assert!(plan.inputs.iter().any(
        |input| matches!(input, linc::ir::LinkInput::Library(library) if library.name == "crypto")
    ));
}

#[test]
#[ignore = "system prerequisite: host C preprocessor"]
fn libpng_vendored_example_is_code_driven_and_consumable() {
    eprintln!("RUN: host C preprocessor vendored-libpng analysis evidence");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/full_apps/external/libpng/header");
    let include_dir = root.join("include");
    let entry = root.join("main.c");

    let result = common::process(
        &linc::raw_headers::HeaderConfig::new()
            .header(&entry)
            .include_dir(&include_dir)
            .link_lib("png")
            .no_origin_filter(),
    )
    .unwrap();
    let plan = linc::resolve_link_plan(&result.package);

    assert!(result
        .package
        .find_function("png_create_read_struct")
        .is_some());
    assert!(result.package.find_function("png_init_io").is_some());
    assert!(result.package.find_type_alias("png_structp").is_some());
    assert!(result
        .package
        .macros
        .iter()
        .any(|macro_binding| macro_binding.name == "PNG_LIBPNG_VER_STRING"));
    assert!(plan.inputs.iter().any(
        |input| matches!(input, linc::ir::LinkInput::Library(library) if library.name == "png")
    ));
}

#[test]
#[ignore = "system prerequisite: host C preprocessor"]
fn libpng_vendored_example_is_deterministic() {
    eprintln!("RUN: host C preprocessor vendored-libpng determinism evidence");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/full_apps/external/libpng/header");
    let include_dir = root.join("include");
    let entry = root.join("main.c");

    let make = || {
        let result = common::process(
            &linc::raw_headers::HeaderConfig::new()
                .header(&entry)
                .include_dir(&include_dir)
                .link_lib("png")
                .no_origin_filter(),
        )
        .expect("libpng analysis");
        serde_json::to_string(&result.package).expect("libpng package json")
    };

    assert_eq!(make(), make());
}

#[test]
#[ignore = "system prerequisite: host C preprocessor"]
fn libpng_vendored_example_resolves_expected_link_surface() {
    eprintln!("RUN: host C preprocessor vendored-libpng link-surface evidence");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/full_apps/external/libpng/header");
    let include_dir = root.join("include");
    let entry = root.join("main.c");

    let result = common::process(
        &linc::raw_headers::HeaderConfig::new()
            .header(&entry)
            .include_dir(&include_dir)
            .link_lib("png")
            .no_origin_filter(),
    )
    .unwrap();
    let plan = linc::resolve_link_plan(&result.package);

    assert!(plan.inputs.iter().any(
        |input| matches!(input, linc::ir::LinkInput::Library(library) if library.name == "png")
    ));
}

#[test]
#[ignore = "system prerequisite: host C compiler"]
fn plugin_abi_example_is_code_driven_and_consumable() {
    eprintln!("RUN: host C compiler plugin-ABI analysis evidence");
    let environment = plugin::plugin_abi_environment().unwrap();
    let config = plugin::plugin_abi_header_config().unwrap();
    let result = plugin::analyze_plugin_abi().unwrap();
    let analysis = linc::LinkAnalysisPackage::from(result.package.clone());

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
    assert!(analysis.runtime_boundaries.iter().any(|boundary| {
        boundary.kind == linc::RuntimeBoundaryKind::DynamicLoader && boundary.trigger == "dl"
    }));
}
