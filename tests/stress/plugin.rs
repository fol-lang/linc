use std::path::{Path, PathBuf};

use linc::{HeaderConfig, LincError, RawHeaderResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginAbiEnvironment {
    pub header: PathBuf,
}

pub fn plugin_abi_environment() -> Result<PluginAbiEnvironment, LincError> {
    let header = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("stress")
        .join("plugin_abi.h");

    if !header.exists() {
        return Err(LincError::InvalidConfig {
            reason: "plugin ABI example requires tests/stress/plugin_abi.h".into(),
        });
    }

    Ok(PluginAbiEnvironment { header })
}

pub fn plugin_abi_header_config() -> Result<HeaderConfig, LincError> {
    let environment = plugin_abi_environment()?;
    Ok(HeaderConfig::new()
        .entry_header(environment.header)
        .link_lib("dl")
        .no_origin_filter()
        .probe_type_layout("struct bic_plugin_message")
        .probe_type_layout("struct bic_plugin_descriptor"))
}

pub fn analyze_plugin_abi() -> Result<RawHeaderResult, LincError> {
    super::common::process(&plugin_abi_header_config()?)
}
