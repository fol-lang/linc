use std::path::{Path, PathBuf};

use bic::{BicError, HeaderConfig, RawHeaderResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginAbiEnvironment {
    pub header: PathBuf,
}

pub fn plugin_abi_environment() -> Result<PluginAbiEnvironment, BicError> {
    let header = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test")
        .join("stress")
        .join("plugin_abi.h");

    if !header.exists() {
        return Err(BicError::InvalidConfig {
            reason: "plugin ABI example requires test/stress/plugin_abi.h".into(),
        });
    }

    Ok(PluginAbiEnvironment { header })
}

pub fn plugin_abi_header_config() -> Result<HeaderConfig, BicError> {
    let environment = plugin_abi_environment()?;
    Ok(HeaderConfig::new()
        .entry_header(environment.header)
        .link_lib("dl")
        .no_origin_filter()
        .probe_type_layout("struct bic_plugin_message")
        .probe_type_layout("struct bic_plugin_descriptor"))
}

pub fn analyze_plugin_abi() -> Result<RawHeaderResult, BicError> {
    plugin_abi_header_config()?.process()
}
