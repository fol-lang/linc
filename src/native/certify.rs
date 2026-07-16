//! LINC-owned production certification for the initial GNU SysV64 profile.
//!
//! The public boundary accepts only a checked analysis request and an explicit
//! compiler toolchain.  Source rendering, compiler invocation, object decoding,
//! and all durable evidence construction stay inside LINC.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path, PathBuf},
    str::FromStr,
};

use object::{Object, ObjectSection, ObjectSymbol};
use parc::contract::{
    Architecture, ArrayBound, BitWidth, CDataModelClass, CFloatingType, CFunctionType,
    CIntegerType, CType, CTypeKind, CallingConvention, CharTypeSignedness, ChildId,
    ClosureRequirement, CompilerFamily, CompilerIdentity, CompleteSourcePackage, DeclarationId,
    Endian, EnumValue, Environment, ExactInteger, ExtensionFamily, FunctionPrototype,
    LanguageStandard, Linkage, ObjectFormat, OperatingSystem, RecordCompleteness, RecordKind,
    Signedness, SourceDeclarationKind, SourceEnum, SourceField, SourceFunction, SourceRecord,
    TargetSpec, TypeQualifiers,
};

use crate::contract::{
    AbiProbeEvidence, AbiProbeEvidenceInput, AnalysisRequest, CallableAbiAssessment,
    EnumLayoutEvidence, EnumVariantEvidence, EvidenceAcceptancePolicy, EvidenceConfidence,
    EvidenceSource, FieldLayoutEvidence, LayoutEvidence, ProbeEvidenceId, ProbeMethod, ProbePolicy,
    ProbeResourceLimits, ProbeRunnerEvidence, ProbeSubject, ProbeSubjectOutcome,
    ProbeSubjectStatus, RecordLayoutEvidence, ResolutionPolicy, RunnerPolicy, SymbolDecoration,
    WeakSymbolPolicy,
};

use super::{
    probe::{compile_owned_probe, observe_certification_compiler, OwnedProbeCompilation},
    sysv::{classify_sysv64_callable, sysv64_parameter_layout, sysv64_return_layout},
    AbiDimension, AbiShapeEvidence, EnvironmentSetting, NativeAnalysisInput,
    NativeDeclarationRequest, NativeError, NativeResult,
};

const BLOB_SYMBOL: &str = "linc_certification_blob_v1";
const BLOB_MAGIC: &[u8; 8] = b"LINCERT1";
const BLOB_VERSION: u64 = 1;
const RECORD_KIND: u64 = 1;
const ENUM_KIND: u64 = 2;
const BITFIELD_OFFSET_SENTINEL: u64 = u64::MAX;
const MAX_CERTIFICATION_SUBJECTS: usize = 100_000;
const MAX_CERTIFICATION_FIELDS: usize = 1_000_000;

/// Explicit compiler identity used by [`super::NativeAnalyzer::certify`].
///
/// Fields are private so callers cannot smuggle probe results or ABI facts
/// across the production certification boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificationToolchain {
    compiler_executable: PathBuf,
    environment: Vec<EnvironmentSetting>,
    compiler_identity: CompilerIdentity,
    compiler_sysroot: Option<PathBuf>,
    compiler_resource_dir: Option<PathBuf>,
    observation_limits: ProbeResourceLimits,
}

impl CertificationToolchain {
    /// Observe a compiler with the same bounded, direct process machinery
    /// which certification uses again after target construction.
    pub fn observe(
        compiler_executable: PathBuf,
        environment: Vec<EnvironmentSetting>,
        limits: ProbeResourceLimits,
    ) -> NativeResult<Self> {
        if !normalized_absolute_path(&compiler_executable) {
            return Err(NativeError::InvalidPolicy {
                detail: "certification compiler executable must be a normalized absolute path"
                    .to_owned(),
            });
        }
        let observed = observe_certification_compiler(&compiler_executable, &environment, limits)?;
        Ok(Self {
            compiler_executable: observed.compiler_executable,
            environment,
            compiler_identity: observed.compiler,
            compiler_sysroot: observed.sysroot,
            compiler_resource_dir: observed.resource_dir,
            observation_limits: limits,
        })
    }

    pub fn compiler_executable(&self) -> &Path {
        &self.compiler_executable
    }

    pub fn environment(&self) -> &[EnvironmentSetting] {
        &self.environment
    }

    pub const fn compiler_identity(&self) -> &CompilerIdentity {
        &self.compiler_identity
    }

    pub fn reported_target(&self) -> &str {
        self.compiler_identity.reported_target()
    }

    /// Canonical compiler-reported sysroot, or `None` when the compiler
    /// reports its default empty sysroot identity.
    pub fn compiler_sysroot(&self) -> Option<&Path> {
        self.compiler_sysroot.as_deref()
    }

    /// Canonical Clang resource directory observed independently from the
    /// empty/default sysroot identity. GCC toolchains return `None`.
    pub fn compiler_resource_dir(&self) -> Option<&Path> {
        self.compiler_resource_dir.as_deref()
    }

    pub const fn observation_limits(&self) -> ProbeResourceLimits {
        self.observation_limits
    }
}

fn normalized_absolute_path(path: &Path) -> bool {
    path.is_absolute()
        && path
            .components()
            .all(|component| !matches!(component, Component::CurDir | Component::ParentDir))
}

pub(super) fn validate_certification_request(request: &AnalysisRequest<'_>) -> NativeResult<()> {
    let policy = request.policy();
    if policy.resolution() != ResolutionPolicy::ExactPathsOnly
        || policy.probe() != ProbePolicy::CompileOnly
        || !matches!(policy.runner(), RunnerPolicy::Unavailable)
        || policy.layout_evidence() != EvidenceAcceptancePolicy::MeasuredOnly
        || policy.callable_abi_evidence() != EvidenceAcceptancePolicy::MeasuredOnly
        || policy.weak_symbols() != WeakSymbolPolicy::Reject
    {
        return Err(NativeError::InvalidPolicy {
            detail: "certification requires exact paths, compile-only probes, no runner, measured-only evidence, and weak-symbol rejection".to_owned(),
        });
    }
    validate_certified_target(request.source().source().target())
}

fn validate_certified_target(target: &TargetSpec) -> NativeResult<()> {
    let profile = target.triple() == "x86_64-unknown-linux-gnu"
        && target.architecture() == Architecture::X86_64
        && target.operating_system() == OperatingSystem::Linux
        && target.environment() == Environment::Gnu
        && target.object_format() == ObjectFormat::Elf
        && target.endian() == Endian::Little
        && target.pointer_width() == 64
        && target.language_standard() == LanguageStandard::C17
        && target.extension_profile().family == ExtensionFamily::Gnu
        && target.extension_profile().enabled.is_empty()
        && matches!(
            target.compiler().family(),
            CompilerFamily::Gcc | CompilerFamily::Clang
        )
        && target.sysroot().is_none()
        && target.abi_flags().len() == 1
        && target.abi_flags()[0].as_str() == "-m64"
        && target.c_data_model().class == CDataModelClass::LP64
        && target.c_data_model().char_bit == 8
        && target.c_data_model().pointer_layout.storage_bits == 64
        && target.c_data_model().pointer_layout.alignment_bits == 64;
    if !profile {
        return Err(NativeError::UnsupportedProbeType {
            detail: "the initial certification profile is exactly C17 GNU x86_64-unknown-linux-gnu ELF LP64 with -m64 and no sysroot".to_owned(),
        });
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct CertificationPlan<'a> {
    source: &'a CompleteSourcePackage,
    declaration_ordinals: BTreeMap<DeclarationId, usize>,
    requirements: BTreeMap<DeclarationId, ClosureRequirement>,
    records: Vec<RecordPlan<'a>>,
    enums: Vec<EnumPlan<'a>>,
    functions: Vec<FunctionPlan<'a>>,
    variables: Vec<DeclarationId>,
}

#[derive(Debug, Clone)]
struct RecordPlan<'a> {
    declaration: DeclarationId,
    ordinal: usize,
    record: &'a SourceRecord,
    fields: Vec<FieldPlan<'a>>,
}

#[derive(Debug, Clone)]
struct FieldPlan<'a> {
    field: &'a SourceField,
    name: String,
    bitfield_symbol: Option<String>,
    bitfield_type: Option<String>,
}

#[derive(Debug, Clone)]
struct EnumPlan<'a> {
    declaration: DeclarationId,
    ordinal: usize,
    enumeration: &'a SourceEnum,
}

#[derive(Debug, Clone)]
struct FunctionPlan<'a> {
    declaration: DeclarationId,
    ordinal: usize,
    function: &'a SourceFunction,
}

impl<'a> CertificationPlan<'a> {
    fn build(source: &'a CompleteSourcePackage) -> NativeResult<Self> {
        if source.declaration_closure().is_empty()
            || source.declaration_closure().len() > MAX_CERTIFICATION_SUBJECTS
        {
            return Err(NativeError::ProbeRender {
                detail: "certification closure is empty or exceeds its subject bound".to_owned(),
            });
        }
        let mut declaration_ordinals = BTreeMap::new();
        let mut requirements = BTreeMap::new();
        for (ordinal, entry) in source.declaration_closure().iter().enumerate() {
            declaration_ordinals.insert(entry.declaration(), ordinal);
            requirements.insert(entry.declaration(), entry.requirement());
        }

        let mut records = Vec::new();
        let mut enums = Vec::new();
        let mut functions = Vec::new();
        let mut variables = Vec::new();
        let mut total_fields = 0_usize;
        for entry in source.declaration_closure() {
            let id = entry.declaration();
            let ordinal = declaration_ordinals[&id];
            let declaration =
                source
                    .source()
                    .declaration(id)
                    .ok_or_else(|| NativeError::ProbeRender {
                        detail: format!("closure declaration {id} is missing"),
                    })?;
            match &declaration.kind {
                SourceDeclarationKind::Record(record)
                    if entry.requirement() == ClosureRequirement::Definition =>
                {
                    if record.completeness != RecordCompleteness::Complete {
                        return Err(NativeError::UnsupportedProbeType {
                            detail: format!("record {id} requires an unavailable definition"),
                        });
                    }
                    total_fields =
                        total_fields
                            .checked_add(record.fields.len())
                            .ok_or_else(|| NativeError::ProbeRender {
                                detail: "field count overflow".to_owned(),
                            })?;
                    let fields = record
                        .fields
                        .iter()
                        .enumerate()
                        .map(|(field_ordinal, field)| {
                            let bitfield = field.bit_width.is_some();
                            FieldPlan {
                                field,
                                name: format!("linc_f_{ordinal}_{field_ordinal}"),
                                bitfield_symbol: bitfield
                                    .then(|| format!("linc_bitfield_{ordinal}_{field_ordinal}")),
                                bitfield_type: bitfield.then(|| {
                                    format!("linc_bitfield_type_{ordinal}_{field_ordinal}")
                                }),
                            }
                        })
                        .collect();
                    records.push(RecordPlan {
                        declaration: id,
                        ordinal,
                        record,
                        fields,
                    });
                }
                SourceDeclarationKind::Record(_) => {}
                SourceDeclarationKind::Enum(enumeration)
                    if entry.requirement() == ClosureRequirement::Definition =>
                {
                    enums.push(EnumPlan {
                        declaration: id,
                        ordinal,
                        enumeration,
                    });
                }
                SourceDeclarationKind::Enum(_) => {
                    return Err(NativeError::UnsupportedProbeType {
                        detail: format!("enum {id} cannot be used without its definition"),
                    });
                }
                SourceDeclarationKind::Function(function)
                    if declaration.linkage == Linkage::External =>
                {
                    functions.push(FunctionPlan {
                        declaration: id,
                        ordinal,
                        function,
                    });
                }
                SourceDeclarationKind::Variable(_) if declaration.linkage == Linkage::External => {
                    variables.push(id);
                }
                SourceDeclarationKind::Function(_) | SourceDeclarationKind::Variable(_) => {}
                SourceDeclarationKind::TypeAlias(_) => {}
                SourceDeclarationKind::Unsupported(_) => {
                    return Err(NativeError::UnsupportedProbeType {
                        detail: format!("unsupported declaration {id} reached certification"),
                    });
                }
            }
        }
        if total_fields > MAX_CERTIFICATION_FIELDS {
            return Err(NativeError::ProbeRender {
                detail: "certification closure exceeds its field bound".to_owned(),
            });
        }
        let plan = Self {
            source,
            declaration_ordinals,
            requirements,
            records,
            enums,
            functions,
            variables,
        };
        plan.validate_references()?;
        Ok(plan)
    }

    fn ordinal(&self, declaration: DeclarationId) -> NativeResult<usize> {
        self.declaration_ordinals
            .get(&declaration)
            .copied()
            .ok_or_else(|| NativeError::ProbeRender {
                detail: format!("type reference {declaration} is outside the complete closure"),
            })
    }

    fn record_tag(&self, declaration: DeclarationId) -> NativeResult<String> {
        Ok(format!("linc_record_{}", self.ordinal(declaration)?))
    }

    fn enum_tag(&self, declaration: DeclarationId) -> NativeResult<String> {
        Ok(format!("linc_enum_{}", self.ordinal(declaration)?))
    }

    fn validate_references(&self) -> NativeResult<()> {
        for record in &self.records {
            for field in &record.record.fields {
                self.validate_type(&field.ty, &mut BTreeSet::new())?;
                if let Some(width) = &field.bit_width {
                    match width {
                        BitWidth::Known { bits } if *bits > 0 => {}
                        _ => {
                            return Err(NativeError::UnsupportedProbeType {
                                detail: format!(
                                    "record {} contains a zero, expression, or invalid bitfield width",
                                    record.declaration
                                ),
                            });
                        }
                    }
                    if field.name.is_none() {
                        return Err(NativeError::UnsupportedProbeType {
                            detail: format!(
                                "record {} contains an unnamed bitfield",
                                record.declaration
                            ),
                        });
                    }
                }
            }
        }
        for function in &self.functions {
            validate_function_profile(function.function)?;
            self.validate_type(&function.function.return_type, &mut BTreeSet::new())?;
            for parameter in &function.function.parameters {
                self.validate_type(&parameter.ty, &mut BTreeSet::new())?;
            }
        }
        for id in &self.variables {
            let declaration = self.source.source().declaration(*id).expect("plan ID");
            let SourceDeclarationKind::Variable(variable) = &declaration.kind else {
                unreachable!("variable plan is typed")
            };
            self.validate_type(&variable.ty, &mut BTreeSet::new())?;
        }
        Ok(())
    }

    fn validate_type(&self, ty: &CType, aliases: &mut BTreeSet<DeclarationId>) -> NativeResult<()> {
        if !ty.support.is_supported() {
            return Err(unsupported_type("PARC marked a source type unsupported"));
        }
        if ty.qualifiers.is_atomic {
            return Err(unsupported_type(
                "atomic-qualified types are outside the initial certification profile",
            ));
        }
        match &ty.kind {
            CTypeKind::Integer(CIntegerType::BitInt { .. }) => Err(NativeError::InvalidPolicy {
                detail: "_BitInt requires an explicit future measured scalar profile".to_owned(),
            }),
            CTypeKind::Floating(CFloatingType::Float128 | CFloatingType::Ts18661 { .. })
            | CTypeKind::Complex(CFloatingType::Float128 | CFloatingType::Ts18661 { .. })
            | CTypeKind::Unsupported { .. } => Err(unsupported_type(
                "extended or unsupported scalar type reached certification",
            )),
            CTypeKind::Pointer(pointee) => self.validate_type(pointee, aliases),
            CTypeKind::Array { element, bound, .. } => {
                if !matches!(bound, ArrayBound::Fixed { elements } if *elements > 0) {
                    return Err(unsupported_type(
                        "only nonzero fixed-size arrays are certified",
                    ));
                }
                self.validate_type(element, aliases)
            }
            CTypeKind::Function(function) => {
                validate_function_type_profile(function)?;
                self.validate_type(&function.return_type, aliases)?;
                for parameter in &function.parameters {
                    self.validate_type(&parameter.ty, aliases)?;
                }
                Ok(())
            }
            CTypeKind::AliasRef(id) => {
                if !aliases.insert(*id) {
                    return Err(NativeError::ProbeRender {
                        detail: format!("alias cycle includes {id}"),
                    });
                }
                let declaration = self.source.source().declaration(*id).ok_or_else(|| {
                    NativeError::ProbeRender {
                        detail: format!("alias {id} is missing"),
                    }
                })?;
                let SourceDeclarationKind::TypeAlias(alias) = &declaration.kind else {
                    return Err(NativeError::ProbeRender {
                        detail: format!("declaration {id} is not an alias"),
                    });
                };
                self.validate_type(&alias.target, aliases)?;
                aliases.remove(id);
                Ok(())
            }
            CTypeKind::RecordRef(id) => {
                let declaration = self.source.source().declaration(*id).ok_or_else(|| {
                    NativeError::ProbeRender {
                        detail: format!("record {id} is missing"),
                    }
                })?;
                if !matches!(declaration.kind, SourceDeclarationKind::Record(_)) {
                    return Err(NativeError::ProbeRender {
                        detail: format!("declaration {id} is not a record"),
                    });
                }
                self.ordinal(*id).map(|_| ())
            }
            CTypeKind::EnumRef(id) => {
                let declaration = self.source.source().declaration(*id).ok_or_else(|| {
                    NativeError::ProbeRender {
                        detail: format!("enum {id} is missing"),
                    }
                })?;
                if !matches!(declaration.kind, SourceDeclarationKind::Enum(_)) {
                    return Err(NativeError::ProbeRender {
                        detail: format!("declaration {id} is not an enum"),
                    });
                }
                if self.requirements.get(id) != Some(&ClosureRequirement::Definition) {
                    return Err(unsupported_type(
                        "enum reference has no complete definition",
                    ));
                }
                Ok(())
            }
            CTypeKind::Void
            | CTypeKind::Bool
            | CTypeKind::Integer(_)
            | CTypeKind::Floating(_)
            | CTypeKind::Complex(_) => Ok(()),
        }
    }
}

fn unsupported_type(detail: impl Into<String>) -> NativeError {
    NativeError::UnsupportedProbeType {
        detail: detail.into(),
    }
}

fn validate_function_profile(function: &SourceFunction) -> NativeResult<()> {
    validate_prototype(function.prototype, &function.calling_convention)
}

fn validate_function_type_profile(function: &CFunctionType) -> NativeResult<()> {
    validate_prototype(function.prototype, &function.calling_convention)
}

fn validate_prototype(
    prototype: FunctionPrototype,
    convention: &CallingConvention,
) -> NativeResult<()> {
    if !matches!(prototype, FunctionPrototype::Prototyped { variadic: false }) {
        return Err(unsupported_type(
            "variadic and unprototyped functions are outside the certified profile",
        ));
    }
    if !matches!(
        convention,
        CallingConvention::C | CallingConvention::Cdecl | CallingConvention::SysV64
    ) {
        return Err(unsupported_type(
            "the initial profile accepts only C, cdecl, and SysV64 call conventions",
        ));
    }
    Ok(())
}

impl CertificationPlan<'_> {
    fn render_probe(&self) -> NativeResult<String> {
        let mut source = String::with_capacity(64 * 1024);
        source.push_str("/* LINC-owned header-free certification unit. */\n");
        self.render_scalar_assertions(&mut source);

        // Every tag is synthetic and closure-derived.  No original identifier,
        // occurrence spelling, include, or ambient declaration is reused.
        for entry in self.source.declaration_closure() {
            let declaration = self
                .source
                .source()
                .declaration(entry.declaration())
                .expect("checked closure");
            if let SourceDeclarationKind::Record(record) = &declaration.kind {
                let keyword = record_keyword(record.kind);
                source.push_str(&format!(
                    "{keyword} {};\n",
                    self.record_tag(entry.declaration())?
                ));
            }
        }

        for enumeration in &self.enums {
            self.render_enum_definition(&mut source, enumeration)?;
        }
        for declaration in self.record_definition_order()? {
            let record = self
                .records
                .iter()
                .find(|record| record.declaration == declaration)
                .expect("definition order came from record plans");
            self.render_record_definition(&mut source, record)?;
        }
        for function in &self.functions {
            self.render_function_witness(&mut source, function)?;
        }
        for declaration in &self.variables {
            self.render_variable_witness(&mut source, *declaration)?;
        }
        self.render_blob(&mut source)?;
        for record in &self.records {
            self.render_bitfield_objects(&mut source, record)?;
        }
        Ok(source)
    }

    fn render_scalar_assertions(&self, source: &mut String) {
        let model = self.source.source().target().c_data_model();
        scalar_assert(source, "_Bool", model.bool_layout);
        scalar_assert(source, "char", model.char_layout);
        scalar_assert(source, "short", model.short_layout);
        scalar_assert(source, "int", model.int_layout);
        scalar_assert(source, "long", model.long_layout);
        scalar_assert(source, "long long", model.long_long_layout);
        scalar_assert(source, "void *", model.pointer_layout);
        scalar_assert(source, "float", model.float_layout.scalar);
        scalar_assert(source, "double", model.double_layout.scalar);
        scalar_assert(source, "long double", model.long_double_layout.scalar);
        if let Some(layout) = model.int128_layout {
            scalar_assert(source, "__int128", layout);
        }
        let signed_char = matches!(
            model.char_signedness,
            parc::contract::CharSignedness::Signed
        );
        source.push_str(&format!(
            "_Static_assert(((char)-1 < 0) == {}, \"plain char signedness\");\n",
            u8::from(signed_char)
        ));
    }

    fn render_enum_definition(&self, source: &mut String, plan: &EnumPlan<'_>) -> NativeResult<()> {
        if plan.enumeration.explicit_underlying_type.is_some() {
            return Err(unsupported_type(
                "explicit-underlying-type enums are outside the C17 certification profile",
            ));
        }
        if plan.enumeration.variants.is_empty() {
            return Err(unsupported_type(
                "empty enums are outside the initial certification profile",
            ));
        }
        source.push_str(&format!("enum linc_enum_{} {{\n", plan.ordinal));
        for (variant_ordinal, variant) in plan.enumeration.variants.iter().enumerate() {
            let EnumValue::Evaluated { value } = variant.value else {
                return Err(unsupported_type(
                    "unevaluated enum values cannot be certified",
                ));
            };
            source.push_str(&format!(
                "  linc_ev_{}_{} = {},\n",
                plan.ordinal,
                variant_ordinal,
                exact_integer_literal(value)?
            ));
        }
        source.push_str("};\n");
        for (variant_ordinal, variant) in plan.enumeration.variants.iter().enumerate() {
            let EnumValue::Evaluated { value } = variant.value else {
                unreachable!("checked above")
            };
            source.push_str(&format!(
                "_Static_assert(linc_ev_{}_{} == {}, \"enum value\");\n",
                plan.ordinal,
                variant_ordinal,
                exact_integer_literal(value)?
            ));
        }
        Ok(())
    }

    fn render_record_definition(
        &self,
        source: &mut String,
        plan: &RecordPlan<'_>,
    ) -> NativeResult<()> {
        for field in &plan.fields {
            if let Some(alias) = &field.bitfield_type {
                source.push_str("typedef ");
                source.push_str(&self.render_declaration(&field.field.ty, alias)?);
                source.push_str(";\n");
            }
        }
        let keyword = record_keyword(plan.record.kind);
        source.push_str(&format!("{keyword} linc_record_{} {{\n", plan.ordinal));
        for field in &plan.fields {
            let name = match &field.field.bit_width {
                Some(BitWidth::Known { bits }) => format!("{} : {bits}", field.name),
                Some(_) => unreachable!("bitfield profile checked while planning"),
                None => field.name.clone(),
            };
            source.push_str("  ");
            source.push_str(&self.render_declaration(&field.field.ty, &name)?);
            source.push_str(";\n");
        }
        source.push_str("};\n");
        Ok(())
    }

    fn render_function_witness(
        &self,
        source: &mut String,
        plan: &FunctionPlan<'_>,
    ) -> NativeResult<()> {
        let target = format!("linc_callable_target_{}", plan.ordinal);
        let wrapper = format!("linc_callable_witness_{}", plan.ordinal);
        source.push_str("extern ");
        source.push_str(&self.render_source_function(plan.function, &target)?);
        source.push_str(";\n");
        source.push_str("__attribute__((used,noinline,visibility(\"hidden\"))) ");
        source.push_str(&self.render_source_function(plan.function, &wrapper)?);
        source.push_str(" {\n  ");
        if !self.is_void_type(&plan.function.return_type, &mut BTreeSet::new())? {
            source.push_str("return ");
        }
        source.push_str(&target);
        source.push('(');
        for (index, _) in plan.function.parameters.iter().enumerate() {
            if index != 0 {
                source.push_str(", ");
            }
            source.push_str(&format!("linc_p_{}_{}", plan.ordinal, index));
        }
        source.push_str(");\n");
        if self.is_void_type(&plan.function.return_type, &mut BTreeSet::new())? {
            source.push_str("  return;\n");
        }
        source.push_str("}\n");
        Ok(())
    }

    fn render_variable_witness(
        &self,
        source: &mut String,
        declaration: DeclarationId,
    ) -> NativeResult<()> {
        let ordinal = self.ordinal(declaration)?;
        let source_declaration = self
            .source
            .source()
            .declaration(declaration)
            .expect("plan declaration");
        let SourceDeclarationKind::Variable(variable) = &source_declaration.kind else {
            unreachable!("variable plan is typed")
        };
        source.push_str("extern ");
        if variable.thread_local {
            source.push_str("_Thread_local ");
        }
        source.push_str(
            &self.render_declaration(&variable.ty, &format!("linc_variable_witness_{ordinal}"))?,
        );
        source.push_str(";\n");
        Ok(())
    }

    fn render_source_function(
        &self,
        function: &SourceFunction,
        name: &str,
    ) -> NativeResult<String> {
        let parameters = function
            .parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                self.render_declaration(&parameter.ty, &format!("linc_p_0_{index}"))
            })
            .collect::<NativeResult<Vec<_>>>()?;
        // Parameter names must be unique per wrapper, not source-derived.  The
        // caller's generated name ends in the stable declaration ordinal.
        let ordinal = name
            .rsplit('_')
            .next()
            .ok_or_else(|| NativeError::ProbeRender {
                detail: "generated callable name has no ordinal".to_owned(),
            })?;
        let parameters = parameters
            .into_iter()
            .enumerate()
            .map(|(index, declaration)| {
                declaration.replace(
                    &format!("linc_p_0_{index}"),
                    &format!("linc_p_{ordinal}_{index}"),
                )
            })
            .collect::<Vec<_>>();
        let list = if parameters.is_empty() {
            "void".to_owned()
        } else {
            parameters.join(", ")
        };
        self.render_declaration(&function.return_type, &format!("{name}({list})"))
    }

    fn render_declaration(&self, ty: &CType, declarator: &str) -> NativeResult<String> {
        self.render_declaration_inner(ty, declarator, &mut BTreeSet::new())
    }

    fn render_declaration_inner(
        &self,
        ty: &CType,
        declarator: &str,
        aliases: &mut BTreeSet<DeclarationId>,
    ) -> NativeResult<String> {
        match &ty.kind {
            CTypeKind::Pointer(pointee) => {
                let pointer_qualifiers = qualifier_words(ty.qualifiers, true)?;
                let mut nested = if pointer_qualifiers.is_empty() {
                    format!("*{declarator}")
                } else {
                    format!("*{pointer_qualifiers} {declarator}")
                };
                if matches!(
                    pointee.kind,
                    CTypeKind::Array { .. } | CTypeKind::Function(_)
                ) {
                    nested = format!("({nested})");
                }
                self.render_declaration_inner(pointee, &nested, aliases)
            }
            CTypeKind::Array {
                element,
                bound: ArrayBound::Fixed { elements },
                parameter_qualifiers,
            } if *elements > 0 => {
                if ty.qualifiers != TypeQualifiers::NONE
                    || *parameter_qualifiers != TypeQualifiers::NONE
                {
                    return Err(unsupported_type(
                        "qualified array declarators are outside the initial renderer",
                    ));
                }
                self.render_declaration_inner(
                    element,
                    &format!("{declarator}[{elements}]"),
                    aliases,
                )
            }
            CTypeKind::Array { .. } => Err(unsupported_type(
                "only nonzero fixed arrays can be rendered",
            )),
            CTypeKind::Function(function) => {
                validate_function_type_profile(function)?;
                if ty.qualifiers != TypeQualifiers::NONE {
                    return Err(unsupported_type(
                        "qualified function types cannot be rendered",
                    ));
                }
                let parameters = function
                    .parameters
                    .iter()
                    .enumerate()
                    .map(|(index, parameter)| {
                        self.render_declaration_inner(
                            &parameter.ty,
                            &format!("linc_fp_{index}"),
                            aliases,
                        )
                    })
                    .collect::<NativeResult<Vec<_>>>()?;
                let list = if parameters.is_empty() {
                    "void".to_owned()
                } else {
                    parameters.join(", ")
                };
                self.render_declaration_inner(
                    &function.return_type,
                    &format!("{declarator}({list})"),
                    aliases,
                )
            }
            CTypeKind::AliasRef(id) => {
                if !aliases.insert(*id) {
                    return Err(NativeError::ProbeRender {
                        detail: format!("alias cycle includes {id}"),
                    });
                }
                let declaration = self.source.source().declaration(*id).ok_or_else(|| {
                    NativeError::ProbeRender {
                        detail: format!("alias {id} is missing"),
                    }
                })?;
                let SourceDeclarationKind::TypeAlias(alias) = &declaration.kind else {
                    return Err(NativeError::ProbeRender {
                        detail: format!("declaration {id} is not an alias"),
                    });
                };
                let mut expanded = alias.target.clone();
                expanded.qualifiers = merge_qualifiers(expanded.qualifiers, ty.qualifiers)?;
                let result = self.render_declaration_inner(&expanded, declarator, aliases);
                aliases.remove(id);
                result
            }
            _ => {
                let qualifiers = qualifier_words(ty.qualifiers, false)?;
                let base = self.base_type(ty)?;
                if qualifiers.is_empty() {
                    Ok(format!("{base} {declarator}"))
                } else {
                    Ok(format!("{qualifiers} {base} {declarator}"))
                }
            }
        }
    }

    fn base_type(&self, ty: &CType) -> NativeResult<String> {
        match &ty.kind {
            CTypeKind::Void => Ok("void".to_owned()),
            CTypeKind::Bool => Ok("_Bool".to_owned()),
            CTypeKind::Integer(integer) => integer_spelling(integer),
            CTypeKind::Floating(floating) => floating_spelling(floating),
            CTypeKind::Complex(floating) => {
                Ok(format!("{} _Complex", floating_spelling(floating)?))
            }
            CTypeKind::RecordRef(id) => {
                let declaration = self.source.source().declaration(*id).ok_or_else(|| {
                    NativeError::ProbeRender {
                        detail: format!("record {id} is missing"),
                    }
                })?;
                let SourceDeclarationKind::Record(record) = &declaration.kind else {
                    return Err(NativeError::ProbeRender {
                        detail: format!("declaration {id} is not a record"),
                    });
                };
                Ok(format!(
                    "{} {}",
                    record_keyword(record.kind),
                    self.record_tag(*id)?
                ))
            }
            CTypeKind::EnumRef(id) => Ok(format!("enum {}", self.enum_tag(*id)?)),
            CTypeKind::Pointer(_)
            | CTypeKind::Array { .. }
            | CTypeKind::Function(_)
            | CTypeKind::AliasRef(_)
            | CTypeKind::Unsupported { .. } => Err(unsupported_type(
                "source type has no certified C17 declarator spelling",
            )),
        }
    }

    fn is_void_type(
        &self,
        ty: &CType,
        aliases: &mut BTreeSet<DeclarationId>,
    ) -> NativeResult<bool> {
        match &ty.kind {
            CTypeKind::Void => Ok(true),
            CTypeKind::AliasRef(id) => {
                if !aliases.insert(*id) {
                    return Err(NativeError::ProbeRender {
                        detail: format!("alias cycle includes {id}"),
                    });
                }
                let declaration = self.source.source().declaration(*id).ok_or_else(|| {
                    NativeError::ProbeRender {
                        detail: format!("alias {id} is missing"),
                    }
                })?;
                let SourceDeclarationKind::TypeAlias(alias) = &declaration.kind else {
                    return Err(NativeError::ProbeRender {
                        detail: format!("declaration {id} is not an alias"),
                    });
                };
                let result = self.is_void_type(&alias.target, aliases);
                aliases.remove(id);
                result
            }
            _ => Ok(false),
        }
    }

    fn record_definition_order(&self) -> NativeResult<Vec<DeclarationId>> {
        let definitions = self
            .records
            .iter()
            .map(|record| (record.declaration, record))
            .collect::<BTreeMap<_, _>>();
        let mut states = BTreeMap::<DeclarationId, u8>::new();
        let mut order = Vec::with_capacity(definitions.len());
        for declaration in definitions.keys().copied() {
            self.visit_record(declaration, &definitions, &mut states, &mut order)?;
        }
        Ok(order)
    }

    fn visit_record(
        &self,
        declaration: DeclarationId,
        definitions: &BTreeMap<DeclarationId, &RecordPlan<'_>>,
        states: &mut BTreeMap<DeclarationId, u8>,
        order: &mut Vec<DeclarationId>,
    ) -> NativeResult<()> {
        match states.get(&declaration).copied() {
            Some(2) => return Ok(()),
            Some(1) => {
                return Err(NativeError::ProbeRender {
                    detail: format!("by-value record dependency cycle includes {declaration}"),
                });
            }
            _ => {}
        }
        states.insert(declaration, 1);
        let plan = definitions[&declaration];
        let mut dependencies = BTreeSet::new();
        for field in &plan.record.fields {
            self.collect_record_dependencies(&field.ty, &mut dependencies, &mut BTreeSet::new())?;
        }
        for dependency in dependencies {
            if definitions.contains_key(&dependency) {
                self.visit_record(dependency, definitions, states, order)?;
            } else {
                return Err(NativeError::ProbeRender {
                    detail: format!(
                        "record {declaration} needs the unavailable definition {dependency}"
                    ),
                });
            }
        }
        states.insert(declaration, 2);
        order.push(declaration);
        Ok(())
    }

    fn collect_record_dependencies(
        &self,
        ty: &CType,
        dependencies: &mut BTreeSet<DeclarationId>,
        aliases: &mut BTreeSet<DeclarationId>,
    ) -> NativeResult<()> {
        match &ty.kind {
            CTypeKind::RecordRef(id) => {
                dependencies.insert(*id);
                Ok(())
            }
            CTypeKind::Array { element, .. } => {
                self.collect_record_dependencies(element, dependencies, aliases)
            }
            CTypeKind::AliasRef(id) => {
                if !aliases.insert(*id) {
                    return Err(NativeError::ProbeRender {
                        detail: format!("alias cycle includes {id}"),
                    });
                }
                let declaration = self.source.source().declaration(*id).ok_or_else(|| {
                    NativeError::ProbeRender {
                        detail: format!("alias {id} is missing"),
                    }
                })?;
                let SourceDeclarationKind::TypeAlias(alias) = &declaration.kind else {
                    return Err(NativeError::ProbeRender {
                        detail: format!("declaration {id} is not an alias"),
                    });
                };
                self.collect_record_dependencies(&alias.target, dependencies, aliases)?;
                aliases.remove(id);
                Ok(())
            }
            // Pointer and function boundaries do not require a complete record.
            _ => Ok(()),
        }
    }

    fn render_blob(&self, source: &mut String) -> NativeResult<()> {
        let mut bytes = Vec::<String>::new();
        push_literal_bytes(&mut bytes, BLOB_MAGIC);
        push_u64(&mut bytes, BLOB_VERSION);
        push_literal_bytes(&mut bytes, self.source.source().fingerprint().as_bytes());
        push_literal_bytes(
            &mut bytes,
            self.source.source().target_fingerprint().as_bytes(),
        );
        let measured_count = self
            .records
            .len()
            .checked_add(self.enums.len())
            .ok_or_else(|| NativeError::ProbeRender {
                detail: "measurement subject count overflow".to_owned(),
            })?;
        push_u64(&mut bytes, measured_count as u64);

        for plan in &self.records {
            let keyword = record_keyword(plan.record.kind);
            push_u64(&mut bytes, RECORD_KIND);
            push_literal_bytes(&mut bytes, plan.declaration.as_bytes());
            push_u64_expression(
                &mut bytes,
                format!("sizeof({keyword} linc_record_{}) * 8ULL", plan.ordinal),
            );
            push_u64_expression(
                &mut bytes,
                format!("_Alignof({keyword} linc_record_{}) * 8ULL", plan.ordinal),
            );
            push_u64(&mut bytes, plan.fields.len() as u64);
            for field in &plan.fields {
                push_literal_bytes(&mut bytes, field.field.id.as_bytes());
                if field.field.bit_width.is_some() {
                    push_u64(&mut bytes, BITFIELD_OFFSET_SENTINEL);
                    let Some(BitWidth::Known { bits }) = field.field.bit_width else {
                        unreachable!("profile checked")
                    };
                    push_u64(&mut bytes, bits);
                    push_u64_expression(
                        &mut bytes,
                        format!(
                            "_Alignof({}) * 8ULL",
                            field.bitfield_type.as_ref().expect("bitfield alias")
                        ),
                    );
                } else {
                    push_u64_expression(
                        &mut bytes,
                        format!(
                            "__builtin_offsetof({keyword} linc_record_{}, {}) * 8ULL",
                            plan.ordinal, field.name,
                        ),
                    );
                    push_u64_expression(
                        &mut bytes,
                        format!(
                            "sizeof((({keyword} linc_record_{} *)0)->{}) * 8ULL",
                            plan.ordinal, field.name,
                        ),
                    );
                    push_u64_expression(
                        &mut bytes,
                        format!(
                            "__alignof__((({keyword} linc_record_{} *)0)->{}) * 8ULL",
                            plan.ordinal, field.name,
                        ),
                    );
                }
            }
        }
        for plan in &self.enums {
            push_u64(&mut bytes, ENUM_KIND);
            push_literal_bytes(&mut bytes, plan.declaration.as_bytes());
            push_u64_expression(
                &mut bytes,
                format!("sizeof(enum linc_enum_{}) * 8ULL", plan.ordinal),
            );
            push_u64_expression(
                &mut bytes,
                format!("_Alignof(enum linc_enum_{}) * 8ULL", plan.ordinal),
            );
            push_u64_expression(
                &mut bytes,
                format!(
                    "(((enum linc_enum_{0})-1 < (enum linc_enum_{0})0) ? 1ULL : 0ULL)",
                    plan.ordinal
                ),
            );
            push_u64(&mut bytes, plan.enumeration.variants.len() as u64);
        }

        source.push_str(&format!(
            "__attribute__((used,visibility(\"hidden\"))) const unsigned char {BLOB_SYMBOL}[] = {{\n"
        ));
        for chunk in bytes.chunks(8) {
            source.push_str("  ");
            source.push_str(&chunk.join(", "));
            source.push_str(",\n");
        }
        source.push_str("};\n");
        Ok(())
    }

    fn render_bitfield_objects(
        &self,
        source: &mut String,
        plan: &RecordPlan<'_>,
    ) -> NativeResult<()> {
        for field in &plan.fields {
            let Some(symbol) = &field.bitfield_symbol else {
                continue;
            };
            let keyword = record_keyword(plan.record.kind);
            source.push_str(&format!(
                "__attribute__((used,visibility(\"hidden\"))) const union {{ {keyword} linc_record_{0} value; unsigned char bytes[sizeof({keyword} linc_record_{0})]; }} {1} = {{ .value = {{ .{2} = -1 }} }};\n",
                plan.ordinal, symbol, field.name,
            ));
        }
        Ok(())
    }
}

fn record_keyword(kind: RecordKind) -> &'static str {
    match kind {
        RecordKind::Struct => "struct",
        RecordKind::Union => "union",
    }
}

fn scalar_assert(source: &mut String, spelling: &str, layout: parc::contract::ScalarLayout) {
    source.push_str(&format!(
        "_Static_assert(sizeof({spelling}) * 8ULL == {}ULL, \"scalar size\");\n",
        layout.storage_bits
    ));
    source.push_str(&format!(
        "_Static_assert(_Alignof({spelling}) * 8ULL == {}ULL, \"scalar alignment\");\n",
        layout.alignment_bits
    ));
}

fn qualifier_words(qualifiers: TypeQualifiers, pointer: bool) -> NativeResult<String> {
    if qualifiers.is_atomic {
        return Err(unsupported_type("atomic qualifiers cannot be rendered"));
    }
    if qualifiers.is_restrict && !pointer {
        return Err(unsupported_type(
            "restrict is accepted only as a pointer qualifier",
        ));
    }
    let mut words = Vec::new();
    if qualifiers.is_const {
        words.push("const");
    }
    if qualifiers.is_volatile {
        words.push("volatile");
    }
    if qualifiers.is_restrict {
        words.push("restrict");
    }
    Ok(words.join(" "))
}

fn merge_qualifiers(left: TypeQualifiers, right: TypeQualifiers) -> NativeResult<TypeQualifiers> {
    if left.is_atomic || right.is_atomic {
        return Err(unsupported_type(
            "atomic aliases are outside the initial renderer",
        ));
    }
    Ok(TypeQualifiers {
        is_const: left.is_const || right.is_const,
        is_volatile: left.is_volatile || right.is_volatile,
        is_restrict: left.is_restrict || right.is_restrict,
        is_atomic: false,
    })
}

fn integer_spelling(integer: &CIntegerType) -> NativeResult<String> {
    let spelling = match integer {
        CIntegerType::Char {
            signedness: CharTypeSignedness::Plain,
        } => "char",
        CIntegerType::Char {
            signedness: CharTypeSignedness::Signed,
        } => "signed char",
        CIntegerType::Char {
            signedness: CharTypeSignedness::Unsigned,
        } => "unsigned char",
        CIntegerType::Short {
            signedness: Signedness::Signed,
        } => "short",
        CIntegerType::Short {
            signedness: Signedness::Unsigned,
        } => "unsigned short",
        CIntegerType::Int {
            signedness: Signedness::Signed,
        } => "int",
        CIntegerType::Int {
            signedness: Signedness::Unsigned,
        } => "unsigned int",
        CIntegerType::Long {
            signedness: Signedness::Signed,
        } => "long",
        CIntegerType::Long {
            signedness: Signedness::Unsigned,
        } => "unsigned long",
        CIntegerType::LongLong {
            signedness: Signedness::Signed,
        } => "long long",
        CIntegerType::LongLong {
            signedness: Signedness::Unsigned,
        } => "unsigned long long",
        CIntegerType::Int128 {
            signedness: Signedness::Signed,
        } => "__int128",
        CIntegerType::Int128 {
            signedness: Signedness::Unsigned,
        } => "unsigned __int128",
        CIntegerType::BitInt { .. } => {
            return Err(NativeError::InvalidPolicy {
                detail: "_BitInt requires an explicit future measured scalar profile".to_owned(),
            });
        }
    };
    Ok(spelling.to_owned())
}

fn floating_spelling(floating: &CFloatingType) -> NativeResult<String> {
    match floating {
        CFloatingType::Float => Ok("float".to_owned()),
        CFloatingType::Double => Ok("double".to_owned()),
        CFloatingType::LongDouble => Ok("long double".to_owned()),
        CFloatingType::Float128 | CFloatingType::Ts18661 { .. } => Err(unsupported_type(
            "extended floating spellings are outside the initial renderer",
        )),
    }
}

fn exact_integer_literal(value: ExactInteger) -> NativeResult<String> {
    match value {
        ExactInteger::Signed { value } => {
            if value == i128::MIN {
                return Err(unsupported_type(
                    "the minimum signed 128-bit enum literal is outside the C17 renderer",
                ));
            }
            Ok(value.to_string())
        }
        ExactInteger::Unsigned { value } if value <= u64::MAX as u128 => Ok(format!("{value}ULL")),
        ExactInteger::Unsigned { .. } => Err(unsupported_type(
            "enum values wider than unsigned long long are outside the C17 renderer",
        )),
    }
}

fn push_literal_bytes(output: &mut Vec<String>, bytes: &[u8]) {
    output.extend(bytes.iter().map(u8::to_string));
}

fn push_u64(output: &mut Vec<String>, value: u64) {
    push_literal_bytes(output, &value.to_le_bytes());
}

fn push_u64_expression(output: &mut Vec<String>, expression: String) {
    for shift in (0..64).step_by(8) {
        output.push(format!(
            "(unsigned char)(((unsigned long long)({expression}) >> {shift}) & 255ULL)"
        ));
    }
}

#[derive(Debug, Clone)]
struct RawRecordMeasurement {
    declaration: DeclarationId,
    size_bits: u64,
    alignment_bits: u32,
    fields: Vec<RawFieldMeasurement>,
}

#[derive(Debug, Clone)]
struct RawFieldMeasurement {
    child: ChildId,
    offset_bits: u64,
    size_bits: u64,
    alignment_bits: u32,
}

#[derive(Debug, Clone)]
struct RawEnumMeasurement {
    declaration: DeclarationId,
    storage_bits: u64,
    alignment_bits: u32,
    signedness: Signedness,
}

#[derive(Debug, Clone, Default)]
struct RawMeasurements {
    records: Vec<RawRecordMeasurement>,
    enums: Vec<RawEnumMeasurement>,
}

pub(super) fn certify_input(
    request: &AnalysisRequest<'_>,
    toolchain: &CertificationToolchain,
) -> NativeResult<NativeAnalysisInput> {
    validate_certification_request(request)?;
    if toolchain.compiler_identity() != request.source().source().target().compiler() {
        return Err(NativeError::ToolIdentity {
            path: toolchain.compiler_executable().to_path_buf(),
            detail: "observed certification toolchain identity differs from TargetSpec".to_owned(),
        });
    }
    if toolchain.compiler_sysroot().is_some() {
        return Err(NativeError::ToolIdentity {
            path: toolchain.compiler_executable().to_path_buf(),
            detail: "the initial certification profile requires an empty compiler sysroot"
                .to_owned(),
        });
    }
    let plan = CertificationPlan::build(request.source())?;
    let source = plan.render_probe()?;
    let compilation = compile_owned_probe(
        request.source().source().target(),
        toolchain.compiler_executable(),
        toolchain.environment(),
        request.policy().probe_execution(),
        &source,
    )?;
    if compilation.compiler_resource_dir.as_deref() != toolchain.compiler_resource_dir() {
        return Err(NativeError::ToolIdentity {
            path: toolchain.compiler_executable().to_path_buf(),
            detail: "compiler resource directory changed after toolchain observation".to_owned(),
        });
    }
    let measurements = decode_measurements(&plan, &compilation.object)?;
    construct_certified_input(&plan, measurements, compilation)
}

fn decode_measurements(
    plan: &CertificationPlan<'_>,
    object_bytes: &[u8],
) -> NativeResult<RawMeasurements> {
    let file = object::File::parse(object_bytes).map_err(|error| NativeError::ProbeParserGap {
        detail: format!("measurement output is not a parseable object: {error}"),
    })?;
    if file.format() != object::BinaryFormat::Elf
        || file.architecture() != object::Architecture::X86_64
        || !file.is_little_endian()
        || !file.is_64()
    {
        return Err(NativeError::ProbeParserGap {
            detail: "measurement object is not little-endian x86-64 ELF".to_owned(),
        });
    }
    let blob = extract_unique_symbol(&file, BLOB_SYMBOL)?;
    let mut reader = BlobReader::new(blob);
    reader.expect_bytes(BLOB_MAGIC, "blob magic")?;
    if reader.u64("blob version")? != BLOB_VERSION {
        return Err(parser_gap("unsupported measurement blob version"));
    }
    reader.expect_bytes(
        plan.source.source().fingerprint().as_bytes(),
        "source fingerprint",
    )?;
    reader.expect_bytes(
        plan.source.source().target_fingerprint().as_bytes(),
        "target fingerprint",
    )?;
    let expected_subjects = plan
        .records
        .len()
        .checked_add(plan.enums.len())
        .ok_or_else(|| parser_gap("measurement subject count overflow"))?;
    if reader.u64("measurement subject count")?
        != u64::try_from(expected_subjects)
            .map_err(|_| parser_gap("measurement subject count does not fit u64"))?
    {
        return Err(parser_gap(
            "measurement subject count differs from the plan",
        ));
    }

    let mut measurements = RawMeasurements::default();
    for expected in &plan.records {
        if reader.u64("record kind")? != RECORD_KIND {
            return Err(parser_gap("record measurement kind differs from the plan"));
        }
        reader.expect_bytes(expected.declaration.as_bytes(), "record declaration ID")?;
        let size_bits = reader.u64("record size")?;
        let alignment_bits = reader.u32_from_u64("record alignment")?;
        if reader.u64("record field count")?
            != u64::try_from(expected.fields.len())
                .map_err(|_| parser_gap("record field count does not fit u64"))?
        {
            return Err(parser_gap("record field count differs from the plan"));
        }
        if size_bits == 0 || !size_bits.is_multiple_of(8) {
            return Err(parser_gap("record size is zero or not byte-addressable"));
        }
        let mut fields = Vec::with_capacity(expected.fields.len());
        for field in &expected.fields {
            reader.expect_bytes(field.field.id.as_bytes(), "record child ID")?;
            let encoded_offset = reader.u64("field offset")?;
            let size = reader.u64("field size")?;
            let alignment = reader.u32_from_u64("field alignment")?;
            let offset = if encoded_offset == BITFIELD_OFFSET_SENTINEL {
                let symbol = field.bitfield_symbol.as_deref().ok_or_else(|| {
                    parser_gap("ordinary field carried a bitfield offset sentinel")
                })?;
                bitfield_offset(extract_unique_symbol(&file, symbol)?, size, size_bits)?
            } else {
                if field.bitfield_symbol.is_some() {
                    return Err(parser_gap("bitfield did not carry its offset sentinel"));
                }
                encoded_offset
            };
            let end = offset
                .checked_add(size)
                .ok_or_else(|| parser_gap("field extent overflow"))?;
            if size == 0 || end > size_bits {
                return Err(parser_gap("field extent is outside its record"));
            }
            fields.push(RawFieldMeasurement {
                child: field.field.id,
                offset_bits: offset,
                size_bits: size,
                alignment_bits: alignment,
            });
        }
        measurements.records.push(RawRecordMeasurement {
            declaration: expected.declaration,
            size_bits,
            alignment_bits,
            fields,
        });
    }
    for expected in &plan.enums {
        if reader.u64("enum kind")? != ENUM_KIND {
            return Err(parser_gap("enum measurement kind differs from the plan"));
        }
        reader.expect_bytes(expected.declaration.as_bytes(), "enum declaration ID")?;
        let storage_bits = reader.u64("enum storage")?;
        let alignment_bits = reader.u32_from_u64("enum alignment")?;
        let signedness = match reader.u64("enum signedness")? {
            0 => Signedness::Unsigned,
            1 => Signedness::Signed,
            _ => return Err(parser_gap("enum signedness is not canonical")),
        };
        if reader.u64("enum variant count")?
            != u64::try_from(expected.enumeration.variants.len())
                .map_err(|_| parser_gap("enum variant count does not fit u64"))?
        {
            return Err(parser_gap("enum variant count differs from the plan"));
        }
        measurements.enums.push(RawEnumMeasurement {
            declaration: expected.declaration,
            storage_bits,
            alignment_bits,
            signedness,
        });
    }
    reader.finish()?;
    Ok(measurements)
}

fn extract_unique_symbol<'data>(
    file: &object::File<'data>,
    expected_name: &str,
) -> NativeResult<&'data [u8]> {
    let mut matching = file.symbols().filter(|symbol| {
        !symbol.is_undefined()
            && symbol
                .name_bytes()
                .is_ok_and(|name| name == expected_name.as_bytes())
    });
    let symbol = matching
        .next()
        .ok_or_else(|| parser_gap(format!("measurement symbol {expected_name:?} is missing")))?;
    if matching.next().is_some() {
        return Err(parser_gap(format!(
            "measurement symbol {expected_name:?} is duplicated"
        )));
    }
    let section_index = symbol.section_index().ok_or_else(|| {
        parser_gap(format!(
            "measurement symbol {expected_name:?} has no section"
        ))
    })?;
    let section = file.section_by_index(section_index).map_err(|error| {
        parser_gap(format!(
            "measurement symbol {expected_name:?} has an invalid section: {error}"
        ))
    })?;
    let section_data = section.data().map_err(|error| {
        parser_gap(format!(
            "measurement symbol {expected_name:?} section cannot be read: {error}"
        ))
    })?;
    let relative = symbol
        .address()
        .checked_sub(section.address())
        .ok_or_else(|| {
            parser_gap(format!(
                "measurement symbol {expected_name:?} precedes its section"
            ))
        })?;
    let start = usize::try_from(relative)
        .map_err(|_| parser_gap("measurement symbol offset does not fit memory"))?;
    let size = usize::try_from(symbol.size())
        .map_err(|_| parser_gap("measurement symbol size does not fit memory"))?;
    if size == 0 {
        return Err(parser_gap(format!(
            "measurement symbol {expected_name:?} is empty"
        )));
    }
    let end = start
        .checked_add(size)
        .ok_or_else(|| parser_gap("measurement symbol extent overflow"))?;
    section_data.get(start..end).ok_or_else(|| {
        parser_gap(format!(
            "measurement symbol {expected_name:?} extends beyond its section"
        ))
    })
}

fn bitfield_offset(bytes: &[u8], width_bits: u64, record_size_bits: u64) -> NativeResult<u64> {
    let expected_bytes = record_size_bits / 8;
    if bytes.len() as u64 != expected_bytes || width_bits == 0 {
        return Err(parser_gap(
            "bitfield witness size or declared width is incoherent",
        ));
    }
    let mut set_bits = Vec::new();
    for (byte_index, byte) in bytes.iter().copied().enumerate() {
        for bit in 0..8_u8 {
            if byte & (1_u8 << bit) != 0 {
                let offset = (byte_index as u64)
                    .checked_mul(8)
                    .and_then(|offset| offset.checked_add(u64::from(bit)))
                    .ok_or_else(|| parser_gap("bitfield bit offset overflow"))?;
                set_bits.push(offset);
            }
        }
    }
    if set_bits.len() as u64 != width_bits {
        return Err(parser_gap(
            "bitfield witness has a set-bit count different from its width",
        ));
    }
    let first = *set_bits
        .first()
        .ok_or_else(|| parser_gap("bitfield witness has no set bit"))?;
    if set_bits
        .iter()
        .enumerate()
        .any(|(index, bit)| *bit != first + index as u64)
    {
        return Err(parser_gap(
            "bitfield witness is not one contiguous little-endian range",
        ));
    }
    Ok(first)
}

struct BlobReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> BlobReader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize, field: &str) -> NativeResult<&'a [u8]> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| parser_gap(format!("{field} offset overflow")))?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| parser_gap(format!("measurement blob is truncated at {field}")))?;
        self.offset = end;
        Ok(value)
    }

    fn expect_bytes(&mut self, expected: &[u8], field: &str) -> NativeResult<()> {
        if self.take(expected.len(), field)? != expected {
            return Err(parser_gap(format!("measurement {field} differs")));
        }
        Ok(())
    }

    fn u64(&mut self, field: &str) -> NativeResult<u64> {
        let bytes: [u8; 8] = self
            .take(8, field)?
            .try_into()
            .expect("take returned exactly eight bytes");
        Ok(u64::from_le_bytes(bytes))
    }

    fn u32_from_u64(&mut self, field: &str) -> NativeResult<u32> {
        u32::try_from(self.u64(field)?)
            .map_err(|_| parser_gap(format!("measurement {field} exceeds u32")))
    }

    fn finish(self) -> NativeResult<()> {
        if self.offset != self.bytes.len() {
            return Err(parser_gap("measurement blob has trailing bytes"));
        }
        Ok(())
    }
}

fn parser_gap(detail: impl Into<String>) -> NativeError {
    NativeError::ProbeParserGap {
        detail: detail.into(),
    }
}

fn construct_certified_input(
    plan: &CertificationPlan<'_>,
    measurements: RawMeasurements,
    compilation: OwnedProbeCompilation,
) -> NativeResult<NativeAnalysisInput> {
    let placeholder = ProbeEvidenceId::from_str(&format!("lprobe1_{}", "0".repeat(64)))
        .expect("canonical placeholder probe ID");
    let prototype_layouts = build_layouts(plan, &measurements, placeholder)?;
    let prototype_shapes = build_shapes(plan, &prototype_layouts, placeholder)?;

    let mut subjects = Vec::new();
    let mut outcomes = Vec::new();
    for layout in &prototype_layouts {
        let subject = match layout {
            LayoutEvidence::Record(record) => ProbeSubject::RecordLayout {
                declaration: record.declaration(),
            },
            LayoutEvidence::Enum(enumeration) => ProbeSubject::EnumRepresentation {
                declaration: enumeration.declaration(),
            },
        };
        subjects.push(subject);
        outcomes.push(ProbeSubjectOutcome::try_new(
            subject,
            ProbeSubjectStatus::Verified {
                evidence_fingerprint: layout.fingerprint()?,
            },
        )?);
    }
    for shape in &prototype_shapes {
        let subject = ProbeSubject::CallableAbi {
            declaration: shape.declaration(),
        };
        subjects.push(subject);
        outcomes.push(ProbeSubjectOutcome::try_new(
            subject,
            ProbeSubjectStatus::Verified {
                evidence_fingerprint: shape.fingerprint()?,
            },
        )?);
    }
    if subjects.is_empty() {
        return Err(unsupported_type(
            "certification requires at least one measured layout or callable subject",
        ));
    }

    let target = plan.source.source().target();
    let evidence = AbiProbeEvidence::try_new(AbiProbeEvidenceInput {
        source_fingerprint: plan.source.source().fingerprint(),
        target_fingerprint: plan.source.source().target_fingerprint(),
        compiler: compilation.compiler,
        compiler_executable: compilation.compiler_executable,
        compiler_arguments: compilation.compiler_arguments,
        abi_flags: target.abi_flags().to_vec(),
        probe_source_fingerprint: compilation.source_fingerprint,
        subjects,
        // The same compiler invocation both emits structural layout facts and
        // compiles ABI-lowered callable witnesses. CompileTimeAssertion is the
        // contract method which is valid for that combined subject set.
        method: ProbeMethod::CompileTimeAssertion,
        execution_policy: compilation.execution_policy,
        compile_result: compilation.compile_result,
        runner: ProbeRunnerEvidence::NotExecuted,
        execution_result: None,
        subject_outcomes: outcomes,
    })?;
    let layouts = build_layouts(plan, &measurements, evidence.id())?;
    let shapes = build_shapes(plan, &layouts, evidence.id())?;
    if layouts
        .iter()
        .zip(&prototype_layouts)
        .any(|(final_layout, prototype)| {
            final_layout.fingerprint().ok() != prototype.fingerprint().ok()
        })
        || shapes
            .iter()
            .zip(&prototype_shapes)
            .any(|(final_shape, prototype)| {
                final_shape.fingerprint().ok() != prototype.fingerprint().ok()
            })
    {
        return Err(parser_gap(
            "evidence shape changed while binding the final probe identity",
        ));
    }

    let shape_map = shapes
        .into_iter()
        .map(|shape| (shape.declaration(), shape))
        .collect::<BTreeMap<_, _>>();
    let mut declarations = Vec::with_capacity(plan.functions.len() + plan.variables.len());
    for function in &plan.functions {
        let shape = shape_map
            .get(&function.declaration)
            .cloned()
            .ok_or_else(|| parser_gap("certified callable shape is missing"))?;
        declarations.push(NativeDeclarationRequest::new(
            function.declaration,
            SymbolDecoration::None,
            CallableAbiAssessment::Confirmed {
                calling_convention: function.function.calling_convention.clone(),
                confidence: EvidenceConfidence::Corroborated,
                probe: evidence.id(),
            },
            Some(shape),
        ));
    }
    for variable in &plan.variables {
        declarations.push(NativeDeclarationRequest::new(
            *variable,
            SymbolDecoration::None,
            CallableAbiAssessment::NotApplicable,
            None,
        ));
    }
    Ok(NativeAnalysisInput {
        abi_probes: vec![evidence],
        layouts,
        declarations,
        diagnostics: Vec::new(),
    })
}

fn build_layouts(
    plan: &CertificationPlan<'_>,
    measurements: &RawMeasurements,
    probe: ProbeEvidenceId,
) -> NativeResult<Vec<LayoutEvidence>> {
    if measurements.records.len() != plan.records.len()
        || measurements.enums.len() != plan.enums.len()
    {
        return Err(parser_gap(
            "decoded measurement cardinality differs from the certification plan",
        ));
    }
    let source_fingerprint = plan.source.source().fingerprint();
    let target_fingerprint = plan.source.source().target_fingerprint();
    let mut layouts = Vec::with_capacity(measurements.records.len() + measurements.enums.len());
    for (raw, expected) in measurements.records.iter().zip(&plan.records) {
        if raw.declaration != expected.declaration || raw.fields.len() != expected.fields.len() {
            return Err(parser_gap("decoded record measurement is out of order"));
        }
        let fields = raw
            .fields
            .iter()
            .map(|field| {
                FieldLayoutEvidence::try_new(
                    field.child,
                    field.offset_bits,
                    Some(field.size_bits),
                    Some(field.alignment_bits),
                )
                .map_err(NativeError::from)
            })
            .collect::<NativeResult<Vec<_>>>()?;
        layouts.push(LayoutEvidence::Record(RecordLayoutEvidence::try_new(
            raw.declaration,
            source_fingerprint,
            target_fingerprint,
            raw.size_bits,
            raw.alignment_bits,
            fields,
            probe,
            EvidenceSource::CompilerProbe,
            EvidenceConfidence::Measured,
        )?));
    }
    for (raw, expected) in measurements.enums.iter().zip(&plan.enums) {
        if raw.declaration != expected.declaration {
            return Err(parser_gap("decoded enum measurement is out of order"));
        }
        let variants = expected
            .enumeration
            .variants
            .iter()
            .map(|variant| {
                let EnumValue::Evaluated { value } = variant.value else {
                    unreachable!("renderer checked evaluated enum values")
                };
                EnumVariantEvidence::new(variant.id, value)
            })
            .collect();
        layouts.push(LayoutEvidence::Enum(EnumLayoutEvidence::try_new(
            raw.declaration,
            source_fingerprint,
            target_fingerprint,
            raw.storage_bits,
            raw.alignment_bits,
            raw.signedness,
            variants,
            probe,
            EvidenceSource::CompilerProbe,
            EvidenceConfidence::Measured,
        )?));
    }
    layouts.sort_by_key(LayoutEvidence::declaration);
    Ok(layouts)
}

fn build_shapes(
    plan: &CertificationPlan<'_>,
    layouts: &[LayoutEvidence],
    probe: ProbeEvidenceId,
) -> NativeResult<Vec<AbiShapeEvidence>> {
    let source_fingerprint = plan.source.source().fingerprint();
    let target_fingerprint = plan.source.source().target_fingerprint();
    let mut shapes = Vec::with_capacity(plan.functions.len());
    for plan_function in &plan.functions {
        let function = plan_function.function;
        let classification = classify_sysv64_callable(plan.source, function, layouts)?;
        if classification.parameters().len() != function.parameters.len() {
            return Err(parser_gap(
                "SysV classifier returned the wrong parameter cardinality",
            ));
        }
        let parameters = function
            .parameters
            .iter()
            .zip(classification.parameters())
            .map(|(parameter, passing)| {
                let (size, alignment) =
                    sysv64_parameter_layout(plan.source, &parameter.ty, layouts)?;
                AbiDimension::try_new(&parameter.ty, size, alignment, *passing)
            })
            .collect::<NativeResult<Vec<_>>>()?;
        let (return_size, return_alignment) =
            sysv64_return_layout(plan.source, &function.return_type, layouts)?;
        let return_value = AbiDimension::try_new(
            &function.return_type,
            return_size,
            return_alignment,
            classification.return_value(),
        )?;
        shapes.push(AbiShapeEvidence::try_new(
            plan_function.declaration,
            source_fingerprint,
            target_fingerprint,
            function.calling_convention.clone(),
            false,
            parameters,
            return_value,
            classification.return_convention(),
            probe,
        )?);
    }
    shapes.sort_by_key(AbiShapeEvidence::declaration);
    Ok(shapes)
}
