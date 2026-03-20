use std::path::{Path, PathBuf};

use bic::{BicError, HeaderConfig, RawHeaderResult, SymbolInventory};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaxPainEnvironment {
    pub header: PathBuf,
    pub root_dir: PathBuf,
}

pub fn max_pain_environment() -> Result<MaxPainEnvironment, BicError> {
    let root_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test")
        .join("stress")
        .join("daemon");
    let header = root_dir.join("max_pain.h");

    if !header.exists() {
        return Err(BicError::InvalidConfig {
            reason: "combined daemon example requires test/stress/daemon/max_pain.h".into(),
        });
    }

    Ok(MaxPainEnvironment { header, root_dir })
}

pub fn max_pain_header_config() -> Result<HeaderConfig, BicError> {
    let environment = max_pain_environment()?;
    Ok(HeaderConfig::new()
        .entry_header(environment.header)
        .include_dir(environment.root_dir)
        .link_lib("dl")
        .no_origin_filter()
        .probe_type_layout("struct bic_daemon_packet")
        .probe_type_layout("struct bic_daemon_config"))
}

pub fn analyze_max_pain() -> Result<RawHeaderResult, BicError> {
    max_pain_header_config()?.process()
}

pub fn daemon_core_inventory_fixture() -> SymbolInventory {
    serde_json::from_str(include_str!("../../contracts/daemon_core_inventory_fixture.json"))
        .expect("daemon core inventory fixture should deserialize")
}
