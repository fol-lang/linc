use std::collections::BTreeSet;

use parc::contract::{
    ArrayBound, CDataModel, CFloatingType, CIntegerType, CType, CTypeKind, CallingConvention,
    CompleteSourcePackage, ContentFingerprint, DeclarationId, FunctionPrototype,
    SourceDeclarationKind, SourceFingerprint, TargetFingerprint,
};
use serde::Serialize;

use crate::contract::{
    canonical_symbol_spelling, AbiProbeEvidence, ArtifactSymbolId, CallableAbiAssessment,
    DeclarationEvidence, DeclarationEvidenceInput, LayoutAssessment, LayoutEvidence,
    ProbeEvidenceId, ProbeSubject, ProviderAssessment, SymbolAssessment, SymbolBinding,
    SymbolDecoration, SymbolDirection, SymbolInventory, SymbolKind, SymbolReference,
    SymbolVisibility, WeakSymbolPolicy,
};

use super::{NativeError, NativeResolution, NativeResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValuePassing {
    Direct,
    Indirect,
    SplitRegisters,
    Ignore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReturnConvention {
    Void,
    Direct,
    IndirectSret,
    RegisterPair,
}

/// One measured ABI dimension, bound to the exact canonical PARC type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AbiDimension {
    source_type: ContentFingerprint,
    size_bits: u64,
    alignment_bits: u32,
    passing: ValuePassing,
}

impl AbiDimension {
    pub fn try_new(
        ty: &CType,
        size_bits: u64,
        alignment_bits: u32,
        passing: ValuePassing,
    ) -> NativeResult<Self> {
        if alignment_bits < 8 || !alignment_bits.is_power_of_two() {
            return Err(NativeError::InvalidPolicy {
                detail: "ABI dimension alignment must be a power of two of at least 8 bits"
                    .to_owned(),
            });
        }
        if size_bits == 0 && passing != ValuePassing::Ignore {
            return Err(NativeError::InvalidPolicy {
                detail: "non-ignored ABI dimension must have nonzero size".to_owned(),
            });
        }
        Ok(Self {
            source_type: type_fingerprint(ty)?,
            size_bits,
            alignment_bits,
            passing,
        })
    }

    pub const fn source_type(&self) -> ContentFingerprint {
        self.source_type
    }

    pub const fn size_bits(&self) -> u64 {
        self.size_bits
    }

    pub const fn alignment_bits(&self) -> u32 {
        self.alignment_bits
    }

    pub const fn passing(&self) -> ValuePassing {
        self.passing
    }
}

/// Typed callable measurement. Its fingerprint is what a verified callable
/// probe outcome binds into the durable contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AbiShapeEvidence {
    declaration: DeclarationId,
    source_fingerprint: SourceFingerprint,
    target_fingerprint: TargetFingerprint,
    calling_convention: CallingConvention,
    variadic: bool,
    parameters: Vec<AbiDimension>,
    return_value: AbiDimension,
    return_convention: ReturnConvention,
    probe: ProbeEvidenceId,
}

impl AbiShapeEvidence {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        declaration: DeclarationId,
        source_fingerprint: SourceFingerprint,
        target_fingerprint: TargetFingerprint,
        calling_convention: CallingConvention,
        variadic: bool,
        parameters: Vec<AbiDimension>,
        return_value: AbiDimension,
        return_convention: ReturnConvention,
        probe: ProbeEvidenceId,
    ) -> NativeResult<Self> {
        Ok(Self {
            declaration,
            source_fingerprint,
            target_fingerprint,
            calling_convention,
            variadic,
            parameters,
            return_value,
            return_convention,
            probe,
        })
    }

    pub const fn declaration(&self) -> DeclarationId {
        self.declaration
    }

    pub const fn source_fingerprint(&self) -> SourceFingerprint {
        self.source_fingerprint
    }

    pub const fn target_fingerprint(&self) -> TargetFingerprint {
        self.target_fingerprint
    }

    pub const fn calling_convention(&self) -> &CallingConvention {
        &self.calling_convention
    }

    pub const fn variadic(&self) -> bool {
        self.variadic
    }

    pub fn parameters(&self) -> &[AbiDimension] {
        &self.parameters
    }

    pub const fn return_value(&self) -> &AbiDimension {
        &self.return_value
    }

    pub const fn return_convention(&self) -> ReturnConvention {
        self.return_convention
    }

    pub const fn probe(&self) -> ProbeEvidenceId {
        self.probe
    }

    pub fn fingerprint(&self) -> NativeResult<ContentFingerprint> {
        #[derive(Serialize)]
        struct CallableShapeFingerprint<'a> {
            domain: &'static str,
            declaration: DeclarationId,
            source_fingerprint: SourceFingerprint,
            target_fingerprint: TargetFingerprint,
            calling_convention: &'a CallingConvention,
            variadic: bool,
            parameters: &'a [AbiDimension],
            return_value: &'a AbiDimension,
            return_convention: ReturnConvention,
        }

        // Probe identity is deliberately excluded. The probe outcome binds
        // this fingerprint while the shape separately points back to that
        // probe; including the probe ID here would create a cryptographic
        // cycle because ProbeEvidenceId also commits to subject outcomes.
        serde_json::to_vec(&CallableShapeFingerprint {
            domain: "follang.linc.callable-abi-shape.v1",
            declaration: self.declaration,
            source_fingerprint: self.source_fingerprint,
            target_fingerprint: self.target_fingerprint,
            calling_convention: &self.calling_convention,
            variadic: self.variadic,
            parameters: &self.parameters,
            return_value: &self.return_value,
            return_convention: self.return_convention,
        })
        .map(|bytes| ContentFingerprint::from_content(&bytes))
        .map_err(|error| NativeError::InvalidPolicy {
            detail: format!("cannot canonicalize callable ABI evidence: {error}"),
        })
    }
}

pub struct StrictDeclarationRequest<'a> {
    pub declaration: DeclarationId,
    pub decoration: SymbolDecoration,
    pub layout: LayoutAssessment,
    pub callable_abi: CallableAbiAssessment,
    pub abi_shape: Option<&'a AbiShapeEvidence>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StrictEvidenceValidator;

impl StrictEvidenceValidator {
    pub fn validate_declaration(
        &self,
        source: &CompleteSourcePackage,
        resolution: &NativeResolution,
        probes: &[AbiProbeEvidence],
        layouts: &[LayoutEvidence],
        request: StrictDeclarationRequest<'_>,
    ) -> NativeResult<DeclarationEvidence> {
        if resolution.source_fingerprint() != source.source().fingerprint()
            || resolution.target_fingerprint() != source.source().target_fingerprint()
        {
            return Err(NativeError::AbiMismatch {
                declaration: request.declaration,
                detail: "resolved providers are bound to a different source or target fingerprint"
                    .to_owned(),
            });
        }
        let declaration = source
            .source()
            .declaration(request.declaration)
            .ok_or_else(|| NativeError::AbiMismatch {
                declaration: request.declaration,
                detail: "declaration ID is absent from the complete source package".to_owned(),
            })?;
        let (expected_name, expected_kind, expected_convention) = match &declaration.kind {
            SourceDeclarationKind::Function(function) => (
                function.link_name.as_str(),
                SymbolKind::Function,
                Some(&function.calling_convention),
            ),
            SourceDeclarationKind::Variable(variable) => (
                variable.link_name.as_str(),
                if variable.thread_local {
                    SymbolKind::ThreadLocal
                } else {
                    SymbolKind::Data
                },
                None,
            ),
            _ => {
                return Err(NativeError::SymbolRejected {
                    declaration: request.declaration,
                    detail: "only linked functions and variables may request provider evidence"
                        .to_owned(),
                });
            }
        };

        let provider_ids = resolution
            .plan()
            .atoms()
            .iter()
            .filter_map(|atom| atom.artifact().map(|artifact| artifact.provider_id()))
            .collect::<BTreeSet<_>>();
        let inventories = resolution
            .inventories()
            .iter()
            .filter(|inventory| provider_ids.contains(&inventory.artifact().provider_id()))
            .collect::<Vec<_>>();
        let confirmed_convention = match &request.callable_abi {
            CallableAbiAssessment::Confirmed {
                calling_convention, ..
            } => Some(calling_convention),
            _ => None,
        };
        let mut named = Vec::<(&SymbolInventory, ArtifactSymbolId)>::new();
        for inventory in inventories {
            let actual = canonical_symbol_spelling(
                expected_name,
                &request.decoration,
                inventory.artifact().observed_target(),
                expected_convention,
                confirmed_convention,
            )
            .map_err(|detail| NativeError::SymbolRejected {
                declaration: request.declaration,
                detail: detail.to_owned(),
            })?;
            named.extend(
                inventory
                    .symbols()
                    .iter()
                    .filter(|symbol| {
                        symbol.name() == actual && symbol.decoration() == &request.decoration
                    })
                    .map(|symbol| (inventory, symbol.id())),
            );
        }
        if named.is_empty() {
            return Err(NativeError::SymbolRejected {
                declaration: request.declaration,
                detail: format!("missing exact provider symbol {expected_name:?}"),
            });
        }
        if resolution.weak_symbol_policy() == WeakSymbolPolicy::Reject
            && named.iter().any(|(inventory, symbol_id)| {
                inventory.symbol(*symbol_id).is_some_and(|symbol| {
                    symbol.direction() == SymbolDirection::Exported
                        && symbol.binding() == SymbolBinding::Weak
                        && matches!(
                            symbol.visibility(),
                            SymbolVisibility::Default | SymbolVisibility::Protected
                        )
                })
            })
        {
            return Err(NativeError::SymbolRejected {
                declaration: request.declaration,
                detail: "weak provider participates in the exact-name candidate set".to_owned(),
            });
        }
        let mut acceptable = Vec::new();
        let mut rejection_reasons = Vec::new();
        for (inventory, symbol_id) in named {
            let symbol = inventory
                .symbol(symbol_id)
                .expect("symbol ID came from this inventory");
            if symbol.direction() != SymbolDirection::Exported {
                rejection_reasons.push("symbol is an import");
                continue;
            }
            match symbol.binding() {
                SymbolBinding::Global => {}
                SymbolBinding::Weak
                    if resolution.weak_symbol_policy() == WeakSymbolPolicy::AllowUnique => {}
                SymbolBinding::Weak => {
                    rejection_reasons.push("weak providers are rejected by analysis policy");
                    continue;
                }
                SymbolBinding::Local => {
                    rejection_reasons.push("local providers cannot satisfy public externs");
                    continue;
                }
            }
            if !matches!(
                symbol.visibility(),
                SymbolVisibility::Default | SymbolVisibility::Protected
            ) {
                rejection_reasons.push("hidden/internal providers cannot satisfy public externs");
                continue;
            }
            if symbol.kind() != expected_kind {
                rejection_reasons.push("provider has the wrong symbol kind");
                continue;
            }
            acceptable.push((inventory, symbol));
        }
        if acceptable.len() != 1 {
            return Err(NativeError::SymbolRejected {
                declaration: request.declaration,
                detail: if acceptable.is_empty() {
                    rejection_reasons.join("; ")
                } else {
                    format!("{} visible providers are ambiguous", acceptable.len())
                },
            });
        }
        let (inventory, symbol) = acceptable[0];

        match &declaration.kind {
            SourceDeclarationKind::Function(function) => {
                if !matches!(request.layout, LayoutAssessment::NotRequired) {
                    return Err(abi_error(
                        request.declaration,
                        "functions cannot carry declaration layout evidence",
                    ));
                }
                match &request.callable_abi {
                    CallableAbiAssessment::Confirmed {
                        calling_convention,
                        confidence,
                        ..
                    } if calling_convention == &function.calling_convention
                        && confidence.is_strictly_measured() => {}
                    _ => {
                        return Err(abi_error(
                            request.declaration,
                            "calling convention or callable confidence differs from the source declaration",
                        ));
                    }
                }
                let shape = request.abi_shape.ok_or_else(|| {
                    abi_error(
                        request.declaration,
                        "callable ABI shape evidence is required",
                    )
                })?;
                if shape.declaration() != request.declaration {
                    return Err(abi_error(
                        request.declaration,
                        "callable ABI shape is bound to another declaration",
                    ));
                }
                self.validate_callable_shape(source, probes, layouts, function, shape)?;
                match &request.callable_abi {
                    CallableAbiAssessment::Confirmed {
                        calling_convention,
                        confidence,
                        probe,
                    } if calling_convention == &function.calling_convention
                        && confidence.is_strictly_measured()
                        && *probe == shape.probe() => {}
                    _ => {
                        return Err(abi_error(
                            request.declaration,
                            "callable assessment does not match measured shape evidence",
                        ));
                    }
                }
            }
            SourceDeclarationKind::Variable(_) => {
                if request.abi_shape.is_some()
                    || !matches!(request.layout, LayoutAssessment::NotRequired)
                    || !matches!(request.callable_abi, CallableAbiAssessment::NotApplicable)
                {
                    return Err(abi_error(
                        request.declaration,
                        "variables require symbol evidence only",
                    ));
                }
            }
            _ => unreachable!("linked kind was checked above"),
        }

        Ok(DeclarationEvidence::try_new(DeclarationEvidenceInput {
            declaration: request.declaration,
            source_fingerprint: source.source().fingerprint(),
            target_fingerprint: source.source().target_fingerprint(),
            provider: ProviderAssessment::Resolved {
                provider: inventory.artifact().provider_id(),
                artifact_fingerprint: inventory.artifact().artifact_fingerprint(),
            },
            symbol: SymbolAssessment::Exact {
                symbol: SymbolReference::new(inventory.artifact().provider_id(), symbol.id()),
                expected_name: expected_name.to_owned(),
                actual_name: symbol.name().to_owned(),
                kind: expected_kind,
                decoration: request.decoration,
            },
            layout: request.layout,
            callable_abi: request.callable_abi,
        })?)
    }

    pub fn validate_callable_shape(
        &self,
        source: &CompleteSourcePackage,
        probes: &[AbiProbeEvidence],
        layouts: &[LayoutEvidence],
        function: &parc::contract::SourceFunction,
        shape: &AbiShapeEvidence,
    ) -> NativeResult<()> {
        if shape.source_fingerprint != source.source().fingerprint()
            || shape.target_fingerprint != source.source().target_fingerprint()
        {
            return Err(abi_error(
                shape.declaration,
                "callable evidence has stale source or target fingerprints",
            ));
        }
        if shape.calling_convention != function.calling_convention {
            return Err(abi_error(
                shape.declaration,
                "calling convention differs from the source declaration",
            ));
        }
        let variadic = match function.prototype {
            FunctionPrototype::Prototyped { variadic } => variadic,
            FunctionPrototype::UnspecifiedParameters => {
                return Err(abi_error(
                    shape.declaration,
                    "unprototyped callables are not accepted by the strict lane",
                ));
            }
        };
        if shape.variadic != variadic {
            return Err(abi_error(
                shape.declaration,
                "variadic ABI dimension differs from the source declaration",
            ));
        }
        if shape.parameters.len() != function.parameters.len() {
            return Err(abi_error(
                shape.declaration,
                "parameter ABI dimension count differs from the source declaration",
            ));
        }
        for (parameter, measured) in function.parameters.iter().zip(&shape.parameters) {
            validate_dimension(
                source,
                &parameter.ty,
                measured,
                layouts,
                true,
                shape.declaration,
            )?;
        }
        validate_dimension(
            source,
            &function.return_type,
            &shape.return_value,
            layouts,
            false,
            shape.declaration,
        )?;
        if source.source().target().triple() == "x86_64-unknown-linux-gnu"
            && matches!(
                function.calling_convention,
                CallingConvention::C | CallingConvention::Cdecl | CallingConvention::SysV64
            )
        {
            let classification = super::sysv::classify_sysv64_callable(source, function, layouts)?;
            if classification.parameters().len() != shape.parameters.len()
                || classification
                    .parameters()
                    .iter()
                    .zip(&shape.parameters)
                    .any(|(expected, measured)| *expected != measured.passing())
                || classification.return_value() != shape.return_value.passing()
                || classification.return_convention() != shape.return_convention
            {
                return Err(abi_error(
                    shape.declaration,
                    "callable passing or return convention differs from the SysV64 classifier",
                ));
            }
        }
        let return_category =
            return_type_category(source, &function.return_type, &mut BTreeSet::new())?;
        match (return_category, shape.return_convention) {
            (ReturnTypeCategory::Void, ReturnConvention::Void) => {}
            (ReturnTypeCategory::Void, _) => {
                return Err(abi_error(
                    shape.declaration,
                    "void return requires the void return convention",
                ));
            }
            (_, ReturnConvention::Void) => {
                return Err(abi_error(
                    shape.declaration,
                    "non-void return cannot use the void return convention",
                ));
            }
            (ReturnTypeCategory::Aggregate, _) => {}
            (ReturnTypeCategory::Scalar, ReturnConvention::IndirectSret) => {
                return Err(abi_error(
                    shape.declaration,
                    "scalar return cannot claim an aggregate sret convention",
                ));
            }
            _ => {}
        }
        let probe = probes
            .iter()
            .find(|probe| probe.id() == shape.probe)
            .ok_or_else(|| abi_error(shape.declaration, "callable probe evidence is missing"))?;
        let subject = ProbeSubject::CallableAbi {
            declaration: shape.declaration,
        };
        if !probe.supports(subject) || !probe.verified(subject) {
            return Err(abi_error(
                shape.declaration,
                "callable probe did not verify this exact declaration",
            ));
        }
        let outcome = probe
            .subject_outcomes()
            .iter()
            .find(|outcome| outcome.subject() == subject)
            .expect("supports() proved the subject exists");
        let expected = shape.fingerprint()?;
        match outcome.status() {
            crate::contract::ProbeSubjectStatus::Verified {
                evidence_fingerprint,
            } if *evidence_fingerprint == expected => Ok(()),
            _ => Err(abi_error(
                shape.declaration,
                "probe outcome fingerprint does not bind the typed callable evidence",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReturnTypeCategory {
    Void,
    Aggregate,
    Scalar,
}

fn return_type_category(
    source: &CompleteSourcePackage,
    ty: &CType,
    aliases: &mut BTreeSet<DeclarationId>,
) -> NativeResult<ReturnTypeCategory> {
    match &ty.kind {
        CTypeKind::Void => Ok(ReturnTypeCategory::Void),
        CTypeKind::RecordRef(_) | CTypeKind::Array { .. } => Ok(ReturnTypeCategory::Aggregate),
        CTypeKind::AliasRef(id) => {
            if !aliases.insert(*id) {
                return Err(NativeError::InvalidPolicy {
                    detail: "alias cycle reached during return-category validation".to_owned(),
                });
            }
            let declaration =
                source
                    .source()
                    .declaration(*id)
                    .ok_or_else(|| NativeError::InvalidPolicy {
                        detail: format!("alias declaration {id} is missing"),
                    })?;
            let SourceDeclarationKind::TypeAlias(alias) = &declaration.kind else {
                return Err(NativeError::InvalidPolicy {
                    detail: format!("declaration {id} is not an alias"),
                });
            };
            let result = return_type_category(source, &alias.target, aliases);
            aliases.remove(id);
            result
        }
        _ => Ok(ReturnTypeCategory::Scalar),
    }
}

fn validate_dimension(
    source: &CompleteSourcePackage,
    ty: &CType,
    measured: &AbiDimension,
    layouts: &[LayoutEvidence],
    parameter_position: bool,
    declaration: DeclarationId,
) -> NativeResult<()> {
    if measured.source_type != type_fingerprint(ty)? {
        return Err(abi_error(
            declaration,
            "ABI dimension is bound to a different source type",
        ));
    }
    let (size, alignment) = expected_layout(
        source,
        ty,
        layouts,
        parameter_position,
        &mut BTreeSet::new(),
    )?;
    if measured.size_bits != size || measured.alignment_bits != alignment {
        return Err(abi_error(
            declaration,
            "measured scalar/aggregate size or alignment differs from checked evidence",
        ));
    }
    Ok(())
}

fn expected_layout(
    source: &CompleteSourcePackage,
    ty: &CType,
    layouts: &[LayoutEvidence],
    parameter_position: bool,
    aliases: &mut BTreeSet<DeclarationId>,
) -> NativeResult<(u64, u32)> {
    let model = source.source().target().c_data_model();
    let scalar = |storage: u16, alignment: u16| (u64::from(storage), u32::from(alignment));
    let pointer = || {
        scalar(
            model.pointer_layout.storage_bits,
            model.pointer_layout.alignment_bits,
        )
    };
    let result = match &ty.kind {
        CTypeKind::Void => (0, 8),
        CTypeKind::Bool => scalar(
            model.bool_layout.storage_bits,
            model.bool_layout.alignment_bits,
        ),
        CTypeKind::Integer(integer) => integer_layout(model, integer)?,
        CTypeKind::Floating(floating) => floating_layout(model, floating)?,
        CTypeKind::Complex(floating) => {
            let (size, alignment) = floating_layout(model, floating)?;
            (
                size.checked_mul(2)
                    .ok_or_else(|| NativeError::InvalidPolicy {
                        detail: "complex ABI size overflow".to_owned(),
                    })?,
                alignment,
            )
        }
        CTypeKind::Pointer(_) => pointer(),
        CTypeKind::Function(_) if parameter_position => pointer(),
        CTypeKind::Function(_) => {
            return Err(NativeError::InvalidPolicy {
                detail: "function values have no return ABI layout".to_owned(),
            });
        }
        CTypeKind::Array { .. } if parameter_position => pointer(),
        CTypeKind::Array { element, bound, .. } => {
            let elements = match bound {
                ArrayBound::Fixed { elements } => *elements,
                _ => {
                    return Err(NativeError::InvalidPolicy {
                        detail: "non-fixed arrays have no strict by-value ABI layout".to_owned(),
                    });
                }
            };
            let (element_size, alignment) =
                expected_layout(source, element, layouts, false, aliases)?;
            (
                element_size
                    .checked_mul(elements)
                    .ok_or_else(|| NativeError::InvalidPolicy {
                        detail: "array ABI size overflow".to_owned(),
                    })?,
                alignment,
            )
        }
        CTypeKind::AliasRef(id) => {
            if !aliases.insert(*id) {
                return Err(NativeError::InvalidPolicy {
                    detail: "alias cycle reached during ABI validation".to_owned(),
                });
            }
            let declaration =
                source
                    .source()
                    .declaration(*id)
                    .ok_or_else(|| NativeError::InvalidPolicy {
                        detail: format!("alias declaration {id} is missing"),
                    })?;
            let SourceDeclarationKind::TypeAlias(alias) = &declaration.kind else {
                return Err(NativeError::InvalidPolicy {
                    detail: format!("declaration {id} is not an alias"),
                });
            };
            let result =
                expected_layout(source, &alias.target, layouts, parameter_position, aliases)?;
            aliases.remove(id);
            result
        }
        CTypeKind::RecordRef(id) => {
            let layout = layouts
                .iter()
                .find(|layout| layout.declaration() == *id)
                .ok_or_else(|| NativeError::InvalidPolicy {
                    detail: format!("record {id} has no measured layout"),
                })?;
            let LayoutEvidence::Record(layout) = layout else {
                return Err(NativeError::InvalidPolicy {
                    detail: format!("record {id} is bound to non-record layout evidence"),
                });
            };
            (layout.size_bits(), layout.alignment_bits())
        }
        CTypeKind::EnumRef(id) => {
            let layout = layouts
                .iter()
                .find(|layout| layout.declaration() == *id)
                .ok_or_else(|| NativeError::InvalidPolicy {
                    detail: format!("enum {id} has no measured representation"),
                })?;
            let LayoutEvidence::Enum(layout) = layout else {
                return Err(NativeError::InvalidPolicy {
                    detail: format!("enum {id} is bound to non-enum layout evidence"),
                });
            };
            (layout.storage_bits(), layout.alignment_bits())
        }
        CTypeKind::Unsupported { .. } => {
            return Err(NativeError::InvalidPolicy {
                detail: "unsupported source type has no strict ABI layout".to_owned(),
            });
        }
    };
    Ok(result)
}

fn integer_layout(model: &CDataModel, integer: &CIntegerType) -> NativeResult<(u64, u32)> {
    let layout = match integer {
        CIntegerType::Char { .. } => model.char_layout,
        CIntegerType::Short { .. } => model.short_layout,
        CIntegerType::Int { .. } => model.int_layout,
        CIntegerType::Long { .. } => model.long_layout,
        CIntegerType::LongLong { .. } => model.long_long_layout,
        CIntegerType::Int128 { .. } => {
            model
                .int128_layout
                .ok_or_else(|| NativeError::InvalidPolicy {
                    detail: "target has no certified __int128 layout".to_owned(),
                })?
        }
        CIntegerType::BitInt { .. } => {
            return Err(NativeError::InvalidPolicy {
                detail: "_BitInt requires explicit measured scalar layout support".to_owned(),
            });
        }
    };
    Ok((
        u64::from(layout.storage_bits),
        u32::from(layout.alignment_bits),
    ))
}

fn floating_layout(model: &CDataModel, floating: &CFloatingType) -> NativeResult<(u64, u32)> {
    let layout = match floating {
        CFloatingType::Float => model.float_layout.scalar,
        CFloatingType::Double => model.double_layout.scalar,
        CFloatingType::LongDouble => model.long_double_layout.scalar,
        CFloatingType::Float128 | CFloatingType::Ts18661 { .. } => {
            return Err(NativeError::InvalidPolicy {
                detail: "extended floating ABI requires explicit target layout support".to_owned(),
            });
        }
    };
    Ok((
        u64::from(layout.storage_bits),
        u32::from(layout.alignment_bits),
    ))
}

fn type_fingerprint(ty: &CType) -> NativeResult<ContentFingerprint> {
    serde_json::to_vec(ty)
        .map(|bytes| ContentFingerprint::from_content(&bytes))
        .map_err(|error| NativeError::InvalidPolicy {
            detail: format!("cannot canonicalize source type: {error}"),
        })
}

fn abi_error(declaration: DeclarationId, detail: &str) -> NativeError {
    NativeError::AbiMismatch {
        declaration,
        detail: detail.to_owned(),
    }
}
