use parc::contract::SchemaHeader;

pub const LINK_ANALYSIS_KIND: &str = "follang.linc.link-analysis";
pub const LINK_ANALYSIS_SCHEMA_ID: &str = "follang.linc.link-analysis";
pub const LINK_ANALYSIS_SCHEMA_VERSION: u32 = 2;

pub(crate) fn link_analysis_schema_v2() -> SchemaHeader {
    SchemaHeader {
        id: LINK_ANALYSIS_SCHEMA_ID.to_owned(),
        version: LINK_ANALYSIS_SCHEMA_VERSION,
    }
}

pub(crate) fn is_link_analysis_v2(schema: &SchemaHeader) -> bool {
    schema.id == LINK_ANALYSIS_SCHEMA_ID && schema.version == LINK_ANALYSIS_SCHEMA_VERSION
}
