use linc::contract::{corpus as linc_corpus, LinkAtom};
use parc::contract::{corpus as parc_corpus, decode_source_package};

#[test]
fn preservation_pair_is_checked_through_public_parc_and_linc_apis() {
    let source = decode_source_package(parc_corpus::COMPLETE_SOURCE_PACKAGE_JSON)
        .expect("decode packaged PARC preservation source");
    let complete = source
        .into_complete(&linc_corpus::preservation_selection())
        .expect("prove the exact preservation closure");
    let package = linc_corpus::decode_preservation_link_analysis()
        .expect("decode packaged LINC preservation evidence");

    assert_eq!(
        package.source_fingerprint(),
        complete.source().fingerprint()
    );
    assert_eq!(
        package.target_fingerprint(),
        complete.source().target_fingerprint()
    );
    assert_eq!(
        package.fingerprint(),
        linc_corpus::preservation_link_analysis_fingerprint()
    );

    let validated = linc_corpus::validated_preservation_link_analysis(&complete)
        .expect("checked PARC closure and checked LINC package must agree");
    assert_eq!(validated.package(), &package);

    let ordered_files = validated
        .package()
        .resolved_link_plan()
        .atoms()
        .iter()
        .filter_map(|atom| match atom {
            LinkAtom::Object(artifact)
            | LinkAtom::StaticLibrary(artifact)
            | LinkAtom::DynamicLibrary(artifact)
            | LinkAtom::ImportLibrary(artifact) => artifact
                .canonical_path()
                .file_name()
                .and_then(|name| name.to_str()),
            LinkAtom::Framework { artifact, .. } => artifact
                .canonical_path()
                .file_name()
                .and_then(|name| name.to_str()),
            LinkAtom::SearchNative(_) | LinkAtom::GroupStart | LinkAtom::GroupEnd => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        ordered_files,
        [
            "libfirst.a",
            "librepeat.a",
            "libmiddle.so",
            "librepeat.a",
            "libparc_fixture.a",
        ]
    );
}
