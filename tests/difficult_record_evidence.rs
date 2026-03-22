mod common;
use std::path::PathBuf;

use linc::ir::AbiConfidence;

#[test]
fn difficult_record_evidence_reports_field_offsets_for_nested_non_bitfield_record() {
    let header =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/difficult_record_evidence.h");
    let result = common::process(&linc::raw_headers::HeaderConfig::new()
        .entry_header(&header)
        .probe_type_layout("struct message_payload")
        .probe_type_layout("message_payload_t")
        .probe_type_layout("struct nested_header"))
        .unwrap();

    let record = result
        .package
        .find_record("message_payload")
        .expect("message_payload record");
    assert_eq!(record.abi_confidence, Some(AbiConfidence::FieldOffsetsProbed));
    let fields = record.fields.as_ref().expect("message_payload fields");
    assert_eq!(fields.len(), 3);
    assert!(fields.iter().all(|field| field.layout.is_some()));
    assert!(result
        .package
        .layouts
        .iter()
        .any(|layout| layout.name == "struct message_payload" && layout.size > 0));
    assert!(result
        .package
        .layouts
        .iter()
        .any(|layout| layout.name == "struct nested_header" && layout.size > 0));
}

#[test]
fn difficult_record_evidence_preserves_partial_bitfield_signal_for_nested_record_family() {
    let header =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/difficult_record_evidence.h");
    let result = common::process(&linc::raw_headers::HeaderConfig::new()
        .entry_header(&header)
        .probe_type_layout("struct packed_window")
        .probe_type_layout("message_payload_t"))
        .unwrap();

    let record = result
        .package
        .find_record("packed_window")
        .expect("packed_window record");
    assert_eq!(record.abi_confidence, Some(AbiConfidence::PartialBitfieldLayout));
    let fields = record.fields.as_ref().expect("packed_window fields");
    assert_eq!(fields[0].bit_width, Some(3));
    assert_eq!(fields[1].bit_width, Some(5));
    assert!(fields[2].layout.is_some());
    assert!(fields[3].layout.is_some());
    assert!(fields.iter().any(|field| field.bit_width.is_some()));
    assert!(result
        .package
        .layouts
        .iter()
        .any(|layout| layout.name == "struct packed_window" && layout.size > 0));
}
