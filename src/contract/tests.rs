use std::{collections::BTreeMap, ffi::OsString, path::PathBuf, str::FromStr};

use parc::contract::{
    corpus, decode_source_package, Architecture, ClosureRequirement, CompilerIdentity,
    CompleteSourcePackage, ContentFingerprint, DeclarationId, Endian, EnumValue, Environment,
    ExactInteger, Linkage, ObjectFormat, OperatingSystem, Selection, Signedness,
    SourceDeclarationKind, SourceFingerprint, SourceRange, TargetFingerprint,
};
use serde::Deserialize;

use super::*;

const COMPLETE_SOURCE_FINGERPRINT: &str =
    "psource2_a6fe4080b7e1a323cc1c5cf0fca625ed5df0747a745ebbedd0b457d96f201035";
const PARTIAL_SOURCE_FINGERPRINT: &str =
    "psource2_fb42eec49099a7597a1b385e7a7f2d13d165ee15dc8dfae97da481e6a32fefa5";
const CORPUS_TARGET_FINGERPRINT: &str =
    "ptarget1_b5558b1e4776d5eff233fcfbea32b0068cd5c8f83bacdceaf3f406cc6c9dc4b2";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceSeed {
    ownership: String,
    source_fingerprint: SourceFingerprint,
    record_layouts: Vec<SeedRecordLayout>,
    enum_representations: Vec<SeedEnumRepresentation>,
    abi_probe: SeedAbiProbe,
    providers: Vec<SeedProvider>,
    ordered_link_inputs: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedRecordLayout {
    declaration: DeclarationId,
    size_bits: u64,
    alignment_bits: u32,
    fields: Vec<SeedFieldLayout>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedFieldLayout {
    child: ChildId,
    offset_bits: u64,
    size_bits: Option<u64>,
    #[serde(default)]
    alignment_bits: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedEnumRepresentation {
    declaration: DeclarationId,
    storage_bits: u64,
    alignment_bits: u32,
    signedness: Signedness,
    #[serde(default)]
    variants: Vec<SeedEnumVariant>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedEnumVariant {
    child: ChildId,
    value: ExactInteger,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedAbiProbe {
    machine: String,
    object_format: String,
    bitness: u16,
    endian: String,
    abi: String,
    linker_flavor: String,
    crt_flavor: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedProvider {
    declaration: DeclarationId,
    symbol: String,
    state: String,
    provider: Option<String>,
}

struct CorpusFixture {
    complete: CompleteSourcePackage,
    package: LinkAnalysisPackage,
    seed: EvidenceSeed,
}

#[derive(Debug, Clone, Copy)]
struct FixtureOptions {
    binding: SymbolBinding,
    direction: SymbolDirection,
    visibility: SymbolVisibility,
    kind_override: Option<SymbolKind>,
    include_provider_in_plan: bool,
    inferred_layout: bool,
    duplicate_visible_provider: bool,
    foreign_record_child: bool,
    source_override: Option<SourceFingerprint>,
    all_supported: bool,
    omit_callable_probe_subject: bool,
    probe_compiler_mismatch: bool,
    object_metadata_layout: bool,
    duplicate_same_provider_symbol: bool,
    reject_probe_outcome: bool,
}

impl Default for FixtureOptions {
    fn default() -> Self {
        Self {
            binding: SymbolBinding::Global,
            direction: SymbolDirection::Exported,
            visibility: SymbolVisibility::Default,
            kind_override: None,
            include_provider_in_plan: true,
            inferred_layout: false,
            duplicate_visible_provider: false,
            foreign_record_child: false,
            source_override: None,
            all_supported: false,
            omit_callable_probe_subject: false,
            probe_compiler_mismatch: false,
            object_metadata_layout: false,
            duplicate_same_provider_symbol: false,
            reject_probe_outcome: false,
        }
    }
}

#[test]
fn embedded_corpus_builds_checked_lossless_analysis() {
    let fixture = corpus_fixture();
    let encoded = encode_link_analysis(&fixture.package).expect("encode checked analysis");
    let decoded = decode_link_analysis(&encoded).expect("decode checked analysis");

    assert_eq!(decoded, fixture.package);
    assert_eq!(
        decoded.source_fingerprint(),
        ledger_source_fingerprint("complete")
    );
    assert_eq!(decoded.target_fingerprint(), ledger_target_fingerprint());
    assert_eq!(decoded.schema().version, LINK_ANALYSIS_SCHEMA_VERSION);

    let actual_names = decoded
        .resolved_link_plan()
        .atoms()
        .iter()
        .filter_map(link_atom_file_name)
        .collect::<Vec<_>>();
    assert_eq!(
        &actual_names[..fixture.seed.ordered_link_inputs.len()],
        fixture.seed.ordered_link_inputs.as_slice()
    );
    assert_eq!(
        actual_names
            .iter()
            .filter(|name| name.as_str() == "librepeat.a")
            .count(),
        2
    );

    let validated = ValidatedLinkAnalysis::try_new(&fixture.complete, decoded)
        .expect("complete PARC closure has exact LINC evidence");
    assert_eq!(
        validated.package().source_fingerprint(),
        fixture.complete.source().fingerprint()
    );
}

#[test]
fn corpus_is_required_and_ledger_fingerprints_bind_evidence() {
    assert!(corpus::COMPLETE_SOURCE_PACKAGE_JSON.len() > 1_000);
    assert!(corpus::PARTIAL_SOURCE_PACKAGE_JSON.len() > 1_000);
    assert!(corpus::PRESERVATION_LEDGER_JSON.len() > 1_000);
    assert_eq!(corpus::preservation_cases().len(), 2);

    let complete = decode_source_package(corpus::COMPLETE_SOURCE_PACKAGE_JSON)
        .expect("required complete corpus must decode");
    let partial = decode_source_package(corpus::PARTIAL_SOURCE_PACKAGE_JSON)
        .expect("required partial corpus must decode");
    assert_eq!(
        complete.fingerprint(),
        ledger_source_fingerprint("complete")
    );
    assert_eq!(partial.fingerprint(), ledger_source_fingerprint("partial"));
    assert_eq!(complete.target_fingerprint(), ledger_target_fingerprint());
    assert_eq!(
        complete.fingerprint().to_string(),
        COMPLETE_SOURCE_FINGERPRINT
    );
    assert_eq!(
        partial.fingerprint().to_string(),
        PARTIAL_SOURCE_FINGERPRINT
    );
    assert_eq!(
        complete.target_fingerprint().to_string(),
        CORPUS_TARGET_FINGERPRINT
    );
    assert!(partial.into_complete(&Selection::all_supported()).is_err());

    let seed = evidence_seed();
    assert_eq!(seed.source_fingerprint, complete.fingerprint());
    assert!(seed.ownership.contains("not part of PARC SourcePackage"));
    assert_eq!(seed.abi_probe.machine, "x86_64");
    assert_eq!(seed.abi_probe.object_format, "elf");
    assert_eq!(seed.abi_probe.endian, "little");
    assert!(seed.enum_representations.iter().all(|representation| {
        representation.storage_bits == 32
            && representation.alignment_bits == 32
            && representation.signedness == Signedness::Unsigned
    }));
    assert!(seed.providers.iter().any(|provider| {
        provider.symbol == "parc_missing"
            && provider.state == "unresolved"
            && provider.provider.is_none()
    }));
}

#[test]
fn codec_rejects_future_version_before_payload_decode() {
    let fixture = corpus_fixture();
    let encoded = encode_link_analysis(&fixture.package).expect("encode");
    let mut envelope: serde_json::Value = serde_json::from_slice(&encoded).expect("json");
    envelope["schema"]["version"] = serde_json::json!(3);
    envelope["payload"] = serde_json::Value::String("not a package".to_owned());
    let bytes = serde_json::to_vec(&envelope).expect("json");
    assert!(matches!(
        decode_link_analysis(&bytes),
        Err(DecodeError::SchemaVersion { found: 3 })
    ));
}

#[test]
fn codec_rejects_unknown_fields_at_envelope_payload_and_nested_boundaries() {
    let fixture = corpus_fixture();
    let encoded = encode_link_analysis(&fixture.package).expect("encode");

    let mut unknown_envelope: serde_json::Value =
        serde_json::from_slice(&encoded).expect("json envelope");
    unknown_envelope["unexpected"] = serde_json::json!(true);
    assert!(matches!(
        decode_link_analysis(&serde_json::to_vec(&unknown_envelope).expect("json")),
        Err(DecodeError::Envelope(_))
    ));

    let mut unknown_payload: serde_json::Value =
        serde_json::from_slice(&encoded).expect("json envelope");
    unknown_payload["payload"]["unexpected"] = serde_json::json!(true);
    assert!(matches!(
        decode_link_analysis(&serde_json::to_vec(&unknown_payload).expect("json")),
        Err(DecodeError::Payload(_))
    ));

    let mut unknown_artifact: serde_json::Value =
        serde_json::from_slice(&encoded).expect("json envelope");
    unknown_artifact["payload"]["inventories"][0]["artifact"]["unexpected"] =
        serde_json::json!(true);
    assert!(matches!(
        decode_link_analysis(&serde_json::to_vec(&unknown_artifact).expect("json")),
        Err(DecodeError::Payload(_))
    ));

    let mut unit_plan: serde_json::Value = serde_json::from_slice(&encoded).expect("json envelope");
    unit_plan["payload"]["resolved_link_plan"]
        .as_array_mut()
        .expect("plan")
        .push(serde_json::json!({"kind":"group_start","future":true}));
    assert!(matches!(
        decode_link_analysis(&serde_json::to_vec(&unit_plan).expect("json")),
        Err(DecodeError::UnitVariantShape { .. })
    ));

    let mut unit_assessment: serde_json::Value =
        serde_json::from_slice(&encoded).expect("json envelope");
    let evidence = unit_assessment["payload"]["declaration_evidence"]
        .as_array_mut()
        .expect("declaration evidence")
        .iter_mut()
        .find(|evidence| evidence["layout"]["state"] == "not_required")
        .expect("unit layout assessment");
    evidence["layout"]["future"] = serde_json::json!(true);
    assert!(matches!(
        decode_link_analysis(&serde_json::to_vec(&unit_assessment).expect("json")),
        Err(DecodeError::UnitVariantShape { .. })
    ));
}

#[test]
fn codec_rejects_malformed_stable_diagnostic_codes() {
    let fixture = corpus_fixture();
    let encoded = encode_link_analysis(&fixture.package).expect("encode");
    let mut malformed: serde_json::Value = serde_json::from_slice(&encoded).expect("json envelope");
    malformed["payload"]["diagnostics"]
        .as_array_mut()
        .expect("diagnostics")
        .push(serde_json::json!({
            "code":"free-form",
            "severity":"error",
            "stage":"validation",
            "message":"bad code",
            "declaration":null,
            "provider":null,
            "context":{
                "target_fingerprint":CORPUS_TARGET_FINGERPRINT,
                "source_range":null,
                "native_input_index":null,
                "dependency_provider":null,
                "probe":null,
                "evidence":null
            }
        }));
    assert!(matches!(
        decode_link_analysis(&serde_json::to_vec(&malformed).expect("json")),
        Err(DecodeError::Payload(_))
    ));
}

#[test]
fn codec_rejects_diagnostic_class_severity_and_probe_rejection_forgery() {
    let fixture = corpus_fixture();
    let declaration = fixture.complete.declaration_closure()[0].declaration();
    let diagnostic = LincDiagnostic::try_new(LincDiagnosticInput {
        code: LincCode::try_new("LINC-E4102").expect("error code"),
        severity: DiagnosticSeverity::Error,
        stage: DiagnosticStage::Validation,
        message: "checked error".to_owned(),
        declaration: Some(declaration),
        provider: None,
        context: LincDiagnosticContext::new(
            fixture.package.target_fingerprint(),
            None,
            None,
            None,
            None,
            Some(DiagnosticEvidenceRef::Declaration { declaration }),
        ),
    })
    .expect("matching code class and severity");
    let mut diagnostic_input = package_input(&fixture.package);
    diagnostic_input.diagnostics = vec![diagnostic];
    let diagnostic_package =
        LinkAnalysisPackage::try_new(diagnostic_input).expect("package with checked diagnostic");
    let mut forged_severity: serde_json::Value = serde_json::from_slice(
        &encode_link_analysis(&diagnostic_package).expect("encode diagnostic package"),
    )
    .expect("json");
    forged_severity["payload"]["diagnostics"][0]["severity"] = serde_json::json!("note");
    assert!(matches!(
        decode_link_analysis(&serde_json::to_vec(&forged_severity).expect("json")),
        Err(DecodeError::Contract(
            ContractError::DiagnosticSeverityMismatch { .. }
        ))
    ));

    let mut forged_rejection: serde_json::Value = serde_json::from_slice(
        &encode_link_analysis(&fixture.package).expect("encode probe package"),
    )
    .expect("json");
    forged_rejection["payload"]["abi_probes"][0]["subject_outcomes"][0]["status"] =
        serde_json::json!({"state":"rejected","code":"LINC-N4100"});
    assert!(matches!(
        decode_link_analysis(&serde_json::to_vec(&forged_rejection).expect("json")),
        Err(DecodeError::Contract(
            ContractError::InvalidProbeRejectionCode { .. }
        ))
    ));

    assert!(ProbeSubjectOutcome::try_new(
        fixture.package.abi_probes()[0].subjects()[0],
        ProbeSubjectStatus::Rejected {
            code: LincCode::try_new("LINC-P4100").expect("partial rejection code")
        },
    )
    .is_ok());
    assert!(matches!(
        ProbeSubjectOutcome::try_new(
            fixture.package.abi_probes()[0].subjects()[0],
            ProbeSubjectStatus::Rejected {
                code: LincCode::try_new("LINC-W4100").expect("warning code")
            },
        ),
        Err(ContractError::InvalidProbeRejectionCode { .. })
    ));
}

#[test]
fn codec_rejects_fingerprint_provider_and_canonical_order_forgery() {
    let fixture = corpus_fixture();
    let encoded = encode_link_analysis(&fixture.package).expect("encode");

    let mut mismatched_envelope: serde_json::Value =
        serde_json::from_slice(&encoded).expect("json envelope");
    let different = LinkAnalysisFingerprint::from_str(&format!("lanalysis2_{}", "0".repeat(64)))
        .expect("syntactically valid fingerprint");
    mismatched_envelope["fingerprint"] = serde_json::json!(different.to_string());
    assert!(matches!(
        decode_link_analysis(&serde_json::to_vec(&mismatched_envelope).expect("json")),
        Err(DecodeError::EnvelopeFingerprint)
    ));

    let mut forged_provider: serde_json::Value =
        serde_json::from_slice(&encoded).expect("json envelope");
    let providers = forged_provider["payload"]["inventories"]
        .as_array()
        .expect("inventories");
    assert!(providers.len() >= 2);
    let replacement = providers[1]["artifact"]["provider_id"].clone();
    forged_provider["payload"]["inventories"][0]["artifact"]["provider_id"] = replacement;
    assert!(matches!(
        decode_link_analysis(&serde_json::to_vec(&forged_provider).expect("json")),
        Err(DecodeError::Contract(
            ContractError::ProviderIdMismatch { .. }
        ))
    ));

    let mut reordered: serde_json::Value = serde_json::from_slice(&encoded).expect("json envelope");
    reordered["payload"]["inventories"]
        .as_array_mut()
        .expect("inventories")
        .reverse();
    assert!(matches!(
        decode_link_analysis(&serde_json::to_vec(&reordered).expect("json")),
        Err(DecodeError::Contract(ContractError::NonCanonicalOrder {
            collection: "inventories"
        }))
    ));
}

#[test]
fn codec_rejects_unbalanced_groups_and_resource_exhaustion() {
    let fixture = corpus_fixture();
    let encoded = encode_link_analysis(&fixture.package).expect("encode");
    let mut unbalanced: serde_json::Value =
        serde_json::from_slice(&encoded).expect("json envelope");
    unbalanced["payload"]["resolved_link_plan"]
        .as_array_mut()
        .expect("plan")
        .push(serde_json::json!({ "kind": "group_start" }));
    assert!(matches!(
        decode_link_analysis(&serde_json::to_vec(&unbalanced).expect("json")),
        Err(DecodeError::Contract(ContractError::UnclosedGroups {
            depth: 1
        }))
    ));

    let limits = DecodeLimits {
        max_link_atoms: 1,
        ..DecodeLimits::default()
    };
    assert!(matches!(
        decode_link_analysis_with_limits(&encoded, limits),
        Err(DecodeError::ResourceLimit {
            resource: "link atoms",
            ..
        })
    ));
}

#[test]
fn wrapper_rejects_foreign_child_inferred_layout_and_wrong_source() {
    let foreign = corpus_fixture_with(FixtureOptions {
        foreign_record_child: true,
        ..FixtureOptions::default()
    });
    assert!(matches!(
        ValidatedLinkAnalysis::try_new(&foreign.complete, foreign.package),
        Err(ContractError::ForeignLayoutChild { .. })
    ));

    let inferred = corpus_fixture_with(FixtureOptions {
        inferred_layout: true,
        ..FixtureOptions::default()
    });
    assert!(matches!(
        ValidatedLinkAnalysis::try_new(&inferred.complete, inferred.package),
        Err(ContractError::InferredLayoutEvidence { .. })
    ));

    let wrong_source = corpus_fixture_with(FixtureOptions {
        source_override: Some(ledger_source_fingerprint("partial")),
        ..FixtureOptions::default()
    });
    assert!(matches!(
        ValidatedLinkAnalysis::try_new(&wrong_source.complete, wrong_source.package),
        Err(ContractError::SourceFingerprintMismatch { .. })
    ));
}

#[test]
fn wrapper_rejects_nonvisible_wrong_kind_absent_and_ambiguous_providers() {
    let cases = [
        FixtureOptions {
            binding: SymbolBinding::Local,
            ..FixtureOptions::default()
        },
        FixtureOptions {
            direction: SymbolDirection::Imported,
            ..FixtureOptions::default()
        },
        FixtureOptions {
            visibility: SymbolVisibility::Hidden,
            ..FixtureOptions::default()
        },
        FixtureOptions {
            binding: SymbolBinding::Weak,
            ..FixtureOptions::default()
        },
    ];
    for options in cases {
        let fixture = corpus_fixture_with(options);
        assert!(matches!(
            ValidatedLinkAnalysis::try_new(&fixture.complete, fixture.package),
            Err(ContractError::SymbolNotVisible { .. })
        ));
    }

    let wrong_kind = corpus_fixture_with(FixtureOptions {
        kind_override: Some(SymbolKind::Data),
        ..FixtureOptions::default()
    });
    assert!(matches!(
        ValidatedLinkAnalysis::try_new(&wrong_kind.complete, wrong_kind.package),
        Err(ContractError::SymbolKindMismatch { .. })
    ));

    let absent = corpus_fixture_with(FixtureOptions {
        include_provider_in_plan: false,
        ..FixtureOptions::default()
    });
    assert!(matches!(
        ValidatedLinkAnalysis::try_new(&absent.complete, absent.package),
        Err(ContractError::ProviderNotInPlan { .. })
    ));

    let ambiguous = corpus_fixture_with(FixtureOptions {
        duplicate_visible_provider: true,
        ..FixtureOptions::default()
    });
    assert!(matches!(
        ValidatedLinkAnalysis::try_new(&ambiguous.complete, ambiguous.package),
        Err(ContractError::AmbiguousVisibleProviders { count: 2, .. })
    ));

    let same_provider = corpus_fixture_with(FixtureOptions {
        duplicate_same_provider_symbol: true,
        ..FixtureOptions::default()
    });
    assert!(matches!(
        ValidatedLinkAnalysis::try_new(&same_provider.complete, same_provider.package),
        Err(ContractError::AmbiguousVisibleProviders { count: 2, .. })
    ));
}

#[test]
fn strict_symbol_decoration_is_target_and_calling_convention_bound() {
    let fixture = corpus_fixture();
    let linux = fixture.package.inventories()[0]
        .artifact()
        .observed_target();
    assert_eq!(
        super::package::canonical_symbol_spelling(
            "entry",
            &SymbolDecoration::None,
            linux,
            Some(&parc::contract::CallingConvention::C),
            Some(&parc::contract::CallingConvention::C),
        )
        .expect("undecorated spelling"),
        "entry"
    );
    assert!(super::package::canonical_symbol_spelling(
        "entry",
        &SymbolDecoration::LeadingUnderscore,
        linux,
        Some(&parc::contract::CallingConvention::C),
        Some(&parc::contract::CallingConvention::C),
    )
    .is_err());

    let mut macho_parts = linux.parts();
    macho_parts.architecture = Architecture::X86_64;
    macho_parts.operating_system = OperatingSystem::MacOs;
    macho_parts.environment = Environment::None;
    macho_parts.object_format = ObjectFormat::MachO;
    macho_parts.endian = Endian::Little;
    macho_parts.pointer_width = 64;
    macho_parts.abi = NativeAbi::SysV64;
    macho_parts.linker = LinkerFlavor::Darwin;
    macho_parts.crt = CrtFlavor::Darwin;
    let macho = ObservedTarget::try_new(macho_parts).expect("modeled Mach-O target");
    assert_eq!(
        super::package::canonical_symbol_spelling(
            "entry",
            &SymbolDecoration::LeadingUnderscore,
            &macho,
            Some(&parc::contract::CallingConvention::C),
            Some(&parc::contract::CallingConvention::C),
        )
        .expect("certified Mach-O C spelling"),
        "_entry"
    );
    assert!(super::package::canonical_symbol_spelling(
        "entry",
        &SymbolDecoration::LeadingUnderscore,
        &macho,
        Some(&parc::contract::CallingConvention::C),
        Some(&parc::contract::CallingConvention::Cdecl),
    )
    .is_err());

    let mut windows_parts = linux.parts();
    windows_parts.architecture = Architecture::X86;
    windows_parts.operating_system = OperatingSystem::Windows;
    windows_parts.environment = Environment::Msvc;
    windows_parts.object_format = ObjectFormat::Coff;
    windows_parts.endian = Endian::Little;
    windows_parts.pointer_width = 32;
    windows_parts.abi = NativeAbi::Win32;
    windows_parts.linker = LinkerFlavor::Msvc;
    windows_parts.crt = CrtFlavor::Msvc;
    let windows = ObservedTarget::try_new(windows_parts).expect("modeled Windows x86 target");
    assert!(super::package::canonical_symbol_spelling(
        "entry",
        &SymbolDecoration::Stdcall { stack_bytes: 12 },
        &windows,
        Some(&parc::contract::CallingConvention::Stdcall),
        Some(&parc::contract::CallingConvention::Stdcall),
    )
    .is_err());
    assert!(super::package::canonical_symbol_spelling(
        "entry",
        &SymbolDecoration::Stdcall { stack_bytes: 8 },
        &windows,
        Some(&parc::contract::CallingConvention::C),
        Some(&parc::contract::CallingConvention::C),
    )
    .is_err());
    assert!(super::package::canonical_symbol_spelling(
        "entry",
        &SymbolDecoration::Versioned {
            version: b"V1".to_vec(),
            is_default: true,
        },
        &macho,
        Some(&parc::contract::CallingConvention::C),
        Some(&parc::contract::CallingConvention::C),
    )
    .is_err());
    assert!(super::package::canonical_symbol_spelling(
        "entry",
        &SymbolDecoration::Other {
            spelling: b"?entry@@".to_vec(),
        },
        &windows,
        Some(&parc::contract::CallingConvention::C),
        Some(&parc::contract::CallingConvention::C),
    )
    .is_err());

    let declaration_index = fixture
        .package
        .declaration_evidence()
        .iter()
        .position(|evidence| {
            matches!(
                evidence.symbol(),
                SymbolAssessment::Exact { expected_name, .. } if expected_name == "parc_open"
            )
        })
        .expect("parc_open evidence");
    let original_evidence = &fixture.package.declaration_evidence()[declaration_index];
    let SymbolAssessment::Exact {
        symbol,
        expected_name,
        kind,
        ..
    } = original_evidence.symbol()
    else {
        unreachable!("selected exact evidence")
    };
    let provider = symbol.provider();
    let inventory = fixture
        .package
        .inventories()
        .iter()
        .find(|inventory| inventory.artifact().provider_id() == provider)
        .expect("parc_open provider")
        .clone();
    let mut symbols = inventory.symbols().to_vec();
    let record = symbols
        .iter_mut()
        .find(|record| record.id() == symbol.symbol())
        .expect("parc_open symbol");
    let mut record_input = record.input();
    record_input.name = format!("_{expected_name}");
    record_input.raw_name = record_input.name.as_bytes().to_vec();
    record_input.decoration = SymbolDecoration::LeadingUnderscore;
    *record = SymbolRecord::try_new(record_input).expect("decorated record");
    let decorated_inventory = SymbolInventory::try_new(
        inventory.artifact().clone(),
        inventory.inspection().clone(),
        symbols,
        inventory.dependency_edges().to_vec(),
    )
    .expect("decorated inventory");
    let decorated_evidence = DeclarationEvidence::try_new(DeclarationEvidenceInput {
        declaration: original_evidence.declaration(),
        source_fingerprint: original_evidence.source_fingerprint(),
        target_fingerprint: original_evidence.target_fingerprint(),
        provider: original_evidence.provider().clone(),
        symbol: SymbolAssessment::Exact {
            symbol: *symbol,
            expected_name: expected_name.clone(),
            actual_name: format!("_{expected_name}"),
            kind: *kind,
            decoration: SymbolDecoration::LeadingUnderscore,
        },
        layout: original_evidence.layout().clone(),
        callable_abi: original_evidence.callable_abi().clone(),
    })
    .expect("decorated declaration evidence");
    let mut input = package_input(&fixture.package);
    replace_inventory(&mut input.inventories, decorated_inventory);
    input.declaration_evidence[declaration_index] = decorated_evidence;
    let decorated_package =
        LinkAnalysisPackage::try_new(input).expect("internally retained decoration claim");
    assert!(matches!(
        ValidatedLinkAnalysis::try_new(&fixture.complete, decorated_package),
        Err(ContractError::InvalidSymbolDecoration { .. })
    ));
}

#[test]
fn strict_wrapper_rejects_unresolved_and_unaudited_probe_evidence() {
    let unresolved = corpus_fixture_with(FixtureOptions {
        all_supported: true,
        ..FixtureOptions::default()
    });
    assert!(unresolved
        .package
        .declaration_evidence()
        .iter()
        .any(|evidence| {
            matches!(evidence.provider(), ProviderAssessment::Unresolved)
                && matches!(evidence.symbol(), SymbolAssessment::Missing { .. })
        }));
    assert!(matches!(
        ValidatedLinkAnalysis::try_new(&unresolved.complete, unresolved.package),
        Err(ContractError::RequiredSymbolEvidence { .. })
    ));

    let wrong_compiler = corpus_fixture_with(FixtureOptions {
        probe_compiler_mismatch: true,
        ..FixtureOptions::default()
    });
    assert!(matches!(
        ValidatedLinkAnalysis::try_new(&wrong_compiler.complete, wrong_compiler.package),
        Err(ContractError::ProbeCompilerMismatch { .. })
    ));

    let wrong_subject = corpus_fixture_with(FixtureOptions {
        omit_callable_probe_subject: true,
        ..FixtureOptions::default()
    });
    assert!(matches!(
        ValidatedLinkAnalysis::try_new(&wrong_subject.complete, wrong_subject.package),
        Err(ContractError::ProbeSubjectMismatch { .. })
    ));

    let metadata = corpus_fixture_with(FixtureOptions {
        object_metadata_layout: true,
        ..FixtureOptions::default()
    });
    assert!(matches!(
        ValidatedLinkAnalysis::try_new(&metadata.complete, metadata.package),
        Err(ContractError::InferredLayoutEvidence { .. })
    ));

    let rejected = corpus_fixture_with(FixtureOptions {
        reject_probe_outcome: true,
        ..FixtureOptions::default()
    });
    assert!(matches!(
        ValidatedLinkAnalysis::try_new(&rejected.complete, rejected.package),
        Err(ContractError::ProbeSubjectMismatch { .. })
    ));
}

#[test]
fn package_canonicalizes_sets_but_never_link_sequence() {
    let fixture = corpus_fixture();
    let mut inventories = fixture.package.inventories().to_vec();
    inventories.reverse();
    let mut layouts = fixture.package.layouts().to_vec();
    layouts.reverse();
    let mut declaration_evidence = fixture.package.declaration_evidence().to_vec();
    declaration_evidence.reverse();

    let reordered = LinkAnalysisPackage::try_new(LinkAnalysisPackageInput {
        source_fingerprint: fixture.package.source_fingerprint(),
        target_fingerprint: fixture.package.target_fingerprint(),
        analysis_policy: fixture.package.analysis_policy().clone(),
        native_inputs: fixture.package.native_inputs().to_vec(),
        inventories,
        abi_probes: fixture.package.abi_probes().iter().rev().cloned().collect(),
        layouts,
        declaration_evidence,
        resolved_link_plan: fixture.package.resolved_link_plan().clone(),
        diagnostics: fixture.package.diagnostics().to_vec(),
    })
    .expect("nonsemantic collections canonicalize");
    assert_eq!(reordered, fixture.package);
    assert_eq!(reordered.fingerprint(), fixture.package.fingerprint());

    let mut atoms = fixture.package.resolved_link_plan().atoms().to_vec();
    atoms.reverse();
    let reversed_plan = LinkAnalysisPackage::try_new(LinkAnalysisPackageInput {
        source_fingerprint: fixture.package.source_fingerprint(),
        target_fingerprint: fixture.package.target_fingerprint(),
        analysis_policy: fixture.package.analysis_policy().clone(),
        native_inputs: fixture.package.native_inputs().to_vec(),
        inventories: fixture.package.inventories().to_vec(),
        abi_probes: fixture.package.abi_probes().to_vec(),
        layouts: fixture.package.layouts().to_vec(),
        declaration_evidence: fixture.package.declaration_evidence().to_vec(),
        resolved_link_plan: ResolvedLinkPlan::try_new(atoms).expect("reverse plan"),
        diagnostics: fixture.package.diagnostics().to_vec(),
    })
    .expect("sequence variant package");
    assert_ne!(reversed_plan.fingerprint(), fixture.package.fingerprint());
    assert_ne!(
        reversed_plan.resolved_link_plan(),
        fixture.package.resolved_link_plan()
    );
}

#[test]
fn inventory_preserves_duplicate_names_size_and_dependency_order() {
    let fixture = corpus_fixture();
    let target = fixture.package.inventories()[0]
        .artifact()
        .observed_target()
        .clone();
    let artifact = artifact("libduplicates.a", ArtifactKind::StaticLibrary, &target);
    let symbol = |index| {
        SymbolRecord::try_new(SymbolRecordInput {
            id: ArtifactSymbolId::new(0, index),
            name: "same_name".to_owned(),
            raw_name: b"same_name".to_vec(),
            version: None,
            direction: SymbolDirection::Exported,
            kind: SymbolKind::Function,
            binding: SymbolBinding::Global,
            visibility: SymbolVisibility::Default,
            decoration: SymbolDecoration::None,
            size: 16 + index,
            address: Some(0x1000 + index),
            section: Some(b".text".to_vec()),
            archive_member: Some(format!("member-{index}.o").into_bytes()),
        })
        .expect("duplicate-name symbol")
    };
    let edges = vec![
        DependencyEdge::try_new(
            OsString::from("libsecond.so"),
            None,
            DependencyProvenance::DynamicTable,
        )
        .expect("edge"),
        DependencyEdge::try_new(
            OsString::from("libfirst.so"),
            None,
            DependencyProvenance::DynamicTable,
        )
        .expect("edge"),
        DependencyEdge::try_new(
            OsString::from("libsecond.so"),
            None,
            DependencyProvenance::DynamicTable,
        )
        .expect("repeated edge"),
    ];
    let inventory = SymbolInventory::try_new(
        artifact,
        fixture_inspection(InspectionParserKind::Archive),
        vec![symbol(2), symbol(1)],
        edges,
    )
    .expect("duplicate names remain distinct by artifact-local identity");
    assert_eq!(inventory.symbols().len(), 2);
    assert_eq!(inventory.symbols()[0].id(), ArtifactSymbolId::new(0, 1));
    assert_eq!(inventory.symbols()[0].size(), 17);
    let names = inventory
        .dependency_edges()
        .iter()
        .map(|edge| edge.requested().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec!["libsecond.so", "libfirst.so", "libsecond.so"]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>()
    );
}

#[test]
fn inspection_parser_identity_must_match_the_artifact_container_and_format() {
    let fixture = corpus_fixture();
    let original = inventory_named(&fixture.package, "libfirst.a");
    let mismatched = SymbolInventory::try_new(
        original.artifact().clone(),
        fixture_inspection(InspectionParserKind::MachO),
        original.symbols().to_vec(),
        original.dependency_edges().to_vec(),
    )
    .expect("inventory constructor preserves parser claims for package validation");
    let mut input = package_input(&fixture.package);
    replace_inventory(&mut input.inventories, mismatched);
    assert!(matches!(
        LinkAnalysisPackage::try_new(input),
        Err(ContractError::InspectionParserMismatch { .. })
    ));

    let encoded = encode_link_analysis(&fixture.package).expect("encode");
    let mut forged: serde_json::Value = serde_json::from_slice(&encoded).expect("json");
    let parsers = forged["payload"]["inventories"][0]["inspection"]["parsers"]
        .as_array_mut()
        .expect("parser identities");
    parsers.truncate(1);
    parsers[0]["kind"] = serde_json::json!("mach_o");
    let decode_result = decode_link_analysis(&serde_json::to_vec(&forged).expect("json"));
    assert!(
        matches!(
            &decode_result,
            Err(DecodeError::Contract(
                ContractError::InspectionParserMismatch { .. }
            ))
        ),
        "unexpected mismatch result: {decode_result:?}"
    );
}

#[test]
fn dependency_graph_requires_bidirectional_evidence_and_acyclic_plan_order() {
    let fixture = corpus_fixture();

    let accepted_input = package_input_with_dependency(&fixture, "libfirst.a", "librepeat.a");
    let accepted = LinkAnalysisPackage::try_new(accepted_input.clone())
        .expect("one parent-to-child dependency is internally coherent");
    ValidatedLinkAnalysis::try_new(&fixture.complete, accepted)
        .expect("parent precedes child in the resolved plan");

    let parent = inventory_named_in(&accepted_input.inventories, "libfirst.a").clone();
    let child = inventory_named_in(&accepted_input.inventories, "librepeat.a").clone();
    let parent_provider = parent.artifact().provider_id();
    let child_provider = child.artifact().provider_id();

    let mut missing_parent_edge = accepted_input.clone();
    replace_inventory(
        &mut missing_parent_edge.inventories,
        rebuild_inventory(parent.clone(), parent.artifact().clone(), Vec::new()),
    );
    assert!(matches!(
        LinkAnalysisPackage::try_new(missing_parent_edge),
        Err(ContractError::DependencyCrossReference {
            parent,
            child
        }) if parent == parent_provider && child == child_provider
    ));

    let explicit_child = inventory_named(&fixture.package, "librepeat.a").clone();
    let mut mismatched_child_resolution = accepted_input.clone();
    replace_inventory(
        &mut mismatched_child_resolution.inventories,
        explicit_child.clone(),
    );
    mismatched_child_resolution.resolved_link_plan = replace_plan_artifact(
        &mismatched_child_resolution.resolved_link_plan,
        explicit_child.artifact().clone(),
    );
    assert!(matches!(
        LinkAnalysisPackage::try_new(mismatched_child_resolution),
        Err(ContractError::DependencyCrossReference {
            parent,
            child
        }) if parent == parent_provider && child == child_provider
    ));

    let mut cycle = accepted_input;
    let parent_dependency = artifact_with_resolution(
        parent.artifact(),
        ProviderResolution::Dependency {
            parent: child_provider,
        },
    );
    replace_inventory(
        &mut cycle.inventories,
        rebuild_inventory(
            parent.clone(),
            parent_dependency.clone(),
            parent.dependency_edges().to_vec(),
        ),
    );
    replace_inventory(
        &mut cycle.inventories,
        rebuild_inventory(
            child.clone(),
            child.artifact().clone(),
            vec![DependencyEdge::try_new(
                OsString::from("libfirst.a"),
                Some(parent_provider),
                DependencyProvenance::ArchiveDirective,
            )
            .expect("reverse dependency edge")],
        ),
    );
    cycle.resolved_link_plan = replace_plan_artifact(&cycle.resolved_link_plan, parent_dependency);
    assert!(matches!(
        LinkAnalysisPackage::try_new(cycle),
        Err(ContractError::DependencyCycle { .. })
    ));
}

#[test]
fn unresolved_dependency_edges_are_preserved_but_rejected_by_strict_validation() {
    let fixture = corpus_fixture();
    let parent = inventory_named(&fixture.package, "libfirst.a").clone();
    let unresolved_edge = DependencyEdge::try_new(
        OsString::from("libnotcaptured.so"),
        None,
        DependencyProvenance::DynamicTable,
    )
    .expect("unresolved dependency evidence");
    let parent_provider = parent.artifact().provider_id();
    let unresolved_inventory = rebuild_inventory(
        parent.clone(),
        parent.artifact().clone(),
        vec![unresolved_edge],
    );
    let mut input = package_input(&fixture.package);
    replace_inventory(&mut input.inventories, unresolved_inventory);
    let package = LinkAnalysisPackage::try_new(input)
        .expect("unchecked packages retain unresolved dependency evidence");
    assert_eq!(
        inventory_named(&package, "libfirst.a").dependency_edges()[0].provider(),
        None
    );
    assert!(matches!(
        ValidatedLinkAnalysis::try_new(&fixture.complete, package),
        Err(ContractError::UnresolvedDependency { parent }) if parent == parent_provider
    ));
}

#[test]
fn probe_environment_is_canonical_and_rejects_stored_fingerprint_forgery() {
    let path = ProbeEnvironmentEntry::try_new(
        "PATH".to_owned(),
        ProbeEnvironmentValue::Set {
            value_fingerprint: ContentFingerprint::from_content(b"/toolchain/bin"),
        },
    )
    .expect("PATH capture");
    let locale = ProbeEnvironmentEntry::try_new(
        "LC_ALL".to_owned(),
        ProbeEnvironmentValue::Set {
            value_fingerprint: ContentFingerprint::from_content(b"C"),
        },
    )
    .expect("locale capture");
    let first = ProbeEnvironmentIdentity::try_new(
        ProbeEnvironmentPolicy::Explicit,
        vec![path.clone(), locale.clone()],
    )
    .expect("explicit environment");
    let second = ProbeEnvironmentIdentity::try_new(
        ProbeEnvironmentPolicy::Explicit,
        vec![locale.clone(), path.clone()],
    )
    .expect("entry order canonicalizes");
    assert_eq!(first, second);
    assert_eq!(
        first
            .entries()
            .iter()
            .map(ProbeEnvironmentEntry::name)
            .collect::<Vec<_>>(),
        ["LC_ALL", "PATH"]
    );
    assert!(ProbeEnvironmentIdentity::try_new(
        ProbeEnvironmentPolicy::Explicit,
        vec![path.clone(), path],
    )
    .is_err());

    let fixture = corpus_fixture();
    let encoded = encode_link_analysis(&fixture.package).expect("encode");
    let mut forged: serde_json::Value = serde_json::from_slice(&encoded).expect("json");
    forged["payload"]["analysis_policy"]["probe_execution"]["environment"]["fingerprint"] =
        serde_json::json!(ContentFingerprint::from_content(b"forged-environment").to_string());
    assert!(matches!(
        decode_link_analysis(&serde_json::to_vec(&forged).expect("json")),
        Err(DecodeError::Contract(
            ContractError::ProbeEnvironmentFingerprintMismatch { .. }
        ))
    ));
}

#[test]
fn probe_method_runner_and_execution_result_must_cohere_in_both_directions() {
    let fixture = corpus_fixture();
    let original = &fixture.package.abi_probes()[0];

    let mut claims_execution_without_runner = original.input();
    claims_execution_without_runner.method = ProbeMethod::ExecutedHarness;
    assert!(matches!(
        AbiProbeEvidence::try_new(claims_execution_without_runner),
        Err(ContractError::InvalidPolicy { .. })
    ));

    let mut carries_runner_without_execution_method = original.input();
    carries_runner_without_execution_method.runner = ProbeRunnerEvidence::Executed {
        executable_path: PathBuf::from("/runner/bin/qemu-x86_64"),
        executable_fingerprint: ArtifactFingerprint::from_content(b"qemu-x86_64"),
        arguments: vec![ProbeRunnerArgument::ProbeExecutable],
    };
    carries_runner_without_execution_method.execution_result = Some(ProbeProcessResult::new(
        ProbeProcessStatus::Exited { code: 0 },
        ContentFingerprint::from_content(b"run-stdout"),
        ContentFingerprint::from_content(b"run-stderr"),
        None,
    ));
    assert!(matches!(
        AbiProbeEvidence::try_new(carries_runner_without_execution_method),
        Err(ContractError::InvalidPolicy { .. })
    ));
}

#[test]
fn probe_identity_excludes_per_run_ephemeral_workspace_paths() {
    let fixture = corpus_fixture();
    let input = fixture.package.abi_probes()[0].input();
    let physical_run_a = input.execution_policy.temporary_parent().join("run-a-123");
    let physical_run_b = input.execution_policy.temporary_parent().join("run-b-987");
    assert_ne!(physical_run_a, physical_run_b);

    let first = AbiProbeEvidence::try_new(input.clone()).expect("first semantic probe");
    let second = AbiProbeEvidence::try_new(input).expect("second semantic probe");
    assert_eq!(first.id(), second.id());
}

#[test]
fn native_input_sequence_and_repetition_are_fingerprint_semantics() {
    let fixture = corpus_fixture();
    let mut reversed_input = package_input(&fixture.package);
    reversed_input.native_inputs.reverse();
    let reversed = LinkAnalysisPackage::try_new(reversed_input).expect("reversed input snapshot");
    assert_ne!(reversed.native_inputs(), fixture.package.native_inputs());
    assert_ne!(reversed.fingerprint(), fixture.package.fingerprint());

    let mut repeated_input = package_input(&fixture.package);
    repeated_input
        .native_inputs
        .push(repeated_input.native_inputs[0].clone());
    let repeated = LinkAnalysisPackage::try_new(repeated_input).expect("repeated input snapshot");
    assert_ne!(repeated.fingerprint(), fixture.package.fingerprint());
}

#[test]
fn diagnostics_are_canonical_unique_and_source_ranges_are_strictly_checked() {
    let fixture = corpus_fixture();
    let declaration = fixture.complete.declaration_closure()[0].declaration();
    let diagnostic = LincDiagnostic::try_new(LincDiagnosticInput {
        code: LincCode::try_new("LINC-W4100").expect("stable code"),
        severity: DiagnosticSeverity::Warning,
        stage: DiagnosticStage::Validation,
        message: "preservation warning".to_owned(),
        declaration: Some(declaration),
        provider: None,
        context: LincDiagnosticContext::new(
            fixture.package.target_fingerprint(),
            None,
            None,
            None,
            None,
            Some(DiagnosticEvidenceRef::Declaration { declaration }),
        ),
    })
    .expect("diagnostic");
    let mut duplicate = package_input(&fixture.package);
    duplicate.diagnostics = vec![diagnostic.clone(), diagnostic];
    assert!(matches!(
        LinkAnalysisPackage::try_new(duplicate),
        Err(ContractError::DuplicateDiagnostic)
    ));

    let file = &fixture.complete.source().files()[0];
    let invalid_range = LincDiagnostic::try_new(LincDiagnosticInput {
        code: LincCode::try_new("LINC-E4101").expect("stable code"),
        severity: DiagnosticSeverity::Error,
        stage: DiagnosticStage::Validation,
        message: "empty source range".to_owned(),
        declaration: Some(declaration),
        provider: None,
        context: LincDiagnosticContext::new(
            fixture.package.target_fingerprint(),
            Some(SourceRange {
                file: file.id,
                start: 12,
                end: 12,
            }),
            None,
            None,
            None,
            Some(DiagnosticEvidenceRef::Declaration { declaration }),
        ),
    })
    .expect("internally representable source context");
    let mut input = package_input(&fixture.package);
    input.diagnostics = vec![invalid_range];
    let package = LinkAnalysisPackage::try_new(input)
        .expect("source ranges require the complete PARC package to check");
    assert!(matches!(
        ValidatedLinkAnalysis::try_new(&fixture.complete, package),
        Err(ContractError::InvalidDiagnosticContext { .. })
    ));
}

#[test]
fn native_name_requests_and_analysis_policies_are_explicit() {
    let fixture = corpus_fixture();
    let inputs = vec![
        NativeInput::SearchNative(PathBuf::from("/native")),
        NativeInput::GroupStart,
        NativeInput::StaticLibraryName(OsString::from("repeat")),
        NativeInput::StaticLibraryName(OsString::from("repeat")),
        NativeInput::GroupEnd,
    ];
    validate_native_inputs(&inputs).expect("ordered repeated name requests are valid");
    let policy = AnalysisPolicy::strict(
        ResolutionPolicy::HermeticSearch,
        ProbePolicy::CompileOnly,
        RunnerPolicy::Unavailable,
        fixture_execution_policy(),
    )
    .expect("compile-only strict policy");
    let request = AnalysisRequest::try_new(&fixture.complete, &inputs, policy)
        .expect("typed complete-source request");
    assert_eq!(request.native_inputs(), inputs);
    assert!(AnalysisPolicy::strict(
        ResolutionPolicy::HermeticSearch,
        ProbePolicy::CompileAndRun,
        RunnerPolicy::Unavailable,
        fixture_execution_policy(),
    )
    .is_err());
    let run_policy = AnalysisPolicy::strict(
        ResolutionPolicy::HermeticSearch,
        ProbePolicy::CompileAndRun,
        RunnerPolicy::Explicit(
            RunnerCommand::try_new(
                PathBuf::from("/runner/bin/qemu-x86_64"),
                ArtifactFingerprint::from_content(b"qemu-runner"),
                vec![
                    ProbeRunnerArgument::Literal(OsString::from("--")),
                    ProbeRunnerArgument::ProbeExecutable,
                ],
            )
            .expect("exact runner identity"),
        ),
        fixture_execution_policy(),
    )
    .expect("compile-and-run requires explicit runner identity");
    assert!(matches!(run_policy.runner(), RunnerPolicy::Explicit(_)));

    let exact = AnalysisPolicy::strict(
        ResolutionPolicy::ExactPathsOnly,
        ProbePolicy::Disabled,
        RunnerPolicy::Unavailable,
        fixture_execution_policy(),
    )
    .expect("exact-only policy");
    assert!(AnalysisRequest::try_new(&fixture.complete, &inputs, exact).is_err());
}

#[cfg(unix)]
#[test]
fn codec_roundtrips_non_utf8_paths_and_framework_identity_losslessly() {
    use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};

    let source = decode_source_package(corpus::COMPLETE_SOURCE_PACKAGE_JSON)
        .expect("complete source corpus");
    let seed = evidence_seed();
    let complete = source
        .into_complete(&Selection::all_supported())
        .expect("complete source closure");
    let target = observed_target(&complete, &seed.abi_probe);
    let mut target_parts = target.parts();
    target_parts.object_format = ObjectFormat::MachO;
    target_parts.linker = LinkerFlavor::Darwin;
    target_parts.crt = CrtFlavor::Darwin;
    let target = ObservedTarget::try_new(target_parts).expect("synthetic Mach-O target evidence");
    let path = PathBuf::from(OsString::from_vec(
        b"/fixtures/Frame\xff.framework/Frame\xff".to_vec(),
    ));
    let artifact = ResolvedArtifact::try_new(ResolvedArtifactInput {
        artifact_fingerprint: ArtifactFingerprint::from_content(b"non-utf8-framework"),
        canonical_path: path.clone(),
        kind: ArtifactKind::Framework,
        resolution: ProviderResolution::Explicit,
        provenance: ProviderProvenance::User,
        observed_target: target,
    })
    .expect("non-UTF8 canonical framework path");
    let inventory = SymbolInventory::try_new(
        artifact.clone(),
        fixture_inspection(InspectionParserKind::MachO),
        Vec::new(),
        Vec::new(),
    )
    .expect("framework inventory");
    let plan = ResolvedLinkPlan::try_new(vec![LinkAtom::Framework {
        name: OsString::from_vec(b"Frame\xff".to_vec()),
        search_path: PathBuf::from("/fixtures"),
        artifact,
    }])
    .expect("resolved framework atom");
    let package = LinkAnalysisPackage::try_new(LinkAnalysisPackageInput {
        source_fingerprint: complete.source().fingerprint(),
        target_fingerprint: complete.source().target_fingerprint(),
        analysis_policy: AnalysisPolicy::strict(
            ResolutionPolicy::ExactPathsOnly,
            ProbePolicy::Disabled,
            RunnerPolicy::Unavailable,
            fixture_execution_policy(),
        )
        .expect("framework policy"),
        native_inputs: vec![NativeInput::FrameworkPath(path.clone())],
        inventories: vec![inventory],
        abi_probes: Vec::new(),
        layouts: Vec::new(),
        declaration_evidence: Vec::new(),
        resolved_link_plan: plan,
        diagnostics: Vec::new(),
    })
    .expect("minimal framework package");
    let decoded = decode_link_analysis(&encode_link_analysis(&package).expect("encode"))
        .expect("decode non-UTF8 path");
    assert_eq!(
        decoded.inventories()[0]
            .artifact()
            .canonical_path()
            .as_os_str()
            .as_bytes(),
        path.as_os_str().as_bytes()
    );
    let LinkAtom::Framework { name, .. } = &decoded.resolved_link_plan().atoms()[0] else {
        panic!("framework atom");
    };
    assert_eq!(name.as_bytes(), b"Frame\xff");
}

fn corpus_fixture() -> CorpusFixture {
    corpus_fixture_with(FixtureOptions::default())
}

#[test]
fn packaged_preservation_link_analysis_matches_checked_fixture() {
    let fixture = corpus_fixture();
    let encoded = encode_link_analysis(&fixture.package).expect("encode preservation package");
    assert_eq!(
        encoded.as_slice(),
        super::corpus::PRESERVATION_LINK_ANALYSIS_JSON
    );
    assert_eq!(
        fixture.package.fingerprint(),
        super::corpus::preservation_link_analysis_fingerprint()
    );
    assert_eq!(
        super::corpus::preservation_selection(),
        preservation_selection(&fixture.seed)
    );
    assert_eq!(
        super::corpus::decode_preservation_link_analysis().expect("decode packaged corpus"),
        fixture.package
    );
    assert!(super::corpus::validated_preservation_link_analysis(&fixture.complete).is_ok());
}

fn corpus_fixture_with(options: FixtureOptions) -> CorpusFixture {
    let source = decode_source_package(corpus::COMPLETE_SOURCE_PACKAGE_JSON)
        .expect("required complete corpus must decode");
    assert_eq!(source.fingerprint(), ledger_source_fingerprint("complete"));
    let seed = evidence_seed();
    let selection = if options.all_supported {
        Selection::all_supported()
    } else {
        preservation_selection(&seed)
    };
    let complete = source
        .into_complete(&selection)
        .expect("required complete corpus must prove selected closure");
    assert_eq!(seed.source_fingerprint, complete.source().fingerprint());
    assert_eq!(
        seed.abi_probe.bitness,
        complete.source().target().pointer_width()
    );
    let evidence_source_fingerprint = options
        .source_override
        .unwrap_or_else(|| complete.source().fingerprint());

    let analysis_policy = AnalysisPolicy::strict(
        ResolutionPolicy::ExactPathsOnly,
        ProbePolicy::CompileOnly,
        RunnerPolicy::Unavailable,
        fixture_execution_policy(),
    )
    .expect("strict corpus analysis policy");

    let observed = observed_target(&complete, &seed.abi_probe);
    let resolved_seed = seed
        .providers
        .iter()
        .find(|provider| provider.state == "resolved" && provider.provider.is_some())
        .expect("resolved seed provider");
    let provider_name = resolved_seed.provider.as_deref().expect("provider name");
    let provider_artifact = artifact(provider_name, ArtifactKind::StaticLibrary, &observed);

    let mut symbols = Vec::new();
    let mut symbol_by_declaration = BTreeMap::new();
    let mut symbol_index = 0_u64;
    for closure_entry in complete.declaration_closure() {
        let declaration = complete
            .source()
            .declaration(closure_entry.declaration())
            .expect("closure declaration");
        let Some(provider_seed) = seed
            .providers
            .iter()
            .find(|provider| provider.declaration == declaration.id)
        else {
            continue;
        };
        if provider_seed.state != "resolved" {
            continue;
        }
        let (link_name, default_kind) = match &declaration.kind {
            SourceDeclarationKind::Function(function) => {
                (&function.link_name, SymbolKind::Function)
            }
            SourceDeclarationKind::Variable(variable) if variable.thread_local => {
                (&variable.link_name, SymbolKind::ThreadLocal)
            }
            SourceDeclarationKind::Variable(variable) => (&variable.link_name, SymbolKind::Data),
            _ => continue,
        };
        assert_eq!(provider_seed.symbol, *link_name);
        let kind = options.kind_override.unwrap_or(default_kind);
        let id = ArtifactSymbolId::new(0, symbol_index);
        symbol_index += 1;
        symbols.push(
            SymbolRecord::try_new(SymbolRecordInput {
                id,
                name: link_name.clone(),
                raw_name: link_name.as_bytes().to_vec(),
                version: None,
                direction: options.direction,
                kind,
                binding: options.binding,
                visibility: options.visibility,
                decoration: SymbolDecoration::None,
                size: 16,
                address: Some(0x1000 + id.symbol_index() * 16),
                section: Some(b".text".to_vec()),
                archive_member: Some(provider_name.as_bytes().to_vec()),
            })
            .expect("seed symbol"),
        );
        symbol_by_declaration.insert(declaration.id, (id, link_name.clone(), kind));
        if options.duplicate_same_provider_symbol {
            symbols.push(
                SymbolRecord::try_new(SymbolRecordInput {
                    id: ArtifactSymbolId::new(0, symbol_index),
                    name: link_name.clone(),
                    raw_name: link_name.as_bytes().to_vec(),
                    version: Some(b"DUPLICATE_1".to_vec()),
                    direction: SymbolDirection::Exported,
                    kind,
                    binding: SymbolBinding::Global,
                    visibility: SymbolVisibility::Default,
                    decoration: SymbolDecoration::Versioned {
                        version: b"DUPLICATE_1".to_vec(),
                        is_default: false,
                    },
                    size: 16,
                    address: Some(0x2000 + symbol_index * 16),
                    section: Some(b".text".to_vec()),
                    archive_member: Some(b"duplicate-member.o".to_vec()),
                })
                .expect("same-provider duplicate candidate"),
            );
            symbol_index += 1;
        }
    }

    let mut inventory_by_provider = BTreeMap::new();
    inventory_by_provider.insert(
        provider_artifact.provider_id(),
        SymbolInventory::try_new(
            provider_artifact.clone(),
            fixture_inspection(InspectionParserKind::Archive),
            symbols.clone(),
            Vec::new(),
        )
        .expect("provider inventory"),
    );

    let mut plan_names = seed.ordered_link_inputs.clone();
    if options.include_provider_in_plan && !plan_names.iter().any(|name| name == provider_name) {
        plan_names.push(provider_name.to_owned());
    }
    if !options.include_provider_in_plan {
        plan_names.retain(|name| name != provider_name);
    }
    let mut plan_artifacts = BTreeMap::<String, ResolvedArtifact>::new();
    for input in &plan_names {
        let artifact = if input == provider_name {
            provider_artifact.clone()
        } else {
            artifact(input, artifact_kind(input), &observed)
        };
        plan_artifacts.entry(input.clone()).or_insert(artifact);
    }
    for artifact in plan_artifacts.values() {
        inventory_by_provider
            .entry(artifact.provider_id())
            .or_insert_with(|| {
                SymbolInventory::try_new(
                    artifact.clone(),
                    fixture_inspection(if artifact.kind() == ArtifactKind::StaticLibrary {
                        InspectionParserKind::Archive
                    } else {
                        InspectionParserKind::Elf
                    }),
                    Vec::new(),
                    Vec::new(),
                )
                .expect("plan inventory")
            });
    }

    let mut duplicate_artifact = None;
    if options.duplicate_visible_provider {
        let duplicate = artifact("libduplicate.a", ArtifactKind::StaticLibrary, &observed);
        inventory_by_provider.insert(
            duplicate.provider_id(),
            SymbolInventory::try_new(
                duplicate.clone(),
                fixture_inspection(InspectionParserKind::Archive),
                symbols,
                Vec::new(),
            )
            .expect("duplicate visible inventory"),
        );
        duplicate_artifact = Some(duplicate);
    }

    let mut plan_atoms = plan_names
        .iter()
        .map(|input| link_atom(plan_artifacts.get(input).expect("resolved input").clone()))
        .collect::<Vec<_>>();
    if let Some(duplicate) = duplicate_artifact {
        plan_atoms.push(LinkAtom::StaticLibrary(duplicate));
    }
    let plan = ResolvedLinkPlan::try_new(plan_atoms).expect("ordered corpus plan");
    let native_inputs = plan_names
        .iter()
        .map(|name| native_path_input(name, artifact_kind(name)))
        .collect::<Vec<_>>();

    let mut probe_subjects =
        seed.record_layouts
            .iter()
            .map(|layout| ProbeSubject::RecordLayout {
                declaration: layout.declaration,
            })
            .chain(seed.enum_representations.iter().map(|layout| {
                ProbeSubject::EnumRepresentation {
                    declaration: layout.declaration,
                }
            }))
            .collect::<Vec<_>>();
    if !options.omit_callable_probe_subject {
        probe_subjects.extend(complete.declaration_closure().iter().filter_map(|entry| {
            let declaration = complete.source().declaration(entry.declaration())?;
            (declaration.linkage == Linkage::External
                && seed.providers.iter().any(|provider| {
                    provider.declaration == declaration.id && provider.state == "resolved"
                })
                && matches!(declaration.kind, SourceDeclarationKind::Function(_)))
            .then_some(ProbeSubject::CallableAbi {
                declaration: declaration.id,
            })
        }));
    }
    let compiler = if options.probe_compiler_mismatch {
        mismatched_compiler(complete.source().target().compiler())
    } else {
        complete.source().target().compiler().clone()
    };
    let subject_outcomes = probe_subjects
        .iter()
        .copied()
        .map(|subject| {
            let label = probe_subject_label(subject);
            ProbeSubjectOutcome::try_new(
                subject,
                if options.reject_probe_outcome {
                    ProbeSubjectStatus::Rejected {
                        code: LincCode::try_new("LINC-E4201").expect("stable rejection code"),
                    }
                } else {
                    ProbeSubjectStatus::Verified {
                        evidence_fingerprint: ContentFingerprint::from_content(label.as_bytes()),
                    }
                },
            )
            .expect("verified probe outcome")
        })
        .collect();
    let probe = AbiProbeEvidence::try_new(AbiProbeEvidenceInput {
        source_fingerprint: evidence_source_fingerprint,
        target_fingerprint: complete.source().target_fingerprint(),
        compiler,
        compiler_executable: PathBuf::from("/toolchain/bin/gcc"),
        compiler_arguments: complete
            .source()
            .target()
            .abi_flags()
            .iter()
            .map(|flag| ProbeCompilerArgument::Literal(OsString::from(flag.as_str())))
            .chain([
                ProbeCompilerArgument::Literal(OsString::from("-c")),
                ProbeCompilerArgument::ProbeSource,
                ProbeCompilerArgument::Literal(OsString::from("-o")),
                ProbeCompilerArgument::OutputArtifact,
            ])
            .collect(),
        abi_flags: complete.source().target().abi_flags().to_vec(),
        probe_source_fingerprint: ContentFingerprint::from_content(
            corpus::PRESERVATION_HEADER.as_bytes(),
        ),
        subjects: probe_subjects,
        method: ProbeMethod::CompileTimeAssertion,
        execution_policy: analysis_policy.probe_execution().clone(),
        compile_result: ProbeProcessResult::new(
            ProbeProcessStatus::Exited { code: 0 },
            ContentFingerprint::from_content(b"linc-corpus-probe-stdout"),
            ContentFingerprint::from_content(b"linc-corpus-probe-stderr"),
            Some(ArtifactFingerprint::from_content(
                b"linc-corpus-probe-object",
            )),
        ),
        runner: ProbeRunnerEvidence::NotExecuted,
        execution_result: None,
        subject_outcomes,
    })
    .expect("typed GCC ABI probe evidence");

    let confidence = if options.inferred_layout {
        EvidenceConfidence::Inferred
    } else {
        EvidenceConfidence::Measured
    };
    let enum_child = complete
        .source()
        .declarations()
        .iter()
        .find_map(|declaration| match &declaration.kind {
            SourceDeclarationKind::Enum(enumeration) => {
                enumeration.variants.first().map(|variant| variant.id)
            }
            _ => None,
        });
    let mut layouts = Vec::new();
    for layout in &seed.record_layouts {
        let fields = layout
            .fields
            .iter()
            .map(|field| {
                let child = if options.foreign_record_child {
                    enum_child.expect("foreign enum child")
                } else {
                    field.child
                };
                FieldLayoutEvidence::try_new(
                    child,
                    field.offset_bits,
                    field.size_bits,
                    field.alignment_bits,
                )
                .expect("seed field layout")
            })
            .collect();
        layouts.push(LayoutEvidence::Record(
            RecordLayoutEvidence::try_new(
                layout.declaration,
                evidence_source_fingerprint,
                complete.source().target_fingerprint(),
                layout.size_bits,
                layout.alignment_bits,
                fields,
                probe.id(),
                if options.object_metadata_layout {
                    EvidenceSource::ObjectMetadata
                } else {
                    EvidenceSource::CompilerProbe
                },
                confidence,
            )
            .expect("seed record layout"),
        ));
    }
    for layout in &seed.enum_representations {
        layouts.push(LayoutEvidence::Enum(
            EnumLayoutEvidence::try_new(
                layout.declaration,
                evidence_source_fingerprint,
                complete.source().target_fingerprint(),
                layout.storage_bits,
                layout.alignment_bits,
                layout.signedness,
                enum_variants(&complete, layout),
                probe.id(),
                if options.object_metadata_layout {
                    EvidenceSource::ObjectMetadata
                } else {
                    EvidenceSource::CompilerProbe
                },
                confidence,
            )
            .expect("seed enum layout"),
        ));
    }
    let layout_assessment: BTreeMap<_, _> = layouts
        .iter()
        .map(|layout| (layout.declaration(), (layout.confidence(), layout.probe())))
        .collect();

    let mut declaration_evidence = Vec::new();
    for closure_entry in complete.declaration_closure() {
        let declaration = complete
            .source()
            .declaration(closure_entry.declaration())
            .expect("closure declaration");
        let provider_seed = seed
            .providers
            .iter()
            .find(|provider| provider.declaration == declaration.id);
        let (provider, symbol, callable_abi) = match (&declaration.kind, provider_seed) {
            (SourceDeclarationKind::Function(function), Some(provider_seed))
                if declaration.linkage == Linkage::External
                    && provider_seed.state == "resolved" =>
            {
                let (id, name, kind) = symbol_by_declaration
                    .get(&declaration.id)
                    .expect("function symbol");
                (
                    ProviderAssessment::Resolved {
                        provider: provider_artifact.provider_id(),
                        artifact_fingerprint: provider_artifact.artifact_fingerprint(),
                    },
                    SymbolAssessment::Exact {
                        symbol: SymbolReference::new(provider_artifact.provider_id(), *id),
                        expected_name: function.link_name.clone(),
                        actual_name: name.clone(),
                        kind: *kind,
                        decoration: SymbolDecoration::None,
                    },
                    CallableAbiAssessment::Confirmed {
                        calling_convention: function.calling_convention.clone(),
                        confidence: EvidenceConfidence::Measured,
                        probe: probe.id(),
                    },
                )
            }
            (SourceDeclarationKind::Variable(variable), Some(provider_seed))
                if declaration.linkage == Linkage::External
                    && provider_seed.state == "resolved" =>
            {
                let (id, name, kind) = symbol_by_declaration
                    .get(&declaration.id)
                    .expect("variable symbol");
                (
                    ProviderAssessment::Resolved {
                        provider: provider_artifact.provider_id(),
                        artifact_fingerprint: provider_artifact.artifact_fingerprint(),
                    },
                    SymbolAssessment::Exact {
                        symbol: SymbolReference::new(provider_artifact.provider_id(), *id),
                        expected_name: variable.link_name.clone(),
                        actual_name: name.clone(),
                        kind: *kind,
                        decoration: SymbolDecoration::None,
                    },
                    CallableAbiAssessment::NotApplicable,
                )
            }
            (SourceDeclarationKind::Function(function), Some(provider_seed))
                if declaration.linkage == Linkage::External
                    && provider_seed.state == "unresolved" =>
            {
                (
                    ProviderAssessment::Unresolved,
                    SymbolAssessment::Missing {
                        expected_name: function.link_name.clone(),
                    },
                    CallableAbiAssessment::Missing,
                )
            }
            (SourceDeclarationKind::Variable(variable), Some(provider_seed))
                if declaration.linkage == Linkage::External
                    && provider_seed.state == "unresolved" =>
            {
                (
                    ProviderAssessment::Unresolved,
                    SymbolAssessment::Missing {
                        expected_name: variable.link_name.clone(),
                    },
                    CallableAbiAssessment::NotApplicable,
                )
            }
            _ => (
                ProviderAssessment::NotRequired,
                SymbolAssessment::NotRequired,
                CallableAbiAssessment::NotApplicable,
            ),
        };
        let layout = if closure_entry.requirement() == ClosureRequirement::Definition {
            layout_assessment
                .get(&declaration.id)
                .copied()
                .map_or(LayoutAssessment::NotRequired, |(confidence, probe)| {
                    LayoutAssessment::Available { confidence, probe }
                })
        } else {
            LayoutAssessment::NotRequired
        };
        declaration_evidence.push(
            DeclarationEvidence::try_new(DeclarationEvidenceInput {
                declaration: declaration.id,
                source_fingerprint: evidence_source_fingerprint,
                target_fingerprint: complete.source().target_fingerprint(),
                provider,
                symbol,
                layout,
                callable_abi,
            })
            .expect("declaration evidence dimensions"),
        );
    }

    let package = LinkAnalysisPackage::try_new(LinkAnalysisPackageInput {
        source_fingerprint: evidence_source_fingerprint,
        target_fingerprint: complete.source().target_fingerprint(),
        analysis_policy,
        native_inputs,
        inventories: inventory_by_provider.into_values().collect(),
        abi_probes: vec![probe],
        layouts,
        declaration_evidence,
        resolved_link_plan: plan,
        diagnostics: Vec::new(),
    })
    .expect("internally valid corpus package");

    CorpusFixture {
        complete,
        package,
        seed,
    }
}

fn preservation_selection(seed: &EvidenceSeed) -> Selection {
    Selection::only(
        seed.providers
            .iter()
            .filter(|provider| provider.state == "resolved")
            .map(|provider| provider.declaration)
            .chain(seed.record_layouts.iter().map(|layout| layout.declaration))
            .chain(
                seed.enum_representations
                    .iter()
                    .map(|layout| layout.declaration),
            ),
    )
    .expect("nonempty distinct preservation roots")
}

fn package_input(package: &LinkAnalysisPackage) -> LinkAnalysisPackageInput {
    LinkAnalysisPackageInput {
        source_fingerprint: package.source_fingerprint(),
        target_fingerprint: package.target_fingerprint(),
        analysis_policy: package.analysis_policy().clone(),
        native_inputs: package.native_inputs().to_vec(),
        inventories: package.inventories().to_vec(),
        abi_probes: package.abi_probes().to_vec(),
        layouts: package.layouts().to_vec(),
        declaration_evidence: package.declaration_evidence().to_vec(),
        resolved_link_plan: package.resolved_link_plan().clone(),
        diagnostics: package.diagnostics().to_vec(),
    }
}

fn inventory_named<'a>(package: &'a LinkAnalysisPackage, name: &str) -> &'a SymbolInventory {
    inventory_named_in(package.inventories(), name)
}

fn inventory_named_in<'a>(inventories: &'a [SymbolInventory], name: &str) -> &'a SymbolInventory {
    inventories
        .iter()
        .find(|inventory| inventory.artifact().canonical_path().file_name() == Some(name.as_ref()))
        .unwrap_or_else(|| panic!("missing corpus inventory {name:?}"))
}

fn replace_inventory(inventories: &mut [SymbolInventory], replacement: SymbolInventory) {
    let provider = replacement.artifact().provider_id();
    let slot = inventories
        .iter_mut()
        .find(|inventory| inventory.artifact().provider_id() == provider)
        .unwrap_or_else(|| panic!("missing replacement provider {provider}"));
    *slot = replacement;
}

fn rebuild_inventory(
    original: SymbolInventory,
    artifact: ResolvedArtifact,
    dependency_edges: Vec<DependencyEdge>,
) -> SymbolInventory {
    SymbolInventory::try_new(
        artifact,
        original.inspection().clone(),
        original.symbols().to_vec(),
        dependency_edges,
    )
    .expect("rebuilt checked inventory")
}

fn artifact_with_resolution(
    artifact: &ResolvedArtifact,
    resolution: ProviderResolution,
) -> ResolvedArtifact {
    let mut input = artifact.input();
    input.resolution = resolution;
    let rebuilt = ResolvedArtifact::try_new(input).expect("rebuilt resolved artifact");
    assert_eq!(rebuilt.provider_id(), artifact.provider_id());
    rebuilt
}

fn replace_plan_artifact(
    plan: &ResolvedLinkPlan,
    replacement: ResolvedArtifact,
) -> ResolvedLinkPlan {
    let provider = replacement.provider_id();
    let atoms = plan
        .atoms()
        .iter()
        .cloned()
        .map(|atom| {
            if atom
                .artifact()
                .is_some_and(|artifact| artifact.provider_id() == provider)
            {
                link_atom(replacement.clone())
            } else {
                atom
            }
        })
        .collect();
    ResolvedLinkPlan::try_new(atoms).expect("plan with replacement artifact")
}

fn package_input_with_dependency(
    fixture: &CorpusFixture,
    parent_name: &str,
    child_name: &str,
) -> LinkAnalysisPackageInput {
    let parent = inventory_named(&fixture.package, parent_name).clone();
    let child = inventory_named(&fixture.package, child_name).clone();
    let child_dependency = artifact_with_resolution(
        child.artifact(),
        ProviderResolution::Dependency {
            parent: parent.artifact().provider_id(),
        },
    );
    let parent_with_edge = rebuild_inventory(
        parent.clone(),
        parent.artifact().clone(),
        vec![DependencyEdge::try_new(
            OsString::from(child_name),
            Some(child.artifact().provider_id()),
            DependencyProvenance::ArchiveDirective,
        )
        .expect("resolved dependency edge")],
    );
    let child_with_parent = rebuild_inventory(child, child_dependency.clone(), Vec::new());

    let mut input = package_input(&fixture.package);
    replace_inventory(&mut input.inventories, parent_with_edge);
    replace_inventory(&mut input.inventories, child_with_parent);
    input.resolved_link_plan = replace_plan_artifact(&input.resolved_link_plan, child_dependency);
    input
}

fn fixture_inspection(parser: InspectionParserKind) -> InspectionProvenance {
    let parser_kinds = if parser == InspectionParserKind::Archive {
        vec![InspectionParserKind::Archive, InspectionParserKind::Elf]
    } else {
        vec![parser]
    };
    InspectionProvenance::try_new(
        InspectionToolIdentity::try_new(
            InspectionToolKind::ObjectCrate,
            "0.36.7".to_owned(),
            ContentFingerprint::from_content(b"object-crate-0.36.7"),
        )
        .expect("inspection tool"),
        parser_kinds
            .into_iter()
            .map(|kind| {
                InspectionParserIdentity::try_new(
                    kind,
                    "linc-contract-parser-v2".to_owned(),
                    ContentFingerprint::from_content(b"linc-contract-parser-v2"),
                )
                .expect("inspection parser")
            })
            .collect(),
    )
    .expect("inspection provenance")
}

fn fixture_execution_policy() -> ProbeExecutionPolicy {
    ProbeExecutionPolicy::try_new(
        PathBuf::from("/tmp"),
        ProbeEnvironmentIdentity::try_new(ProbeEnvironmentPolicy::Empty, Vec::new())
            .expect("empty captured environment"),
        ProbeResourceLimits::try_new(30_000, 512 * 1024 * 1024, 16 * 1024 * 1024, 16)
            .expect("bounded probe resources"),
    )
    .expect("explicit deterministic probe temp parent")
}

fn native_path_input(name: &str, kind: ArtifactKind) -> NativeInput {
    let path = PathBuf::from(format!("/fixtures/{name}"));
    match kind {
        ArtifactKind::Object => NativeInput::ObjectPath(path),
        ArtifactKind::StaticLibrary => NativeInput::StaticLibraryPath(path),
        ArtifactKind::DynamicLibrary => NativeInput::DynamicLibraryPath(path),
        ArtifactKind::ImportLibrary => NativeInput::ImportLibraryPath(path),
        ArtifactKind::Framework => NativeInput::FrameworkPath(path),
    }
}

fn mismatched_compiler(compiler: &CompilerIdentity) -> CompilerIdentity {
    CompilerIdentity::try_new(
        compiler.family(),
        compiler.logical_executable().to_owned(),
        compiler.executable_content(),
        ContentFingerprint::from_content(b"mismatched-compiler-version-output"),
        compiler.reported_target().to_owned(),
        format!("{} mismatch", compiler.version()),
    )
    .expect("syntactically valid mismatched compiler")
}

fn probe_subject_label(subject: ProbeSubject) -> String {
    match subject {
        ProbeSubject::RecordLayout { declaration } => {
            format!("linc-corpus-record-layout:{declaration}")
        }
        ProbeSubject::EnumRepresentation { declaration } => {
            format!("linc-corpus-enum-representation:{declaration}")
        }
        ProbeSubject::CallableAbi { declaration } => {
            format!("linc-corpus-callable-abi:{declaration}")
        }
    }
}

fn enum_variants(
    complete: &CompleteSourcePackage,
    seed: &SeedEnumRepresentation,
) -> Vec<EnumVariantEvidence> {
    let declaration = complete
        .source()
        .declaration(seed.declaration)
        .expect("seed enum declaration");
    let SourceDeclarationKind::Enum(enumeration) = &declaration.kind else {
        panic!("enum representation must reference an enum");
    };
    if !seed.variants.is_empty() {
        return seed
            .variants
            .iter()
            .map(|variant| EnumVariantEvidence::new(variant.child, variant.value))
            .collect();
    }
    enumeration
        .variants
        .iter()
        .map(|variant| {
            let EnumValue::Evaluated { value } = &variant.value else {
                panic!("complete corpus enum values must be exact");
            };
            EnumVariantEvidence::new(variant.id, *value)
        })
        .collect()
}

fn observed_target(complete: &CompleteSourcePackage, probe: &SeedAbiProbe) -> ObservedTarget {
    let abi = match probe.abi.as_str() {
        "sysv64" => NativeAbi::SysV64,
        other => panic!("unsupported corpus ABI seed {other:?}"),
    };
    let linker = match probe.linker_flavor.as_str() {
        "gnu" => LinkerFlavor::Gnu,
        other => panic!("unsupported corpus linker seed {other:?}"),
    };
    let crt = match probe.crt_flavor.as_str() {
        "glibc" => CrtFlavor::Glibc,
        other => panic!("unsupported corpus CRT seed {other:?}"),
    };
    ObservedTarget::for_target(complete.source().target(), abi, linker, crt)
}

fn artifact(name: &str, kind: ArtifactKind, target: &ObservedTarget) -> ResolvedArtifact {
    ResolvedArtifact::try_new(ResolvedArtifactInput {
        artifact_fingerprint: ArtifactFingerprint::from_content(
            format!("fixture:{name}").as_bytes(),
        ),
        canonical_path: PathBuf::from(format!("/fixtures/{name}")),
        kind,
        resolution: ProviderResolution::Explicit,
        provenance: ProviderProvenance::User,
        observed_target: target.clone(),
    })
    .expect("fixture artifact")
}

fn artifact_kind(name: &str) -> ArtifactKind {
    if name.ends_with(".a") {
        ArtifactKind::StaticLibrary
    } else if name.ends_with(".so") {
        ArtifactKind::DynamicLibrary
    } else {
        panic!("unrecognized corpus link input {name:?}")
    }
}

fn link_atom(artifact: ResolvedArtifact) -> LinkAtom {
    match artifact.kind() {
        ArtifactKind::StaticLibrary => LinkAtom::StaticLibrary(artifact),
        ArtifactKind::DynamicLibrary => LinkAtom::DynamicLibrary(artifact),
        kind => panic!("unexpected corpus link artifact kind {kind:?}"),
    }
}

fn link_atom_file_name(atom: &LinkAtom) -> Option<String> {
    atom.artifact().and_then(|artifact| {
        artifact
            .canonical_path()
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
    })
}

fn evidence_seed() -> EvidenceSeed {
    let ledger: serde_json::Value =
        serde_json::from_str(corpus::PRESERVATION_LEDGER_JSON).expect("required ledger JSON");
    serde_json::from_value(
        ledger
            .get("linc_evidence_seed")
            .cloned()
            .expect("ledger must carry LINC evidence seed"),
    )
    .expect("strict LINC evidence seed shape")
}

fn ledger_source_fingerprint(case_name: &str) -> SourceFingerprint {
    let ledger: serde_json::Value =
        serde_json::from_str(corpus::PRESERVATION_LEDGER_JSON).expect("required ledger JSON");
    ledger["cases"]
        .as_array()
        .expect("ledger cases")
        .iter()
        .find(|case| case["name"].as_str() == Some(case_name))
        .and_then(|case| case["source_fingerprint"].as_str())
        .expect("ledger case source fingerprint")
        .parse()
        .expect("canonical ledger source fingerprint")
}

fn ledger_target_fingerprint() -> TargetFingerprint {
    let ledger: serde_json::Value =
        serde_json::from_str(corpus::PRESERVATION_LEDGER_JSON).expect("required ledger JSON");
    ledger["target_fingerprint"]
        .as_str()
        .expect("ledger target fingerprint")
        .parse()
        .expect("canonical ledger target fingerprint")
}
