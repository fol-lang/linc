#![cfg(all(feature = "native-inspection", target_os = "linux"))]

use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
};

use linc::{
    contract::{
        corpus as linc_corpus, AnalysisPolicy, AnalysisRequest, ArtifactKind,
        CallableAbiAssessment, CrtFlavor, EvidenceConfidence, LayoutAssessment, LinkAtom,
        LinkerFlavor, NativeAbi, NativeInput, ProbeCompilerArgument, ProbeEnvironmentIdentity,
        ProbeEnvironmentPolicy, ProbeExecutionPolicy, ProbeMethod, ProbePolicy,
        ProbeResourceLimits, ProbeRunnerArgument, ProbeSubject, ProviderProvenance,
        ProviderResolution, ResolutionPolicy, RunnerPolicy, SymbolBinding, SymbolDecoration,
        SymbolDirection, SymbolKind, SymbolVisibility, WeakSymbolPolicy,
    },
    native::{
        AbiDimension, AbiShapeEvidence, CertificationToolchain, EnvironmentSetting,
        InspectionLimits, LibraryPreference, NativeAnalysisInput, NativeAnalyzer,
        NativeDeclarationRequest, NativeError, NativeInspector, NativeResolver, ProbeExpectation,
        ProbeProgram, ProbeRejectionKind, ProbeRequest, ProbeRunOutcome, ProbeRunner,
        ResolverConfiguration, ReturnConvention, RunnerSpec, StrictDeclarationRequest,
        StrictEvidenceValidator, ValuePassing,
    },
};
use parc::contract::{
    corpus as parc_corpus, decode_source_package, Architecture, CallingConvention, CompilerFamily,
    CompilerIdentity, CompleteSourcePackage, ContentFingerprint, DeclarationId, ExtensionFamily,
    ExtensionProfile, Selection, SourceDeclarationKind, SourceFingerprint, SourcePackage,
    SourcePackageInput, TargetSpec, TargetSpecParts,
};
use parc::scan::{scan_headers, PathMapping, PathMappingRule, PreprocessorMode, ScanConfig};
use tempfile::TempDir;

const PARC_OPEN: &str = "pdecl1_524bcccd395cfaad5d0697f01bc545663e82eaad03be1e515beeb81933f5b37d";

struct Fixtures {
    root: TempDir,
    compiler: PathBuf,
    cross_compiler: PathBuf,
    object: PathBuf,
    first_archive: PathBuf,
    repeat_archive: PathBuf,
    dependency: PathBuf,
    provider: PathBuf,
    versioned: PathBuf,
    global_one: PathBuf,
    global_two: PathBuf,
    hidden: PathBuf,
    weak: PathBuf,
    local: PathBuf,
    imported: PathBuf,
}

impl Fixtures {
    fn build() -> Self {
        let root = tempfile::Builder::new()
            .prefix("linc-native-fixtures-")
            .tempdir()
            .expect("create fixture directory");
        let compiler = required_tool("LINC_TEST_CC");
        let archive_tool = required_tool("LINC_TEST_AR");
        let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/native-fixtures");
        let cross_compiler = root.path().join("fake-aarch64-cc");
        compile_executable(
            &compiler,
            &source_root.join("fake_cross_compiler.c"),
            &cross_compiler,
        );

        let object = root.path().join("explicit-object.o");
        compile_object(&compiler, &source_root.join("object.c"), &object, &[]);
        let first_member = root.path().join("first-member.o");
        compile_object(
            &compiler,
            &source_root.join("archive_first.c"),
            &first_member,
            &[],
        );
        let repeat_member = root.path().join("repeat-member.o");
        compile_object(
            &compiler,
            &source_root.join("archive_repeat.c"),
            &repeat_member,
            &[],
        );
        let first_archive = root.path().join("libfirst.a");
        make_archive(&archive_tool, &first_archive, &[&first_member]);
        let repeat_archive = root.path().join("librepeat.a");
        make_archive(&archive_tool, &repeat_archive, &[&repeat_member]);

        let dependency_object = root.path().join("dependency.o");
        compile_object(
            &compiler,
            &source_root.join("dependency.c"),
            &dependency_object,
            &["-fPIC"],
        );
        let dependency = root.path().join("libdependency.so.1");
        link_shared(
            &compiler,
            &[&dependency_object],
            &dependency,
            &["-Wl,-soname,libdependency.so.1"],
        );

        let provider_object = root.path().join("provider.o");
        compile_object(
            &compiler,
            &source_root.join("provider.c"),
            &provider_object,
            &["-fPIC"],
        );
        let provider = root.path().join("libprovider.so");
        link_shared(&compiler, &[&provider_object, &dependency], &provider, &[]);

        let versioned_object = root.path().join("versioned.o");
        compile_object(
            &compiler,
            &source_root.join("versioned.c"),
            &versioned_object,
            &["-fPIC"],
        );
        let versioned = root.path().join("libversioned.so");
        let version_option = format!(
            "-Wl,--version-script={}",
            source_root.join("version.map").display()
        );
        link_shared(
            &compiler,
            &[&versioned_object],
            &versioned,
            &[&version_option],
        );

        let global_one = build_shared_source(
            &compiler,
            root.path(),
            &source_root.join("parc_open_global.c"),
            "libglobal_one.so",
        );
        let global_two = build_shared_source(
            &compiler,
            root.path(),
            &source_root.join("parc_open_global.c"),
            "libglobal_two.so",
        );
        let hidden = build_shared_source(
            &compiler,
            root.path(),
            &source_root.join("parc_open_hidden.c"),
            "libhidden.so",
        );
        let weak = build_shared_source(
            &compiler,
            root.path(),
            &source_root.join("parc_open_weak.c"),
            "libweak.so",
        );
        let local = build_shared_source(
            &compiler,
            root.path(),
            &source_root.join("parc_open_local.c"),
            "liblocal.so",
        );
        let imported = build_shared_source(
            &compiler,
            root.path(),
            &source_root.join("parc_open_import.c"),
            "libimported.so",
        );

        Self {
            root,
            compiler,
            cross_compiler,
            object,
            first_archive,
            repeat_archive,
            dependency,
            provider,
            versioned,
            global_one,
            global_two,
            hidden,
            weak,
            local,
            imported,
        }
    }
}

#[test]
fn real_elf_inventory_and_resolution_preserve_all_required_evidence() {
    let fixtures = Fixtures::build();
    let complete = complete_source();
    let root = canonical(fixtures.root.path());
    let inputs = vec![
        NativeInput::SearchNative(root),
        NativeInput::ObjectPath(canonical(&fixtures.object)),
        NativeInput::GroupStart,
        NativeInput::StaticLibraryName(OsString::from("first")),
        NativeInput::StaticLibraryName(OsString::from("repeat")),
        NativeInput::StaticLibraryName(OsString::from("repeat")),
        NativeInput::GroupEnd,
        NativeInput::DynamicLibraryName(OsString::from("provider")),
        NativeInput::DynamicLibraryPath(canonical(&fixtures.versioned)),
    ];
    let resolution = resolve(
        &complete,
        &inputs,
        ResolutionPolicy::HermeticSearch,
        LibraryPreference::DynamicOnly,
        fixtures.root.path(),
    )
    .expect("resolve real ELF fixtures");
    let repeated = resolution
        .plan()
        .atoms()
        .iter()
        .filter_map(|atom| atom.artifact())
        .filter_map(|artifact| artifact.canonical_path().file_name())
        .collect::<Vec<_>>();
    assert_eq!(
        repeated,
        [
            OsStr::new("explicit-object.o"),
            OsStr::new("libfirst.a"),
            OsStr::new("librepeat.a"),
            OsStr::new("librepeat.a"),
            OsStr::new("libprovider.so"),
            OsStr::new("libversioned.so"),
            OsStr::new("libdependency.so.1"),
        ]
    );
    assert!(matches!(resolution.plan().atoms()[2], LinkAtom::GroupStart));
    assert!(matches!(resolution.plan().atoms()[6], LinkAtom::GroupEnd));

    let provider = inventory_for(&resolution, "libprovider.so");
    let target = provider.artifact().observed_target();
    assert_eq!(target.architecture(), Architecture::X86_64);
    assert_eq!(target.object_format(), parc::contract::ObjectFormat::Elf);
    assert_eq!(target.pointer_width(), 64);
    assert_eq!(target.endian(), parc::contract::Endian::Little);
    assert_eq!(target.abi(), NativeAbi::SysV64);
    assert_eq!(target.linker(), LinkerFlavor::Gnu);
    assert_eq!(target.crt(), CrtFlavor::Glibc);
    assert_eq!(
        provider.inspection().tool().version(),
        "linc-native-inspection-v1/object-0.37.3"
    );
    assert!(provider
        .inspection()
        .parsers()
        .iter()
        .any(|parser| parser.kind() == linc::contract::InspectionParserKind::Elf));

    let exported = symbol(provider, "provider_api");
    assert_eq!(exported.raw_name(), b"provider_api");
    assert_eq!(exported.direction(), SymbolDirection::Exported);
    assert_eq!(exported.binding(), SymbolBinding::Global);
    assert_eq!(exported.visibility(), SymbolVisibility::Default);
    assert_eq!(exported.kind(), SymbolKind::Function);
    assert!(exported.section().is_some());
    assert!(exported.archive_member().is_none());
    let imported = symbol(provider, "dependency_api");
    assert_eq!(imported.direction(), SymbolDirection::Imported);
    assert_eq!(provider.dependency_edges().len(), 1);
    assert_eq!(
        provider.dependency_edges()[0].requested(),
        OsStr::new("libdependency.so.1")
    );
    assert_eq!(
        provider.dependency_edges()[0].provenance(),
        linc::contract::DependencyProvenance::DynamicTable
    );
    assert!(provider.dependency_edges()[0].provider().is_some());

    let archive = inventory_for(&resolution, "libfirst.a");
    let archive_symbol = symbol(archive, "archive_first");
    assert_eq!(
        archive_symbol.archive_member(),
        Some(&b"first-member.o"[..])
    );
    assert!(archive
        .inspection()
        .parsers()
        .iter()
        .any(|parser| parser.kind() == linc::contract::InspectionParserKind::Archive));
    assert_eq!(
        canonical(&fixtures.repeat_archive),
        inventory_for(&resolution, "librepeat.a")
            .artifact()
            .canonical_path()
    );
    assert_eq!(
        symbol(inventory_for(&resolution, "librepeat.a"), "archive_repeat").kind(),
        SymbolKind::Function
    );

    let versioned = inventory_for(&resolution, "libversioned.so");
    let versioned_symbol = symbol(versioned, "versioned_api");
    assert_eq!(versioned_symbol.version(), Some(&b"LINC_1.0"[..]));
    assert!(matches!(
        versioned_symbol.decoration(),
        SymbolDecoration::Versioned {
            version,
            is_default: true
        } if version == b"LINC_1.0"
    ));

    let second = resolve(
        &complete,
        &inputs,
        ResolutionPolicy::HermeticSearch,
        LibraryPreference::DynamicOnly,
        fixtures.root.path(),
    )
    .expect("repeat resolution");
    assert_eq!(resolution, second);
}

#[test]
fn resolver_rejects_ambiguity_wrong_target_wrong_format_and_unbalanced_groups() {
    let fixtures = Fixtures::build();
    let complete = complete_source();
    let second_root = tempfile::Builder::new()
        .prefix("linc-ambiguous-")
        .tempdir()
        .unwrap();
    fs::copy(
        &fixtures.first_archive,
        second_root.path().join("libfirst.a"),
    )
    .unwrap();
    let ambiguous = vec![
        NativeInput::SearchNative(canonical(fixtures.root.path())),
        NativeInput::SearchNative(canonical(second_root.path())),
        NativeInput::StaticLibraryName(OsString::from("first")),
    ];
    let error = resolve(
        &complete,
        &ambiguous,
        ResolutionPolicy::HermeticSearch,
        LibraryPreference::StaticOnly,
        fixtures.root.path(),
    )
    .unwrap_err();
    assert!(matches!(error, NativeError::AmbiguousProvider { .. }));

    let bytes = fs::read(&fixtures.object).unwrap();
    let wrong_machine = fixtures.root.path().join("wrong-machine.o");
    let mut machine_bytes = bytes.clone();
    machine_bytes[18..20].copy_from_slice(&183_u16.to_le_bytes());
    fs::write(&wrong_machine, machine_bytes).unwrap();
    let inspector = NativeInspector::default();
    let error = inspector
        .inspect(
            &wrong_machine,
            ArtifactKind::Object,
            ProviderResolution::Explicit,
            ProviderProvenance::User,
            complete.source().target(),
        )
        .unwrap_err();
    assert!(matches!(error, NativeError::TargetMismatch { .. }));

    let wrong_format = fixtures.root.path().join("wrong-format.bin");
    let mut format_bytes = bytes;
    format_bytes[..4].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
    fs::write(&wrong_format, format_bytes).unwrap();
    let error = inspector
        .inspect(
            &wrong_format,
            ArtifactKind::Object,
            ProviderResolution::Explicit,
            ProviderProvenance::User,
            complete.source().target(),
        )
        .unwrap_err();
    assert!(matches!(error, NativeError::UnsupportedArtifact { .. }));

    let policy = analysis_policy(ResolutionPolicy::HermeticSearch, fixtures.root.path());
    assert!(AnalysisRequest::try_new(&complete, &[NativeInput::GroupEnd], policy).is_err());
}

#[test]
fn exact_resolution_binds_only_explicit_transitive_providers() {
    let fixtures = Fixtures::build();
    let complete = complete_source();
    let inputs = vec![
        NativeInput::DynamicLibraryPath(canonical(&fixtures.provider)),
        NativeInput::DynamicLibraryPath(canonical(&fixtures.dependency)),
    ];
    let resolution = resolve(
        &complete,
        &inputs,
        ResolutionPolicy::ExactPathsOnly,
        LibraryPreference::DynamicOnly,
        fixtures.root.path(),
    )
    .expect("explicit dependency follows its parent");
    let provider = inventory_for(&resolution, "libprovider.so");
    assert_eq!(provider.dependency_edges().len(), 1);
    assert_eq!(
        provider.dependency_edges()[0].provider(),
        Some(
            inventory_for(&resolution, "libdependency.so.1")
                .artifact()
                .provider_id()
        )
    );

    let error = resolve(
        &complete,
        &[NativeInput::DynamicLibraryPath(canonical(
            &fixtures.provider,
        ))],
        ResolutionPolicy::ExactPathsOnly,
        LibraryPreference::DynamicOnly,
        fixtures.root.path(),
    )
    .unwrap_err();
    assert_eq!(error.code(), "LINC-E3010");

    let reversed = vec![
        NativeInput::DynamicLibraryPath(canonical(&fixtures.dependency)),
        NativeInput::DynamicLibraryPath(canonical(&fixtures.provider)),
    ];
    let error = resolve(
        &complete,
        &reversed,
        ResolutionPolicy::ExactPathsOnly,
        LibraryPreference::DynamicOnly,
        fixtures.root.path(),
    )
    .unwrap_err();
    assert_eq!(error.code(), "LINC-E3014");
}

#[test]
fn parser_boundaries_and_relocated_artifact_identity_are_deterministic() {
    let fixtures = Fixtures::build();
    let complete = complete_source();
    let inspector = NativeInspector::new(InspectionLimits {
        max_artifact_bytes: 16 * 1024 * 1024,
        ..InspectionLimits::default()
    })
    .unwrap();
    let bytes = fs::read(&fixtures.object).unwrap();
    for length in 0..bytes.len().min(96) {
        let path = fixtures.root.path().join(format!("truncated-{length}.o"));
        fs::write(&path, &bytes[..length]).unwrap();
        assert!(
            inspector
                .inspect(
                    &path,
                    ArtifactKind::Object,
                    ProviderResolution::Explicit,
                    ProviderProvenance::User,
                    complete.source().target(),
                )
                .is_err(),
            "truncation at byte {length} must fail closed"
        );
    }
    let relocated_root = tempfile::Builder::new()
        .prefix("linc-relocated-")
        .tempdir()
        .unwrap();
    let relocated = relocated_root.path().join("explicit-object.o");
    fs::copy(&fixtures.object, &relocated).unwrap();
    let first = inspector
        .inspect(
            &fixtures.object,
            ArtifactKind::Object,
            ProviderResolution::Explicit,
            ProviderProvenance::User,
            complete.source().target(),
        )
        .unwrap();
    let second = inspector
        .inspect(
            &relocated,
            ArtifactKind::Object,
            ProviderResolution::Explicit,
            ProviderProvenance::User,
            complete.source().target(),
        )
        .unwrap();
    assert_eq!(
        first.artifact().artifact_fingerprint(),
        second.artifact().artifact_fingerprint()
    );
    assert_ne!(
        first.artifact().provider_id(),
        second.artifact().provider_id(),
        "canonical provider path is intentionally part of provider identity"
    );
    assert_eq!(first.symbols(), second.symbols());
}

#[test]
fn strict_symbol_validation_rejects_hidden_local_imported_missing_weak_and_duplicate() {
    let fixtures = Fixtures::build();
    let complete = complete_source();
    let declaration = DeclarationId::from_str(PARC_OPEN).unwrap();
    for path in [
        &fixtures.hidden,
        &fixtures.local,
        &fixtures.imported,
        &fixtures.weak,
        &fixtures.versioned,
    ] {
        let resolution = exact_resolution(&complete, path, fixtures.root.path());
        let error = StrictEvidenceValidator
            .validate_declaration(
                &complete,
                &resolution,
                &[],
                &[],
                StrictDeclarationRequest {
                    declaration,
                    decoration: SymbolDecoration::None,
                    layout: LayoutAssessment::NotRequired,
                    callable_abi: CallableAbiAssessment::Missing,
                    abi_shape: None,
                },
            )
            .unwrap_err();
        assert_eq!(error.code(), "LINC-E3040");
    }

    let duplicate_inputs = vec![
        NativeInput::DynamicLibraryPath(canonical(&fixtures.global_one)),
        NativeInput::DynamicLibraryPath(canonical(&fixtures.global_two)),
    ];
    let duplicate = resolve(
        &complete,
        &duplicate_inputs,
        ResolutionPolicy::ExactPathsOnly,
        LibraryPreference::DynamicOnly,
        fixtures.root.path(),
    )
    .unwrap();
    let error = StrictEvidenceValidator
        .validate_declaration(
            &complete,
            &duplicate,
            &[],
            &[],
            StrictDeclarationRequest {
                declaration,
                decoration: SymbolDecoration::None,
                layout: LayoutAssessment::NotRequired,
                callable_abi: CallableAbiAssessment::Missing,
                abi_shape: None,
            },
        )
        .unwrap_err();
    assert_eq!(error.code(), "LINC-E3040");

    let weak_ambiguity_inputs = vec![
        NativeInput::DynamicLibraryPath(canonical(&fixtures.global_one)),
        NativeInput::DynamicLibraryPath(canonical(&fixtures.weak)),
    ];
    let weak_ambiguity = resolve(
        &complete,
        &weak_ambiguity_inputs,
        ResolutionPolicy::ExactPathsOnly,
        LibraryPreference::DynamicOnly,
        fixtures.root.path(),
    )
    .unwrap();
    let error = StrictEvidenceValidator
        .validate_declaration(
            &complete,
            &weak_ambiguity,
            &[],
            &[],
            StrictDeclarationRequest {
                declaration,
                decoration: SymbolDecoration::None,
                layout: LayoutAssessment::NotRequired,
                callable_abi: CallableAbiAssessment::Missing,
                abi_shape: None,
            },
        )
        .unwrap_err();
    assert_eq!(error.code(), "LINC-E3040");

    let weak_inputs = [NativeInput::DynamicLibraryPath(canonical(&fixtures.weak))];
    let policy = analysis_policy(ResolutionPolicy::ExactPathsOnly, fixtures.root.path())
        .with_weak_symbol_policy(WeakSymbolPolicy::AllowUnique);
    let request = AnalysisRequest::try_new(&complete, &weak_inputs, policy).unwrap();
    let weak_allowed = NativeResolver::new(
        NativeInspector::default(),
        ResolverConfiguration::new(Vec::new(), LibraryPreference::DynamicOnly, 128).unwrap(),
    )
    .unwrap()
    .resolve(&request)
    .unwrap();
    let error = StrictEvidenceValidator
        .validate_declaration(
            &complete,
            &weak_allowed,
            &[],
            &[],
            StrictDeclarationRequest {
                declaration,
                decoration: SymbolDecoration::None,
                layout: LayoutAssessment::NotRequired,
                callable_abi: CallableAbiAssessment::Missing,
                abi_shape: None,
            },
        )
        .unwrap_err();
    assert_eq!(
        error.code(),
        "LINC-E3041",
        "AllowUnique must pass the symbol dimension before callable evidence is checked"
    );
}

#[test]
fn strict_validation_rejects_decoration_calling_convention_and_missing_probe_evidence() {
    let fixtures = Fixtures::build();
    let complete = complete_source();
    let declaration = DeclarationId::from_str(PARC_OPEN).unwrap();
    let resolution = exact_resolution(&complete, &fixtures.global_one, fixtures.root.path());
    let corpus = linc_corpus::decode_preservation_link_analysis().unwrap();
    let probe = corpus.abi_probes()[0].id();

    let error = StrictEvidenceValidator
        .validate_declaration(
            &complete,
            &resolution,
            &[],
            &[],
            StrictDeclarationRequest {
                declaration,
                decoration: SymbolDecoration::LeadingUnderscore,
                layout: LayoutAssessment::NotRequired,
                callable_abi: CallableAbiAssessment::Missing,
                abi_shape: None,
            },
        )
        .unwrap_err();
    assert_eq!(error.code(), "LINC-E3040");

    let error = StrictEvidenceValidator
        .validate_declaration(
            &complete,
            &resolution,
            &[],
            &[],
            StrictDeclarationRequest {
                declaration,
                decoration: SymbolDecoration::None,
                layout: LayoutAssessment::NotRequired,
                callable_abi: CallableAbiAssessment::Confirmed {
                    calling_convention: parc::contract::CallingConvention::C,
                    confidence: EvidenceConfidence::Measured,
                    probe,
                },
                abi_shape: None,
            },
        )
        .unwrap_err();
    assert_eq!(error.code(), "LINC-E3041");

    let function = match &complete.source().declaration(declaration).unwrap().kind {
        SourceDeclarationKind::Function(function) => function,
        _ => unreachable!(),
    };
    let parameter =
        AbiDimension::try_new(&function.parameters[0].ty, 64, 64, ValuePassing::Direct).unwrap();
    let return_value =
        AbiDimension::try_new(&function.return_type, 64, 64, ValuePassing::Direct).unwrap();
    let shape = AbiShapeEvidence::try_new(
        declaration,
        complete.source().fingerprint(),
        complete.source().target_fingerprint(),
        function.calling_convention.clone(),
        false,
        vec![parameter],
        return_value,
        ReturnConvention::Direct,
        probe,
    )
    .unwrap();
    let stale_source = SourceFingerprint::from_str(&format!("psource2_{}", "0".repeat(64)))
        .expect("syntactically valid stale source fingerprint");
    let stale_shape = AbiShapeEvidence::try_new(
        declaration,
        stale_source,
        complete.source().target_fingerprint(),
        function.calling_convention.clone(),
        false,
        shape.parameters().to_vec(),
        shape.return_value().clone(),
        shape.return_convention(),
        probe,
    )
    .unwrap();
    let error = StrictEvidenceValidator
        .validate_declaration(
            &complete,
            &resolution,
            &[],
            &[],
            StrictDeclarationRequest {
                declaration,
                decoration: SymbolDecoration::None,
                layout: LayoutAssessment::NotRequired,
                callable_abi: CallableAbiAssessment::Confirmed {
                    calling_convention: function.calling_convention.clone(),
                    confidence: EvidenceConfidence::Measured,
                    probe,
                },
                abi_shape: Some(&stale_shape),
            },
        )
        .unwrap_err();
    assert_eq!(error.code(), "LINC-E3041");

    let error = StrictEvidenceValidator
        .validate_declaration(
            &complete,
            &resolution,
            &[],
            &[],
            StrictDeclarationRequest {
                declaration,
                decoration: SymbolDecoration::None,
                layout: LayoutAssessment::NotRequired,
                callable_abi: CallableAbiAssessment::Confirmed {
                    calling_convention: function.calling_convention.clone(),
                    confidence: EvidenceConfidence::Measured,
                    probe,
                },
                abi_shape: Some(&shape),
            },
        )
        .unwrap_err();
    assert_eq!(error.code(), "LINC-E3041");
}

#[test]
fn authoritative_analyzer_returns_only_fully_validated_packages() {
    let fixtures = Fixtures::build();
    let complete = complete_source_for_compiler(&fixtures.compiler);
    let declaration = DeclarationId::from_str(PARC_OPEN).unwrap();
    let function = match &complete.source().declaration(declaration).unwrap().kind {
        SourceDeclarationKind::Function(function) => function,
        _ => unreachable!(),
    };
    let parameter =
        AbiDimension::try_new(&function.parameters[0].ty, 64, 64, ValuePassing::Direct).unwrap();
    let return_value =
        AbiDimension::try_new(&function.return_type, 64, 64, ValuePassing::Direct).unwrap();
    let placeholder_probe = linc_corpus::decode_preservation_link_analysis()
        .unwrap()
        .abi_probes()[0]
        .id();
    let prototype_shape = AbiShapeEvidence::try_new(
        declaration,
        complete.source().fingerprint(),
        complete.source().target_fingerprint(),
        function.calling_convention.clone(),
        false,
        vec![parameter.clone()],
        return_value.clone(),
        ReturnConvention::Direct,
        placeholder_probe,
    )
    .unwrap();
    let shape_fingerprint = prototype_shape.fingerprint().unwrap();
    let resource_limits = limits(3_000, 64 * 1024);
    let outcome = ProbeRunner
        .run(ProbeRequest {
            source_fingerprint: complete.source().fingerprint(),
            target: complete.source().target(),
            program: ProbeProgram::try_new(
                vec!["stddef.h".to_owned()],
                "typedef void *linc_handle;\n\
                 typedef linc_handle (__attribute__((ms_abi)) *linc_function)(linc_handle);\n\
                 _Static_assert(sizeof(linc_handle) == 8, \"pointer width\");\n\
                 _Static_assert(sizeof(linc_function) == 8, \"function pointer width\");\n\
                 int linc_probe_anchor(void) { return 0; }\n"
                    .to_owned(),
            )
            .unwrap(),
            expectations: vec![ProbeExpectation::new(
                ProbeSubject::CallableAbi { declaration },
                shape_fingerprint,
            )],
            method: ProbeMethod::CompileTimeAssertion,
            compiler_executable: fixtures.compiler.clone(),
            compiler_arguments: vec![
                ProbeCompilerArgument::Literal(OsString::from("-std=c11")),
                ProbeCompilerArgument::Literal(OsString::from("-m64")),
                ProbeCompilerArgument::Literal(OsString::from("-c")),
                ProbeCompilerArgument::ProbeSource,
                ProbeCompilerArgument::Literal(OsString::from("-o")),
                ProbeCompilerArgument::OutputArtifact,
            ],
            environment: Vec::new(),
            temporary_parent: canonical(fixtures.root.path()),
            resource_limits,
            runner: None,
        })
        .unwrap();
    let ProbeRunOutcome::Verified { evidence, .. } = outcome else {
        panic!("compile-only callable probe must verify");
    };
    let shape = AbiShapeEvidence::try_new(
        declaration,
        complete.source().fingerprint(),
        complete.source().target_fingerprint(),
        function.calling_convention.clone(),
        false,
        vec![parameter],
        return_value,
        ReturnConvention::Direct,
        evidence.id(),
    )
    .unwrap();
    assert_eq!(shape.fingerprint().unwrap(), shape_fingerprint);

    let policy = AnalysisPolicy::strict(
        ResolutionPolicy::ExactPathsOnly,
        ProbePolicy::CompileOnly,
        RunnerPolicy::Unavailable,
        evidence.execution_policy().clone(),
    )
    .unwrap();
    let native_inputs = [NativeInput::DynamicLibraryPath(canonical(
        &fixtures.global_one,
    ))];
    let request = AnalysisRequest::try_new(&complete, &native_inputs, policy).unwrap();
    let resolver = NativeResolver::new(
        NativeInspector::default(),
        ResolverConfiguration::new(Vec::new(), LibraryPreference::DynamicOnly, 128).unwrap(),
    )
    .unwrap();
    let analyzer = NativeAnalyzer::new(resolver);
    let declaration_request = NativeDeclarationRequest::new(
        declaration,
        SymbolDecoration::None,
        CallableAbiAssessment::Confirmed {
            calling_convention: function.calling_convention.clone(),
            confidence: EvidenceConfidence::Measured,
            probe: evidence.id(),
        },
        Some(shape),
    );
    let validated = analyzer
        .analyze(
            &request,
            NativeAnalysisInput {
                abi_probes: vec![evidence.clone()],
                layouts: Vec::new(),
                declarations: vec![declaration_request.clone()],
                diagnostics: Vec::new(),
            },
        )
        .expect("authoritative analysis must produce a validated package");
    assert_eq!(
        validated.package().source_fingerprint(),
        complete.source().fingerprint()
    );
    assert_eq!(validated.package().declaration_evidence().len(), 3);

    let error = analyzer
        .analyze(
            &request,
            NativeAnalysisInput {
                abi_probes: vec![evidence.clone()],
                layouts: Vec::new(),
                declarations: Vec::new(),
                diagnostics: Vec::new(),
            },
        )
        .unwrap_err();
    assert_eq!(error.code(), "LINC-E3041");

    let error = analyzer
        .analyze(
            &request,
            NativeAnalysisInput {
                abi_probes: vec![evidence],
                layouts: Vec::new(),
                declarations: vec![declaration_request.clone(), declaration_request],
                diagnostics: Vec::new(),
            },
        )
        .unwrap_err();
    assert_eq!(error.code(), "LINC-E3014");
}

#[test]
fn production_certifier_owns_probe_facts_and_returns_validated_analysis() {
    let fixtures = Fixtures::build();
    let compiler = canonical(&fixtures.compiler);
    let toolchain =
        CertificationToolchain::observe(compiler, Vec::new(), limits(5_000, 1024 * 1024)).unwrap();
    let complete = complete_certification_source_for_toolchain(&toolchain);
    let temporary_parent = canonical(fixtures.root.path());
    let policy = AnalysisPolicy::strict(
        ResolutionPolicy::ExactPathsOnly,
        ProbePolicy::CompileOnly,
        RunnerPolicy::Unavailable,
        ProbeExecutionPolicy::try_new(
            temporary_parent,
            ProbeEnvironmentIdentity::try_new(ProbeEnvironmentPolicy::Empty, Vec::new()).unwrap(),
            limits(5_000, 1024 * 1024),
        )
        .unwrap(),
    )
    .unwrap();
    let native_inputs = [NativeInput::DynamicLibraryPath(canonical(
        &fixtures.global_one,
    ))];
    let request = AnalysisRequest::try_new(&complete, &native_inputs, policy).unwrap();
    let resolver = NativeResolver::new(
        NativeInspector::default(),
        ResolverConfiguration::new(Vec::new(), LibraryPreference::DynamicOnly, 128).unwrap(),
    )
    .unwrap();
    let validated = NativeAnalyzer::new(resolver)
        .certify(&request, &toolchain)
        .expect("production certifier must construct and validate its own evidence");

    assert_eq!(validated.package().abi_probes().len(), 1);
    assert_eq!(
        validated.package().abi_probes()[0].method(),
        ProbeMethod::CompileTimeAssertion
    );
    assert!(validated.package().abi_probes()[0]
        .execution_result()
        .is_none());
    assert_eq!(validated.package().layouts().len(), 2);
    assert_eq!(validated.package().declaration_evidence().len(), 5);
}

#[test]
fn clang_observation_uses_resource_directory_not_gcc_sysroot_flags() {
    let Some(clang) = std::env::var_os("LINC_TEST_CLANG").filter(|value| !value.is_empty()) else {
        eprintln!("skipping optional Clang observation: LINC_TEST_CLANG is unset");
        return;
    };
    let toolchain = CertificationToolchain::observe(
        canonical(Path::new(&clang)),
        Vec::new(),
        limits(5_000, 1024 * 1024),
    )
    .expect("supported Clang must be observable without GCC-only sysroot flags");
    assert!(matches!(
        toolchain.compiler_identity().family(),
        CompilerFamily::Clang | CompilerFamily::AppleClang
    ));
    assert!(toolchain.compiler_sysroot().is_none());
    assert!(toolchain.compiler_resource_dir().is_some_and(Path::is_dir));
}

#[test]
fn production_certifier_measures_nontrivial_aggregate_closure() {
    let fixtures = Fixtures::build();
    let compiler = canonical(&fixtures.compiler);
    let toolchain =
        CertificationToolchain::observe(compiler.clone(), Vec::new(), limits(5_000, 1024 * 1024))
            .unwrap();
    let header = fixtures.root.path().join("certification-api.h");
    let provider_source = fixtures.root.path().join("certification-provider.c");
    let declaration_source = r#"
struct linc_inner { int x; double y; };
union linc_word { unsigned long long u; double d; };
enum linc_mode { LINC_MODE_LOW = -1, LINC_MODE_HIGH = 7 };
typedef int (*linc_callback)(int value);
struct linc_aggregate {
    int values[3];
    struct linc_inner inner;
    union linc_word word;
    linc_callback callback;
};
typedef struct linc_aggregate linc_payload;
typedef void linc_nothing;
struct linc_bits { unsigned flags : 3; unsigned ready : 1; };
extern linc_payload linc_state;
linc_payload linc_transform(linc_payload value, enum linc_mode mode);
linc_nothing linc_notify(int value);
"#;
    fs::write(&header, declaration_source).unwrap();
    fs::write(
        &provider_source,
        format!(
            "{declaration_source}\nlinc_payload linc_state;\nlinc_payload linc_transform(linc_payload value, enum linc_mode mode) {{ (void)mode; return value; }}\nlinc_nothing linc_notify(int value) {{ (void)value; }}\n"
        ),
    )
    .unwrap();
    let target = certification_target(&toolchain);
    let mapping =
        PathMapping::try_new([PathMappingRule::try_new(fixtures.root.path(), "fixture").unwrap()])
            .unwrap();
    let report = scan_headers(
        &ScanConfig::new(target, mapping, PreprocessorMode::Builtin)
            .unwrap()
            .entry_header(&header),
    )
    .unwrap();
    let complete = report.into_complete(&Selection::all_supported()).unwrap();
    let provider_object = fixtures.root.path().join("certification-provider.o");
    let provider = fixtures.root.path().join("libcertification-provider.so");
    compile_object(
        &compiler,
        &provider_source,
        &provider_object,
        &["-fPIC", "-std=gnu17"],
    );
    link_shared(&compiler, &[&provider_object], &provider, &[]);
    let policy = AnalysisPolicy::strict(
        ResolutionPolicy::ExactPathsOnly,
        ProbePolicy::CompileOnly,
        RunnerPolicy::Unavailable,
        ProbeExecutionPolicy::try_new(
            canonical(fixtures.root.path()),
            ProbeEnvironmentIdentity::try_new(ProbeEnvironmentPolicy::Empty, Vec::new()).unwrap(),
            limits(5_000, 1024 * 1024),
        )
        .unwrap(),
    )
    .unwrap();
    let native_inputs = [NativeInput::DynamicLibraryPath(canonical(&provider))];
    let request = AnalysisRequest::try_new(&complete, &native_inputs, policy).unwrap();
    let validated = NativeAnalyzer::default()
        .certify(&request, &toolchain)
        .expect("aggregate certification must be fully LINC-owned");

    assert_eq!(validated.package().layouts().len(), 5);
    assert!(validated.package().abi_probes()[0]
        .subjects()
        .iter()
        .any(|subject| matches!(subject, ProbeSubject::CallableAbi { .. })));
    let bitfield_layout = validated
        .package()
        .layouts()
        .iter()
        .find_map(|layout| match layout {
            linc::contract::LayoutEvidence::Record(record)
                if record
                    .fields()
                    .iter()
                    .any(|field| field.size_bits() == Some(3)) =>
            {
                Some(record)
            }
            _ => None,
        })
        .expect("bitfield record layout");
    assert_eq!(bitfield_layout.fields().len(), 2);
    let mut bit_ranges = bitfield_layout
        .fields()
        .iter()
        .map(|field| (field.offset_bits(), field.size_bits()))
        .collect::<Vec<_>>();
    bit_ranges.sort_unstable();
    assert_eq!(bit_ranges, [(0, Some(3)), (3, Some(1))]);
}

#[test]
fn bounded_probe_runner_captures_identity_environment_and_exact_subject_mapping() {
    let fixtures = Fixtures::build();
    let complete = complete_source();
    let declaration = DeclarationId::from_str(PARC_OPEN).unwrap();
    let fingerprint = ContentFingerprint::from_content(b"callable-shape");
    let body = marker_program(declaration, fingerprint, "return 0;");
    let before = probe_directories(fixtures.root.path());
    let outcome = ProbeRunner
        .run(probe_request(
            &complete,
            &fixtures,
            ProbeProgram::try_new(
                vec![
                    "stdio.h".to_owned(),
                    "stdlib.h".to_owned(),
                    "string.h".to_owned(),
                ],
                format!(
                    "int main(void) {{\n\
                     if (getenv(\"HOME\") != 0) return 7;\n\
                     const char *value = getenv(\"LINC_EXPLICIT\");\n\
                     if (value == 0 || strcmp(value, \"present\") != 0) return 8;\n\
                     {body}\n\
                     }}\n"
                ),
            )
            .unwrap(),
            declaration,
            fingerprint,
            ProbeMethod::ExecutedHarness,
            Some(env_runner()),
            vec![
                EnvironmentSetting::Unset {
                    name: "HOME".to_owned(),
                },
                EnvironmentSetting::Set {
                    name: "LINC_EXPLICIT".to_owned(),
                    value: OsString::from("present"),
                },
            ],
            3_000,
            64 * 1024,
        ))
        .unwrap();
    let ProbeRunOutcome::Verified {
        evidence,
        compiler_sysroot,
    } = outcome
    else {
        panic!("probe should verify");
    };
    assert!(compiler_sysroot.is_absolute());
    assert!(evidence.verified(ProbeSubject::CallableAbi { declaration }));
    assert_eq!(
        evidence.execution_policy().environment().policy(),
        ProbeEnvironmentPolicy::Explicit
    );
    assert_eq!(evidence.execution_policy().environment().entries().len(), 2);
    assert_eq!(probe_directories(fixtures.root.path()), before);
}

#[test]
fn bounded_probe_runner_rejects_bad_runner_timeout_output_nonzero_parser_gap_and_foreign_native_run(
) {
    let fixtures = Fixtures::build();
    let complete = complete_source();
    let declaration = DeclarationId::from_str(PARC_OPEN).unwrap();
    let fingerprint = ContentFingerprint::from_content(b"probe-rejection");

    let bad_runner = RunnerSpec::new(
        canonical(Path::new("/usr/bin/false")),
        vec![ProbeRunnerArgument::ProbeExecutable],
    )
    .unwrap();
    assert_rejection(
        ProbeRunner
            .run(probe_request(
                &complete,
                &fixtures,
                ProbeProgram::try_new(Vec::new(), "int main(void) { return 0; }".to_owned())
                    .unwrap(),
                declaration,
                fingerprint,
                ProbeMethod::ExecutedHarness,
                Some(bad_runner),
                Vec::new(),
                3_000,
                64 * 1024,
            ))
            .unwrap(),
        ProbeRejectionKind::Nonzero,
    );

    assert_rejection(
        ProbeRunner
            .run(probe_request(
                &complete,
                &fixtures,
                ProbeProgram::try_new(Vec::new(), "int main(void) { for (;;) {} }".to_owned())
                    .unwrap(),
                declaration,
                fingerprint,
                ProbeMethod::ExecutedHarness,
                Some(env_runner()),
                Vec::new(),
                300,
                64 * 1024,
            ))
            .unwrap(),
        ProbeRejectionKind::Timeout,
    );

    assert_rejection(
        ProbeRunner
            .run(probe_request(
                &complete,
                &fixtures,
                ProbeProgram::try_new(
                    vec!["stdio.h".to_owned()],
                    "int main(void) { for (;;) puts(\"01234567890123456789\"); }".to_owned(),
                )
                .unwrap(),
                declaration,
                fingerprint,
                ProbeMethod::ExecutedHarness,
                Some(env_runner()),
                Vec::new(),
                3_000,
                512,
            ))
            .unwrap(),
        ProbeRejectionKind::OutputLimit,
    );

    assert_rejection(
        ProbeRunner
            .run(probe_request(
                &complete,
                &fixtures,
                ProbeProgram::try_new(
                    vec!["stdio.h".to_owned(), "unistd.h".to_owned()],
                    format!(
                        "int main(void) {{\n\
                         int child = fork();\n\
                         if (child < 0) return 11;\n\
                         if (child == 0) {{ sleep(10); return 0; }}\n\
                         {}\n\
                         }}\n",
                        marker_program(declaration, fingerprint, "return 0;")
                    ),
                )
                .unwrap(),
                declaration,
                fingerprint,
                ProbeMethod::ExecutedHarness,
                Some(env_runner()),
                Vec::new(),
                3_000,
                64 * 1024,
            ))
            .unwrap(),
        ProbeRejectionKind::UnsafeStreams,
    );

    assert_rejection(
        ProbeRunner
            .run(probe_request(
                &complete,
                &fixtures,
                ProbeProgram::try_new(
                    Vec::new(),
                    "int main(void) { this is not valid C; }".to_owned(),
                )
                .unwrap(),
                declaration,
                fingerprint,
                ProbeMethod::CompileTimeAssertion,
                None,
                Vec::new(),
                3_000,
                64 * 1024,
            ))
            .unwrap(),
        ProbeRejectionKind::Nonzero,
    );

    assert_rejection(
        ProbeRunner
            .run(probe_request(
                &complete,
                &fixtures,
                ProbeProgram::try_new(Vec::new(), "int main(void) { return 0; }".to_owned())
                    .unwrap(),
                declaration,
                fingerprint,
                ProbeMethod::ExecutedHarness,
                Some(env_runner()),
                Vec::new(),
                3_000,
                64 * 1024,
            ))
            .unwrap(),
        ProbeRejectionKind::ParserGap,
    );

    let foreign = foreign_target(complete.source().target());
    let request = ProbeRequest {
        source_fingerprint: complete.source().fingerprint(),
        target: &foreign,
        program: ProbeProgram::try_new(Vec::new(), "int main(void) { return 0; }".to_owned())
            .unwrap(),
        expectations: vec![ProbeExpectation::new(
            ProbeSubject::CallableAbi { declaration },
            fingerprint,
        )],
        method: ProbeMethod::ExecutedHarness,
        compiler_executable: fixtures.compiler.clone(),
        compiler_arguments: compiler_arguments(),
        environment: Vec::new(),
        temporary_parent: canonical(fixtures.root.path()),
        resource_limits: limits(3_000, 64 * 1024),
        runner: None,
    };
    assert_rejection(
        ProbeRunner.run(request).unwrap(),
        ProbeRejectionKind::MissingRunner,
    );

    let outcome = ProbeRunner
        .run(ProbeRequest {
            source_fingerprint: complete.source().fingerprint(),
            target: &foreign,
            program: ProbeProgram::try_new(
                Vec::new(),
                "int foreign_probe_anchor(void) { return 0; }".to_owned(),
            )
            .unwrap(),
            expectations: vec![ProbeExpectation::new(
                ProbeSubject::CallableAbi { declaration },
                fingerprint,
            )],
            method: ProbeMethod::CompileTimeAssertion,
            compiler_executable: fixtures.cross_compiler.clone(),
            compiler_arguments: compiler_arguments(),
            environment: Vec::new(),
            temporary_parent: canonical(fixtures.root.path()),
            resource_limits: limits(3_000, 64 * 1024),
            runner: None,
        })
        .unwrap();
    assert!(
        matches!(outcome, ProbeRunOutcome::Verified { .. }),
        "compile-only foreign evidence must not execute its deliberately non-executable output"
    );
}

fn complete_source() -> CompleteSourcePackage {
    decode_source_package(parc_corpus::COMPLETE_SOURCE_PACKAGE_JSON)
        .unwrap()
        .into_complete(&linc_corpus::preservation_selection())
        .unwrap()
}

fn complete_source_for_compiler(compiler_path: &Path) -> CompleteSourcePackage {
    let base = decode_source_package(parc_corpus::COMPLETE_SOURCE_PACKAGE_JSON).unwrap();
    let target = base.target();
    let compiler = observed_compiler_identity(compiler_path);
    let target = TargetSpec::try_new(TargetSpecParts {
        triple: target.triple().to_owned(),
        architecture: target.architecture(),
        vendor: target.vendor().clone(),
        operating_system: target.operating_system(),
        environment: target.environment(),
        object_format: target.object_format(),
        endian: target.endian(),
        pointer_width: target.pointer_width(),
        c_data_model: target.c_data_model().clone(),
        language_standard: target.language_standard(),
        extension_profile: target.extension_profile().clone(),
        compiler,
        sysroot: target.sysroot().cloned(),
        abi_flags: target.abi_flags().to_vec(),
    })
    .unwrap();
    let package = SourcePackage::try_new(SourcePackageInput {
        target,
        files: base.files().to_vec(),
        inputs: base.inputs().clone(),
        declarations: base.declarations().to_vec(),
        macros: base.macros().to_vec(),
        diagnostics: base.diagnostics().to_vec(),
        completeness: base.completeness().clone(),
    })
    .unwrap();
    let selection = Selection::only([DeclarationId::from_str(PARC_OPEN).unwrap()]).unwrap();
    package.into_complete(&selection).unwrap()
}

fn complete_certification_source_for_toolchain(
    toolchain: &CertificationToolchain,
) -> CompleteSourcePackage {
    let base = decode_source_package(parc_corpus::COMPLETE_SOURCE_PACKAGE_JSON).unwrap();
    let target = certification_target(toolchain);
    let declaration = DeclarationId::from_str(PARC_OPEN).unwrap();
    let mut declarations = base.declarations().to_vec();
    let callable = declarations
        .iter_mut()
        .find(|candidate| candidate.id == declaration)
        .unwrap();
    let SourceDeclarationKind::Function(function) = &mut callable.kind else {
        unreachable!()
    };
    function.calling_convention = CallingConvention::C;
    let package = SourcePackage::try_new(SourcePackageInput {
        target,
        files: base.files().to_vec(),
        inputs: base.inputs().clone(),
        declarations,
        macros: base.macros().to_vec(),
        diagnostics: base.diagnostics().to_vec(),
        completeness: base.completeness().clone(),
    })
    .unwrap();
    package
        .into_complete(&linc_corpus::preservation_selection())
        .unwrap()
}

fn certification_target(toolchain: &CertificationToolchain) -> TargetSpec {
    let base = decode_source_package(parc_corpus::COMPLETE_SOURCE_PACKAGE_JSON).unwrap();
    let target = base.target();
    TargetSpec::try_new(TargetSpecParts {
        triple: target.triple().to_owned(),
        architecture: target.architecture(),
        vendor: target.vendor().clone(),
        operating_system: target.operating_system(),
        environment: target.environment(),
        object_format: target.object_format(),
        endian: target.endian(),
        pointer_width: target.pointer_width(),
        c_data_model: target.c_data_model().clone(),
        language_standard: target.language_standard(),
        extension_profile: ExtensionProfile::new(ExtensionFamily::Gnu, []),
        compiler: toolchain.compiler_identity().clone(),
        sysroot: None,
        abi_flags: target.abi_flags().to_vec(),
    })
    .unwrap()
}

fn observed_compiler_identity(compiler: &Path) -> CompilerIdentity {
    let version = tool_stdout(compiler, &["--version"]);
    let target = tool_stdout(compiler, &["-dumpmachine"]);
    let version_text = std::str::from_utf8(&version).unwrap();
    let first_line = version_text.lines().next().unwrap().trim();
    let lower = first_line.to_ascii_lowercase();
    let family = if lower.contains("apple clang") {
        CompilerFamily::AppleClang
    } else if lower.contains("clang") {
        CompilerFamily::Clang
    } else if lower.contains("gcc") || lower.contains("gnu") {
        CompilerFamily::Gcc
    } else {
        panic!("test compiler family is not supported: {first_line}");
    };
    let logical_name = compiler.file_name().and_then(OsStr::to_str).unwrap();
    CompilerIdentity::try_new(
        family,
        format!("toolchain/bin/{logical_name}"),
        ContentFingerprint::from_content(&fs::read(compiler).unwrap()),
        ContentFingerprint::from_content(&version),
        std::str::from_utf8(&target).unwrap().trim(),
        first_line,
    )
    .unwrap()
}

fn tool_stdout(program: &Path, arguments: &[&str]) -> Vec<u8> {
    let mut command = Command::new(program);
    command
        .args(arguments)
        .env_clear()
        .stdin(std::process::Stdio::null());
    let output = command.output().expect("execute explicit test tool");
    assert!(
        output.status.success(),
        "test tool query failed: {command:?}"
    );
    output.stdout
}

fn resolve(
    complete: &CompleteSourcePackage,
    inputs: &[NativeInput],
    policy: ResolutionPolicy,
    preference: LibraryPreference,
    temporary_parent: &Path,
) -> Result<linc::native::NativeResolution, NativeError> {
    let request =
        AnalysisRequest::try_new(complete, inputs, analysis_policy(policy, temporary_parent))
            .unwrap();
    NativeResolver::new(
        NativeInspector::default(),
        ResolverConfiguration::new(Vec::new(), preference, 128).unwrap(),
    )
    .unwrap()
    .resolve(&request)
}

fn analysis_policy(policy: ResolutionPolicy, temporary_parent: &Path) -> AnalysisPolicy {
    AnalysisPolicy::strict(
        policy,
        ProbePolicy::Disabled,
        RunnerPolicy::Unavailable,
        ProbeExecutionPolicy::try_new(
            canonical(temporary_parent),
            ProbeEnvironmentIdentity::try_new(ProbeEnvironmentPolicy::Empty, Vec::new()).unwrap(),
            limits(5_000, 1024 * 1024),
        )
        .unwrap(),
    )
    .unwrap()
}

fn exact_resolution(
    complete: &CompleteSourcePackage,
    path: &Path,
    temporary_parent: &Path,
) -> linc::native::NativeResolution {
    resolve(
        complete,
        &[NativeInput::DynamicLibraryPath(canonical(path))],
        ResolutionPolicy::ExactPathsOnly,
        LibraryPreference::DynamicOnly,
        temporary_parent,
    )
    .unwrap()
}

fn inventory_for<'a>(
    resolution: &'a linc::native::NativeResolution,
    filename: &str,
) -> &'a linc::contract::SymbolInventory {
    resolution
        .inventories()
        .iter()
        .find(|inventory| {
            inventory.artifact().canonical_path().file_name() == Some(OsStr::new(filename))
        })
        .unwrap()
}

fn symbol<'a>(
    inventory: &'a linc::contract::SymbolInventory,
    name: &str,
) -> &'a linc::contract::SymbolRecord {
    inventory
        .symbols()
        .iter()
        .find(|symbol| symbol.name() == name)
        .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn probe_request<'a>(
    complete: &'a CompleteSourcePackage,
    fixtures: &Fixtures,
    program: ProbeProgram,
    declaration: DeclarationId,
    fingerprint: ContentFingerprint,
    method: ProbeMethod,
    runner: Option<RunnerSpec>,
    environment: Vec<EnvironmentSetting>,
    wall_millis: u64,
    output_bytes: u64,
) -> ProbeRequest<'a> {
    ProbeRequest {
        source_fingerprint: complete.source().fingerprint(),
        target: complete.source().target(),
        program,
        expectations: vec![ProbeExpectation::new(
            ProbeSubject::CallableAbi { declaration },
            fingerprint,
        )],
        method,
        compiler_executable: fixtures.compiler.clone(),
        compiler_arguments: compiler_arguments(),
        environment,
        temporary_parent: canonical(fixtures.root.path()),
        resource_limits: limits(wall_millis, output_bytes),
        runner,
    }
}

fn compiler_arguments() -> Vec<ProbeCompilerArgument> {
    vec![
        ProbeCompilerArgument::Literal(OsString::from("-std=c11")),
        ProbeCompilerArgument::Literal(OsString::from("-O0")),
        ProbeCompilerArgument::ProbeSource,
        ProbeCompilerArgument::Literal(OsString::from("-o")),
        ProbeCompilerArgument::OutputArtifact,
    ]
}

fn limits(wall_millis: u64, output_bytes: u64) -> ProbeResourceLimits {
    ProbeResourceLimits::try_new(wall_millis, 1024 * 1024 * 1024, output_bytes, 16).unwrap()
}

fn env_runner() -> RunnerSpec {
    RunnerSpec::new(
        canonical(Path::new("/usr/bin/env")),
        vec![ProbeRunnerArgument::ProbeExecutable],
    )
    .unwrap()
}

fn marker_program(
    declaration: DeclarationId,
    fingerprint: ContentFingerprint,
    tail: &str,
) -> String {
    format!("printf(\"LINC_PROBE_V1|callable_abi|{declaration}|{fingerprint}\\n\"); {tail}")
}

fn assert_rejection(outcome: ProbeRunOutcome, expected: ProbeRejectionKind) {
    let ProbeRunOutcome::Rejected(rejection) = outcome else {
        panic!("expected structured rejection");
    };
    assert_eq!(rejection.kind(), expected, "{}", rejection.message());
}

fn probe_directories(parent: &Path) -> Vec<PathBuf> {
    let mut paths = fs::read_dir(parent)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|name| name.starts_with("linc-probe-"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn foreign_target(base: &TargetSpec) -> TargetSpec {
    let compiler = base.compiler();
    let compiler = CompilerIdentity::try_new(
        compiler.family(),
        compiler.logical_executable(),
        compiler.executable_content(),
        compiler.version_text(),
        "aarch64-unknown-linux-gnu",
        compiler.version(),
    )
    .unwrap();
    TargetSpec::try_new(TargetSpecParts {
        triple: "aarch64-unknown-linux-gnu".to_owned(),
        architecture: Architecture::Aarch64,
        vendor: base.vendor().clone(),
        operating_system: base.operating_system(),
        environment: base.environment(),
        object_format: base.object_format(),
        endian: base.endian(),
        pointer_width: 64,
        c_data_model: base.c_data_model().clone(),
        language_standard: base.language_standard(),
        extension_profile: base.extension_profile().clone(),
        compiler,
        sysroot: base.sysroot().cloned(),
        abi_flags: base.abi_flags().to_vec(),
    })
    .unwrap()
}

fn required_tool(variable: &str) -> PathBuf {
    let value = std::env::var_os(variable)
        .unwrap_or_else(|| panic!("{variable} must name an explicit required tool"));
    canonical(Path::new(&value))
}

fn compile_object(compiler: &Path, source: &Path, output: &Path, flags: &[&str]) {
    let mut command = Command::new(compiler);
    command
        .args(flags)
        .arg("-c")
        .arg(source)
        .arg("-o")
        .arg(output);
    run_command(command);
}

fn compile_executable(compiler: &Path, source: &Path, output: &Path) {
    let mut command = Command::new(compiler);
    command.arg(source).arg("-o").arg(output);
    run_command(command);
}

fn link_shared(compiler: &Path, objects: &[&Path], output: &Path, flags: &[&str]) {
    let mut command = Command::new(compiler);
    // Keep the evidence graph hermetic: the driver must not inject libc or a
    // target runtime behind the fixture's explicit provider list.
    command
        .args(["-shared", "-nostdlib"])
        .args(objects)
        .args(flags)
        .arg("-o")
        .arg(output);
    run_command(command);
}

fn make_archive(archive_tool: &Path, output: &Path, members: &[&Path]) {
    let mut command = Command::new(archive_tool);
    command.arg("rcs").arg(output).args(members);
    run_command(command);
}

fn build_shared_source(compiler: &Path, root: &Path, source: &Path, output_name: &str) -> PathBuf {
    let object = root.join(format!("{output_name}.o"));
    compile_object(compiler, source, &object, &["-fPIC"]);
    let output = root.join(output_name);
    link_shared(compiler, &[&object], &output, &[]);
    output
}

fn run_command(mut command: Command) {
    let display = format!("{command:?}");
    let output = command.output().expect("execute explicit fixture tool");
    assert!(
        output.status.success(),
        "{display} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap()
}
