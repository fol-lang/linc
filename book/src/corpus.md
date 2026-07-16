# Preservation corpus

`linc::contract::corpus` embeds the checked schema-v2 link-analysis artifact
paired with PARC's complete preservation source package.

The normal downstream flow is:

1. decode `parc::contract::corpus::COMPLETE_SOURCE_PACKAGE_JSON`;
2. complete it with `linc::contract::corpus::preservation_selection()`;
3. call `validated_preservation_link_analysis()`;
4. consume the validated package and its ordered, repeated resolved link plan.

The frozen link-analysis identity is available through
`preservation_link_analysis_fingerprint()`. The package test extracts both PARC
and LINC archives and verifies this flow from a clean contract-only consumer.
