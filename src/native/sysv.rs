//! Initial x86-64 System V callable-ABI classification.
//!
//! This classifier deliberately implements only the classes needed by the
//! certified C surface: INTEGER and SSE scalars, plus recursively classified
//! arrays, records, and unions occupying at most two eightbytes. Types which
//! require the x87, SSEUP, COMPLEX_X87, or vector rules are rejected rather
//! than guessed.

use std::collections::BTreeSet;

use parc::contract::{
    Architecture, ArrayBound, CDataModel, CFloatingType, CIntegerType, CType, CTypeKind,
    CallingConvention, CompleteSourcePackage, DeclarationId, Endian, Environment, FloatingFormat,
    FunctionPrototype, ObjectFormat, OperatingSystem, RecordCompleteness, RecordKind,
    SourceDeclarationKind, SourceFunction,
};

use crate::contract::{EvidenceSource, LayoutEvidence, RecordLayoutEvidence};

use super::{NativeError, NativeResult, ReturnConvention, ValuePassing};

const EIGHTBYTE_BITS: u64 = 64;
const MAX_DIRECT_BITS: u64 = 2 * EIGHTBYTE_BITS;

/// Fully derived passing information for one supported SysV64 callable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Sysv64CallableClassification {
    parameters: Vec<ValuePassing>,
    return_value: ValuePassing,
    return_convention: ReturnConvention,
}

impl Sysv64CallableClassification {
    pub(crate) fn parameters(&self) -> &[ValuePassing] {
        &self.parameters
    }

    pub(crate) const fn return_value(&self) -> ValuePassing {
        self.return_value
    }

    pub(crate) const fn return_convention(&self) -> ReturnConvention {
        self.return_convention
    }
}

/// Classifies a complete, non-variadic C function for the certified SysV64
/// lane. This is the preferred entry point for callable certification because
/// it cannot accidentally omit prototype or calling-convention checks.
pub(crate) fn classify_sysv64_callable(
    source: &CompleteSourcePackage,
    function: &SourceFunction,
    layouts: &[LayoutEvidence],
) -> NativeResult<Sysv64CallableClassification> {
    validate_callable(function)?;
    let parameters = function
        .parameters
        .iter()
        .map(|parameter| classify_sysv64_parameter(source, &parameter.ty, layouts))
        .collect::<NativeResult<Vec<_>>>()?;
    let (return_value, return_convention) =
        classify_sysv64_return(source, &function.return_type, layouts)?;
    Ok(Sysv64CallableClassification {
        parameters,
        return_value,
        return_convention,
    })
}

/// Derives how one C parameter is passed after the C array/function parameter
/// adjustments have been applied.
pub(crate) fn classify_sysv64_parameter(
    source: &CompleteSourcePackage,
    ty: &CType,
    layouts: &[LayoutEvidence],
) -> NativeResult<ValuePassing> {
    Classifier::new(source, layouts)?
        .classify(ty, Position::Parameter)
        .map(|classification| classification.passing)
}

/// Derives both the value-passing shape and return convention for one C return
/// type.
pub(crate) fn classify_sysv64_return(
    source: &CompleteSourcePackage,
    ty: &CType,
    layouts: &[LayoutEvidence],
) -> NativeResult<(ValuePassing, ReturnConvention)> {
    let classification = Classifier::new(source, layouts)?.classify(ty, Position::Return)?;
    Ok((classification.passing, return_convention(classification)))
}

/// Returns the post-adjustment layout of a C parameter. In particular, array
/// and function parameter declarators use the certified pointer layout.
pub(crate) fn sysv64_parameter_layout(
    source: &CompleteSourcePackage,
    ty: &CType,
    layouts: &[LayoutEvidence],
) -> NativeResult<(u64, u32)> {
    layout_for_position(source, ty, layouts, Position::Parameter)
}

/// Returns the layout of a C return value, including `(0, 8)` for `void` (or a
/// typedef resolving to `void`).
pub(crate) fn sysv64_return_layout(
    source: &CompleteSourcePackage,
    ty: &CType,
    layouts: &[LayoutEvidence],
) -> NativeResult<(u64, u32)> {
    layout_for_position(source, ty, layouts, Position::Return)
}

fn layout_for_position(
    source: &CompleteSourcePackage,
    ty: &CType,
    layouts: &[LayoutEvidence],
    position: Position,
) -> NativeResult<(u64, u32)> {
    Classifier::new(source, layouts)?
        .classify(ty, position)
        .map(|classification| (classification.size_bits, classification.alignment_bits))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Position {
    Parameter,
    Return,
    Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EightbyteClass {
    NoClass,
    Integer,
    Sse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TypeClassification {
    size_bits: u64,
    alignment_bits: u32,
    passing: ValuePassing,
}

#[derive(Debug, Clone, Copy)]
struct TypeLayout {
    size_bits: u64,
    alignment_bits: u32,
}

#[derive(Debug, Clone, Copy)]
struct Eightbytes {
    classes: [EightbyteClass; 2],
    memory: bool,
}

impl Eightbytes {
    const fn new() -> Self {
        Self {
            classes: [EightbyteClass::NoClass; 2],
            memory: false,
        }
    }

    fn merge_range(
        &mut self,
        offset_bits: u64,
        size_bits: u64,
        class: EightbyteClass,
    ) -> NativeResult<()> {
        if size_bits == 0 {
            return Err(invalid("a non-void ABI component has zero size"));
        }
        let end = offset_bits
            .checked_add(size_bits)
            .ok_or_else(|| invalid("ABI component offset overflow"))?;
        let first = offset_bits / EIGHTBYTE_BITS;
        let last = (end - 1) / EIGHTBYTE_BITS;
        if last >= self.classes.len() as u64 {
            self.memory = true;
            return Ok(());
        }
        for index in first..=last {
            let slot = &mut self.classes[index as usize];
            *slot = merge_class(*slot, class);
        }
        Ok(())
    }

    fn passing(self, size_bits: u64) -> NativeResult<ValuePassing> {
        if self.memory || size_bits > MAX_DIRECT_BITS {
            return Ok(ValuePassing::Indirect);
        }
        match self
            .classes
            .iter()
            .filter(|class| **class != EightbyteClass::NoClass)
            .count()
        {
            1 => Ok(ValuePassing::Direct),
            2 => Ok(ValuePassing::SplitRegisters),
            _ => Err(invalid(
                "a non-void SysV64 value must occupy at least one eightbyte class",
            )),
        }
    }
}

fn merge_class(left: EightbyteClass, right: EightbyteClass) -> EightbyteClass {
    use EightbyteClass::{Integer, NoClass, Sse};
    match (left, right) {
        (NoClass, class) | (class, NoClass) => class,
        (Integer, _) | (_, Integer) => Integer,
        (Sse, Sse) => Sse,
    }
}

struct Classifier<'a> {
    source: &'a CompleteSourcePackage,
    layouts: &'a [LayoutEvidence],
}

impl<'a> Classifier<'a> {
    fn new(source: &'a CompleteSourcePackage, layouts: &'a [LayoutEvidence]) -> NativeResult<Self> {
        validate_target(source)?;
        Ok(Self { source, layouts })
    }

    fn classify(&self, ty: &CType, position: Position) -> NativeResult<TypeClassification> {
        let mut eightbytes = Eightbytes::new();
        let mut recursion = Recursion::default();
        let layout = self.classify_at(ty, position, 0, &mut eightbytes, &mut recursion)?;
        let passing = if layout.size_bits == 0 {
            if position != Position::Return {
                return Err(invalid("void is supported only as a callable return type"));
            }
            ValuePassing::Ignore
        } else {
            eightbytes.passing(layout.size_bits)?
        };
        Ok(TypeClassification {
            size_bits: layout.size_bits,
            alignment_bits: layout.alignment_bits,
            passing,
        })
    }

    fn classify_at(
        &self,
        ty: &CType,
        position: Position,
        base_bits: u64,
        eightbytes: &mut Eightbytes,
        recursion: &mut Recursion,
    ) -> NativeResult<TypeLayout> {
        if !ty.support.is_supported() {
            return Err(invalid(
                "unsupported PARC type reached SysV64 classification",
            ));
        }
        if ty.qualifiers.is_atomic {
            return Err(invalid(
                "atomic-qualified values are outside the initial SysV64 surface",
            ));
        }

        let model = self.source.source().target().c_data_model();
        match &ty.kind {
            CTypeKind::Void => {
                if position != Position::Return {
                    return Err(invalid("void has no by-value SysV64 representation"));
                }
                Ok(TypeLayout {
                    size_bits: 0,
                    alignment_bits: 8,
                })
            }
            CTypeKind::Bool => {
                let layout = scalar_layout(
                    model.bool_layout.storage_bits,
                    model.bool_layout.alignment_bits,
                    "_Bool",
                )?;
                self.merge_scalar(base_bits, layout, EightbyteClass::Integer, eightbytes)?;
                Ok(layout)
            }
            CTypeKind::Integer(integer) => {
                let layout = integer_layout(model, integer)?;
                self.merge_scalar(base_bits, layout, EightbyteClass::Integer, eightbytes)?;
                Ok(layout)
            }
            CTypeKind::Floating(floating) => {
                let layout = floating_layout(model, floating)?;
                self.merge_scalar(base_bits, layout, EightbyteClass::Sse, eightbytes)?;
                Ok(layout)
            }
            CTypeKind::Complex(_) => Err(invalid(
                "complex values are outside the initial SysV64 surface",
            )),
            CTypeKind::Pointer(pointee) => {
                self.validate_pointer_pointee(pointee, recursion)?;
                let layout = pointer_layout(model)?;
                self.merge_scalar(base_bits, layout, EightbyteClass::Integer, eightbytes)?;
                Ok(layout)
            }
            CTypeKind::Function(function) if position == Position::Parameter => {
                self.validate_callback_function(function, recursion)?;
                let layout = pointer_layout(model)?;
                self.merge_scalar(base_bits, layout, EightbyteClass::Integer, eightbytes)?;
                Ok(layout)
            }
            CTypeKind::Function(_) => Err(invalid(
                "function values are valid only after C parameter adjustment",
            )),
            CTypeKind::Array { .. } if position == Position::Parameter => {
                let layout = pointer_layout(model)?;
                self.merge_scalar(base_bits, layout, EightbyteClass::Integer, eightbytes)?;
                Ok(layout)
            }
            CTypeKind::Array { .. } if position == Position::Return => Err(invalid(
                "C array return values are outside the certified SysV64 surface",
            )),
            CTypeKind::Array { element, bound, .. } => {
                self.classify_array(element, bound, base_bits, eightbytes, recursion)
            }
            CTypeKind::AliasRef(declaration) => {
                if !recursion.aliases.insert(*declaration) {
                    return Err(invalid("alias cycle reached SysV64 classification"));
                }
                let result =
                    self.classify_alias(*declaration, position, base_bits, eightbytes, recursion);
                recursion.aliases.remove(declaration);
                result
            }
            CTypeKind::RecordRef(declaration) => {
                self.classify_record(*declaration, base_bits, eightbytes, recursion)
            }
            CTypeKind::EnumRef(declaration) => {
                let layout = self.enum_layout(*declaration)?;
                if layout.size_bits > EIGHTBYTE_BITS {
                    return Err(invalid(
                        "enum representations wider than 64 bits are not certified",
                    ));
                }
                self.merge_scalar(base_bits, layout, EightbyteClass::Integer, eightbytes)?;
                Ok(layout)
            }
            CTypeKind::Unsupported { .. } => Err(invalid(
                "unsupported source types have no certified SysV64 classification",
            )),
        }
    }

    fn merge_scalar(
        &self,
        base_bits: u64,
        layout: TypeLayout,
        class: EightbyteClass,
        eightbytes: &mut Eightbytes,
    ) -> NativeResult<()> {
        if !base_bits.is_multiple_of(u64::from(layout.alignment_bits)) {
            return Err(invalid(
                "unaligned scalar or aggregate member requires SysV64 MEMORY classification and is rejected by the initial surface",
            ));
        }
        eightbytes.merge_range(base_bits, layout.size_bits, class)
    }

    fn classify_array(
        &self,
        element: &CType,
        bound: &ArrayBound,
        base_bits: u64,
        eightbytes: &mut Eightbytes,
        recursion: &mut Recursion,
    ) -> NativeResult<TypeLayout> {
        let elements = match bound {
            ArrayBound::Fixed { elements } if *elements > 0 => *elements,
            ArrayBound::Fixed { .. } => {
                return Err(invalid(
                    "zero-length arrays are outside the initial SysV64 surface",
                ));
            }
            _ => {
                return Err(invalid(
                    "only fixed-size arrays have a by-value SysV64 classification",
                ));
            }
        };

        let element_layout =
            self.classify_at(element, Position::Value, base_bits, eightbytes, recursion)?;
        if element_layout.size_bits == 0
            || !element_layout
                .size_bits
                .is_multiple_of(u64::from(element_layout.alignment_bits))
        {
            return Err(invalid(
                "array element size is not a nonzero multiple of its alignment",
            ));
        }
        let size_bits = element_layout
            .size_bits
            .checked_mul(elements)
            .ok_or_else(|| invalid("array ABI size overflow"))?;
        if size_bits > MAX_DIRECT_BITS {
            // The result is MEMORY regardless of its leaf classes. One element
            // has still been traversed above, so unsupported element types and
            // stale nested layout evidence cannot hide behind the size rule.
            eightbytes.memory = true;
        } else {
            for index in 1..elements {
                let offset = element_layout
                    .size_bits
                    .checked_mul(index)
                    .and_then(|offset| base_bits.checked_add(offset))
                    .ok_or_else(|| invalid("array element offset overflow"))?;
                let repeated =
                    self.classify_at(element, Position::Value, offset, eightbytes, recursion)?;
                if repeated.size_bits != element_layout.size_bits
                    || repeated.alignment_bits != element_layout.alignment_bits
                {
                    return Err(invalid("array element classification is not stable"));
                }
            }
        }
        Ok(TypeLayout {
            size_bits,
            alignment_bits: element_layout.alignment_bits,
        })
    }

    fn classify_alias(
        &self,
        declaration: DeclarationId,
        position: Position,
        base_bits: u64,
        eightbytes: &mut Eightbytes,
        recursion: &mut Recursion,
    ) -> NativeResult<TypeLayout> {
        let source_declaration = self
            .source
            .source()
            .declaration(declaration)
            .ok_or_else(|| invalid(format!("alias declaration {declaration} is missing")))?;
        let SourceDeclarationKind::TypeAlias(alias) = &source_declaration.kind else {
            return Err(invalid(format!(
                "declaration {declaration} is not a type alias"
            )));
        };
        self.classify_at(&alias.target, position, base_bits, eightbytes, recursion)
    }

    fn validate_pointer_pointee(&self, ty: &CType, recursion: &mut Recursion) -> NativeResult<()> {
        match &ty.kind {
            CTypeKind::Function(function) => self.validate_callback_function(function, recursion),
            CTypeKind::Pointer(pointee) => self.validate_pointer_pointee(pointee, recursion),
            CTypeKind::AliasRef(declaration) => {
                if !recursion.aliases.insert(*declaration) {
                    return Err(invalid("alias cycle reached callback-profile validation"));
                }
                let source_declaration = self
                    .source
                    .source()
                    .declaration(*declaration)
                    .ok_or_else(|| {
                        invalid(format!("alias declaration {declaration} is missing"))
                    })?;
                let SourceDeclarationKind::TypeAlias(alias) = &source_declaration.kind else {
                    recursion.aliases.remove(declaration);
                    return Err(invalid(format!(
                        "declaration {declaration} is not a type alias"
                    )));
                };
                let result = self.validate_pointer_pointee(&alias.target, recursion);
                recursion.aliases.remove(declaration);
                result
            }
            _ => Ok(()),
        }
    }

    fn validate_callback_function(
        &self,
        function: &parc::contract::CFunctionType,
        recursion: &mut Recursion,
    ) -> NativeResult<()> {
        match function.prototype {
            FunctionPrototype::Prototyped { variadic: false } => {}
            FunctionPrototype::Prototyped { variadic: true } => {
                return Err(invalid(
                    "variadic callbacks are outside the initial SysV64 surface",
                ));
            }
            FunctionPrototype::UnspecifiedParameters => {
                return Err(invalid(
                    "unprototyped callbacks are outside the initial SysV64 surface",
                ));
            }
        }
        validate_calling_convention(&function.calling_convention)?;
        self.validate_callback_component(&function.return_type, recursion)?;
        for parameter in &function.parameters {
            self.validate_callback_component(&parameter.ty, recursion)?;
        }
        Ok(())
    }

    fn validate_callback_component(
        &self,
        ty: &CType,
        recursion: &mut Recursion,
    ) -> NativeResult<()> {
        match &ty.kind {
            CTypeKind::Complex(_)
            | CTypeKind::Floating(
                CFloatingType::LongDouble | CFloatingType::Float128 | CFloatingType::Ts18661 { .. },
            )
            | CTypeKind::Integer(CIntegerType::Int128 { .. } | CIntegerType::BitInt { .. })
            | CTypeKind::Unsupported { .. } => Err(invalid(
                "callback component is outside the initial SysV64 scalar profile",
            )),
            CTypeKind::Function(function) => self.validate_callback_function(function, recursion),
            CTypeKind::Pointer(pointee) => self.validate_pointer_pointee(pointee, recursion),
            CTypeKind::Array { element, .. } => {
                self.validate_callback_component(element, recursion)
            }
            CTypeKind::AliasRef(declaration) => {
                if !recursion.aliases.insert(*declaration) {
                    return Err(invalid("alias cycle reached callback-profile validation"));
                }
                let source_declaration = self
                    .source
                    .source()
                    .declaration(*declaration)
                    .ok_or_else(|| {
                        invalid(format!("alias declaration {declaration} is missing"))
                    })?;
                let SourceDeclarationKind::TypeAlias(alias) = &source_declaration.kind else {
                    recursion.aliases.remove(declaration);
                    return Err(invalid(format!(
                        "declaration {declaration} is not a type alias"
                    )));
                };
                let result = self.validate_callback_component(&alias.target, recursion);
                recursion.aliases.remove(declaration);
                result
            }
            CTypeKind::Void
            | CTypeKind::Bool
            | CTypeKind::Integer(_)
            | CTypeKind::Floating(_)
            | CTypeKind::RecordRef(_)
            | CTypeKind::EnumRef(_) => Ok(()),
        }
    }

    fn classify_record(
        &self,
        declaration: DeclarationId,
        base_bits: u64,
        eightbytes: &mut Eightbytes,
        recursion: &mut Recursion,
    ) -> NativeResult<TypeLayout> {
        if !recursion.records.insert(declaration) {
            return Err(invalid(
                "by-value record cycle reached SysV64 classification",
            ));
        }
        let result = self.classify_record_inner(declaration, base_bits, eightbytes, recursion);
        recursion.records.remove(&declaration);
        result
    }

    fn classify_record_inner(
        &self,
        declaration: DeclarationId,
        base_bits: u64,
        eightbytes: &mut Eightbytes,
        recursion: &mut Recursion,
    ) -> NativeResult<TypeLayout> {
        let source_declaration = self
            .source
            .source()
            .declaration(declaration)
            .ok_or_else(|| invalid(format!("record declaration {declaration} is missing")))?;
        let SourceDeclarationKind::Record(record) = &source_declaration.kind else {
            return Err(invalid(format!(
                "declaration {declaration} is not a record"
            )));
        };
        if record.completeness != RecordCompleteness::Complete {
            return Err(invalid("incomplete records cannot be classified by value"));
        }
        if record.fields.is_empty() {
            return Err(invalid(
                "empty records are outside the initial SysV64 surface",
            ));
        }
        let measured = self.record_layout(declaration)?;
        validate_aggregate_layout(measured.size_bits(), measured.alignment_bits(), "record")?;
        if !base_bits.is_multiple_of(u64::from(measured.alignment_bits())) {
            return Err(invalid(
                "unaligned record members are rejected by the initial SysV64 surface",
            ));
        }
        if measured.fields().len() != record.fields.len() {
            return Err(invalid(format!(
                "record {declaration} field-layout cardinality differs from source"
            )));
        }
        if measured.size_bits() > MAX_DIRECT_BITS {
            eightbytes.memory = true;
        }

        let mut previous_end = 0_u64;
        let mut maximum_field_alignment = 8_u32;
        for field in &record.fields {
            if field.bit_width.is_some() {
                return Err(invalid(
                    "bitfield layout is outside the initial SysV64 surface",
                ));
            }
            let mut matching = measured
                .fields()
                .iter()
                .filter(|candidate| candidate.child() == field.id);
            let field_evidence = matching.next().ok_or_else(|| {
                invalid(format!(
                    "record {declaration} field {} has no measured layout",
                    field.id
                ))
            })?;
            if matching.next().is_some() {
                return Err(invalid(format!(
                    "record {declaration} field {} has duplicate measured layouts",
                    field.id
                )));
            }
            let offset_bits = field_evidence.offset_bits();
            if !offset_bits.is_multiple_of(8) {
                return Err(invalid(
                    "non-bitfield record members must start on a byte boundary",
                ));
            }
            let absolute_offset = base_bits
                .checked_add(offset_bits)
                .ok_or_else(|| invalid("record field offset overflow"))?;
            let field_layout = self.classify_at(
                &field.ty,
                Position::Value,
                absolute_offset,
                eightbytes,
                recursion,
            )?;
            let measured_size = field_evidence
                .size_bits()
                .ok_or_else(|| invalid("SysV64 certification requires measured field sizes"))?;
            let measured_alignment = field_evidence.alignment_bits().ok_or_else(|| {
                invalid("SysV64 certification requires measured field alignments")
            })?;
            if measured_size != field_layout.size_bits
                || measured_alignment != field_layout.alignment_bits
            {
                return Err(invalid(format!(
                    "record {declaration} field {} layout differs from its canonical type",
                    field.id
                )));
            }
            if !offset_bits.is_multiple_of(u64::from(field_layout.alignment_bits)) {
                return Err(invalid(
                    "unaligned record fields require MEMORY classification and are rejected by the initial surface",
                ));
            }
            maximum_field_alignment = maximum_field_alignment.max(field_layout.alignment_bits);
            let field_end = offset_bits
                .checked_add(field_layout.size_bits)
                .ok_or_else(|| invalid("record field extent overflow"))?;
            if field_end > measured.size_bits() {
                return Err(invalid(format!(
                    "record {declaration} field {} extends beyond measured size",
                    field.id
                )));
            }
            match record.kind {
                RecordKind::Struct => {
                    if offset_bits < previous_end {
                        return Err(invalid(
                            "non-bitfield struct members overlap in measured layout",
                        ));
                    }
                    previous_end = field_end;
                }
                RecordKind::Union if offset_bits != 0 => {
                    return Err(invalid("union members must have a zero measured offset"));
                }
                RecordKind::Union => {}
            }
        }
        if measured.alignment_bits() < maximum_field_alignment {
            return Err(invalid("record alignment under-aligns one or more fields"));
        }
        Ok(TypeLayout {
            size_bits: measured.size_bits(),
            alignment_bits: measured.alignment_bits(),
        })
    }

    fn record_layout(&self, declaration: DeclarationId) -> NativeResult<&'a RecordLayoutEvidence> {
        let evidence = self.unique_layout(declaration)?;
        let LayoutEvidence::Record(record) = evidence else {
            return Err(invalid(format!(
                "record {declaration} is bound to non-record layout evidence"
            )));
        };
        Ok(record)
    }

    fn enum_layout(&self, declaration: DeclarationId) -> NativeResult<TypeLayout> {
        let source_declaration = self
            .source
            .source()
            .declaration(declaration)
            .ok_or_else(|| invalid(format!("enum declaration {declaration} is missing")))?;
        if !matches!(source_declaration.kind, SourceDeclarationKind::Enum(_)) {
            return Err(invalid(format!("declaration {declaration} is not an enum")));
        }
        let evidence = self.unique_layout(declaration)?;
        let LayoutEvidence::Enum(enumeration) = evidence else {
            return Err(invalid(format!(
                "enum {declaration} is bound to non-enum layout evidence"
            )));
        };
        validate_aggregate_layout(
            enumeration.storage_bits(),
            enumeration.alignment_bits(),
            "enum",
        )?;
        Ok(TypeLayout {
            size_bits: enumeration.storage_bits(),
            alignment_bits: enumeration.alignment_bits(),
        })
    }

    fn unique_layout(&self, declaration: DeclarationId) -> NativeResult<&'a LayoutEvidence> {
        let mut matching = self
            .layouts
            .iter()
            .filter(|layout| layout.declaration() == declaration);
        let evidence = matching.next().ok_or_else(|| {
            invalid(format!(
                "declaration {declaration} has no measured layout evidence"
            ))
        })?;
        if matching.next().is_some() {
            return Err(invalid(format!(
                "declaration {declaration} has duplicate measured layouts"
            )));
        }
        if evidence.source_fingerprint() != self.source.source().fingerprint()
            || evidence.target_fingerprint() != self.source.source().target_fingerprint()
        {
            return Err(invalid(format!(
                "declaration {declaration} layout has stale source or target fingerprints"
            )));
        }
        if !evidence.confidence().is_strictly_measured() {
            return Err(invalid(format!(
                "declaration {declaration} layout is not strictly measured"
            )));
        }
        if !matches!(
            evidence.source(),
            EvidenceSource::CompilerProbe | EvidenceSource::Corroborated
        ) {
            return Err(invalid(format!(
                "declaration {declaration} layout is not backed by a compiler probe"
            )));
        }
        Ok(evidence)
    }
}

#[derive(Default)]
struct Recursion {
    aliases: BTreeSet<DeclarationId>,
    records: BTreeSet<DeclarationId>,
}

fn validate_target(source: &CompleteSourcePackage) -> NativeResult<()> {
    let target = source.source().target();
    if target.triple() != "x86_64-unknown-linux-gnu"
        || target.architecture() != Architecture::X86_64
        || target.operating_system() != OperatingSystem::Linux
        || target.environment() != Environment::Gnu
        || target.object_format() != ObjectFormat::Elf
        || target.endian() != Endian::Little
        || target.pointer_width() != 64
    {
        return Err(invalid(
            "the initial SysV64 classifier accepts only x86_64-unknown-linux-gnu ELF",
        ));
    }
    let model = target.c_data_model();
    if model.char_bit != 8
        || model.pointer_layout.storage_bits != 64
        || model.pointer_layout.alignment_bits != 64
    {
        return Err(invalid(
            "the initial SysV64 classifier requires an eight-bit-byte LP64 pointer layout",
        ));
    }
    Ok(())
}

fn validate_callable(function: &SourceFunction) -> NativeResult<()> {
    match function.prototype {
        FunctionPrototype::Prototyped { variadic: false } => {}
        FunctionPrototype::Prototyped { variadic: true } => {
            return Err(invalid(
                "variadic callables are outside the initial SysV64 surface",
            ));
        }
        FunctionPrototype::UnspecifiedParameters => {
            return Err(invalid(
                "unprototyped callables are outside the initial SysV64 surface",
            ));
        }
    }
    validate_calling_convention(&function.calling_convention)
}

fn validate_calling_convention(convention: &CallingConvention) -> NativeResult<()> {
    if matches!(
        convention,
        CallingConvention::C | CallingConvention::Cdecl | CallingConvention::SysV64
    ) {
        Ok(())
    } else {
        Err(invalid(
            "callable convention is not certified for x86-64 System V",
        ))
    }
}

fn return_convention(classification: TypeClassification) -> ReturnConvention {
    match classification.passing {
        ValuePassing::Ignore => ReturnConvention::Void,
        ValuePassing::Direct => ReturnConvention::Direct,
        ValuePassing::SplitRegisters => ReturnConvention::RegisterPair,
        ValuePassing::Indirect => ReturnConvention::IndirectSret,
    }
}

fn integer_layout(model: &CDataModel, integer: &CIntegerType) -> NativeResult<TypeLayout> {
    let layout = match integer {
        CIntegerType::Char { .. } => model.char_layout,
        CIntegerType::Short { .. } => model.short_layout,
        CIntegerType::Int { .. } => model.int_layout,
        CIntegerType::Long { .. } => model.long_layout,
        CIntegerType::LongLong { .. } => model.long_long_layout,
        CIntegerType::Int128 { .. } => {
            return Err(invalid(
                "128-bit integers are outside the initial SysV64 surface",
            ));
        }
        CIntegerType::BitInt { .. } => {
            return Err(invalid("_BitInt is outside the initial SysV64 surface"));
        }
    };
    let layout = scalar_layout(layout.storage_bits, layout.alignment_bits, "integer")?;
    if layout.size_bits > EIGHTBYTE_BITS {
        return Err(invalid(
            "integer representations wider than 64 bits are not certified",
        ));
    }
    Ok(layout)
}

fn floating_layout(model: &CDataModel, floating: &CFloatingType) -> NativeResult<TypeLayout> {
    let (layout, expected_size, expected_format, name) = match floating {
        CFloatingType::Float => (
            model.float_layout.scalar,
            32,
            FloatingFormat::IeeeBinary32,
            "float",
        ),
        CFloatingType::Double => (
            model.double_layout.scalar,
            64,
            FloatingFormat::IeeeBinary64,
            "double",
        ),
        CFloatingType::LongDouble => {
            return Err(invalid("long double is outside the initial SysV64 surface"));
        }
        CFloatingType::Float128 | CFloatingType::Ts18661 { .. } => {
            return Err(invalid(
                "extended floating types are outside the initial SysV64 surface",
            ));
        }
    };
    let actual_format = match floating {
        CFloatingType::Float => &model.float_layout.format,
        CFloatingType::Double => &model.double_layout.format,
        _ => unreachable!("extended floating types returned above"),
    };
    if layout.storage_bits != expected_size || actual_format != &expected_format {
        return Err(invalid(format!(
            "{name} does not use its certified IEEE binary representation"
        )));
    }
    scalar_layout(layout.storage_bits, layout.alignment_bits, name)
}

fn pointer_layout(model: &CDataModel) -> NativeResult<TypeLayout> {
    let layout = scalar_layout(
        model.pointer_layout.storage_bits,
        model.pointer_layout.alignment_bits,
        "pointer",
    )?;
    if layout.size_bits != 64 || layout.alignment_bits != 64 {
        return Err(invalid(
            "the certified SysV64 surface requires 64-bit pointers",
        ));
    }
    Ok(layout)
}

fn scalar_layout(storage_bits: u16, alignment_bits: u16, name: &str) -> NativeResult<TypeLayout> {
    if storage_bits == 0
        || !storage_bits.is_multiple_of(8)
        || alignment_bits < 8
        || !alignment_bits.is_multiple_of(8)
        || !alignment_bits.is_power_of_two()
    {
        return Err(invalid(format!(
            "{name} has a non-canonical byte-sized ABI layout"
        )));
    }
    Ok(TypeLayout {
        size_bits: u64::from(storage_bits),
        alignment_bits: u32::from(alignment_bits),
    })
}

fn validate_aggregate_layout(size_bits: u64, alignment_bits: u32, name: &str) -> NativeResult<()> {
    if size_bits == 0
        || !size_bits.is_multiple_of(8)
        || alignment_bits < 8
        || !alignment_bits.is_multiple_of(8)
        || !alignment_bits.is_power_of_two()
        || alignment_bits > EIGHTBYTE_BITS as u32
        || !size_bits.is_multiple_of(u64::from(alignment_bits))
    {
        return Err(invalid(format!(
            "{name} has a non-canonical measured layout"
        )));
    }
    Ok(())
}

fn invalid(detail: impl Into<String>) -> NativeError {
    NativeError::InvalidPolicy {
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{merge_class, EightbyteClass, Eightbytes};
    use crate::native::ValuePassing;

    #[test]
    fn integer_dominates_sse_in_one_eightbyte() {
        assert_eq!(
            merge_class(EightbyteClass::Sse, EightbyteClass::Integer),
            EightbyteClass::Integer
        );
    }

    #[test]
    fn two_populated_eightbytes_split_registers() {
        let mut classes = Eightbytes::new();
        classes.merge_range(0, 64, EightbyteClass::Integer).unwrap();
        classes.merge_range(64, 64, EightbyteClass::Sse).unwrap();
        assert_eq!(classes.passing(128).unwrap(), ValuePassing::SplitRegisters);
    }

    #[test]
    fn values_larger_than_two_eightbytes_are_indirect() {
        let mut classes = Eightbytes::new();
        classes
            .merge_range(0, 192, EightbyteClass::Integer)
            .unwrap();
        assert_eq!(classes.passing(192).unwrap(), ValuePassing::Indirect);
    }
}
