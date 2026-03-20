use serde::{Deserialize, Serialize};

use crate::ir::{BindingItem, BindingPackage};
use crate::symbols::{
    FunctionAbiHint, SymbolBinding, SymbolDirection, SymbolInventory, SymbolVisibility,
};

/// Declaration category that validation currently reasons about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemKind {
    Function,
    Variable,
}

/// Validation outcome for one declaration/provider comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchStatus {
    Matched,
    AbiShapeMismatch,
    Missing,
    UnresolvedDeclaredLinkInputs,
    DecorationMismatch,
    NotAFunction,
    NotAVariable,
    Hidden,
    WeakMatch,
    DuplicateProviders,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationPhase {
    ProviderDiscovery,
    SymbolIdentity,
    AbiEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationPhaseReport {
    pub phase: ValidationPhase,
    pub completed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationDeclaration {
    pub name: String,
    pub item_kind: ItemKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationEvidence {
    #[serde(default)]
    pub provider_artifacts: Vec<String>,
    #[serde(default)]
    pub raw_symbol_names: Vec<String>,
    pub visibility: Option<SymbolVisibility>,
    #[serde(default = "default_match_confidence")]
    pub confidence: MatchConfidence,
    #[serde(default = "default_evidence_kind")]
    pub evidence_kind: EvidenceKind,
    #[serde(default)]
    pub abi_shape: Option<AbiShapeEvidence>,
    #[serde(default)]
    pub routine_abi: Option<RoutineAbiEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationEntry {
    pub declaration: ValidationDeclaration,
    pub status: MatchStatus,
    pub evidence: ValidationEvidence,
}

impl ValidationEvidence {
    pub fn has_layout_backed_confidence(&self) -> bool {
        self.abi_shape.is_some()
            || self.routine_abi.as_ref().is_some_and(|routine| {
                routine.expected_return_size.is_some()
                    || routine.observed_return_size.is_some()
                    || !routine.expected_parameter_sizes.is_empty()
                    || !routine.observed_parameter_sizes.is_empty()
            })
    }
}

impl ValidationEntry {
    pub fn has_layout_backed_confidence(&self) -> bool {
        self.evidence.has_layout_backed_confidence()
    }

    pub fn has_resolved_provider_state(&self) -> bool {
        matches!(
            self.status,
            MatchStatus::Matched | MatchStatus::AbiShapeMismatch | MatchStatus::WeakMatch
        )
    }

    pub fn has_unresolved_provider_state(&self) -> bool {
        matches!(
            self.status,
            MatchStatus::Missing | MatchStatus::UnresolvedDeclaredLinkInputs
        )
    }

    pub fn has_ambiguous_provider_state(&self) -> bool {
        self.status == MatchStatus::DuplicateProviders
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ValidationSummary {
    pub total: usize,
    pub matched: usize,
    #[serde(default)]
    pub abi_shape_mismatches: usize,
    pub missing: usize,
    pub unresolved_declared_link_inputs: usize,
    pub hidden: usize,
    pub weak_matches: usize,
    pub duplicate_providers: usize,
    pub decoration_mismatches: usize,
    pub kind_mismatches: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchConfidence {
    High,
    Medium,
    Low,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceKind {
    ExactExported,
    AbiShapeVerified,
    WeakExported,
    HiddenProvider,
    DecoratedCandidate,
    ReexportedCandidate,
    DuplicateVisibleProviders,
    DeclaredLinkInputsWithoutProvider,
    MissingProvider,
    AbiShapeMismatch,
    KindMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbiShapeEvidence {
    #[serde(default)]
    pub expected_size: Option<u64>,
    #[serde(default)]
    pub observed_size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutineAbiEvidence {
    #[serde(default)]
    pub evidence_kind: Option<RoutineAbiEvidenceKind>,
    #[serde(default)]
    pub confidence: Option<RoutineAbiConfidence>,
    #[serde(default)]
    pub expected_parameter_count: Option<usize>,
    #[serde(default)]
    pub observed_parameter_count: Option<usize>,
    #[serde(default)]
    pub expected_return_size: Option<u64>,
    #[serde(default)]
    pub observed_return_size: Option<u64>,
    #[serde(default)]
    pub expected_parameter_sizes: Vec<Option<u64>>,
    #[serde(default)]
    pub observed_parameter_sizes: Vec<Option<u64>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoutineAbiEvidenceKind {
    ParameterCountOnly,
    ReturnShapeOnly,
    ParameterShapesOnly,
    ParameterCountAndReturnShape,
    ParameterCountAndParameterShapes,
    ReturnShapeAndParameterShapes,
    FullyShaped,
    Mismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoutineAbiConfidence {
    Partial,
    Strong,
    Mismatch,
}

/// Renamed from FunctionMatch to support both functions and variables.
pub type FunctionMatch = SymbolMatch;

/// Validation evidence for one declaration name.
///
/// Invariant: this is report data, not an error channel, so non-matched states still represent a
/// successful validation run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolMatch {
    pub name: String,
    pub item_kind: ItemKind,
    pub status: MatchStatus,
    pub visibility: Option<SymbolVisibility>,
    #[serde(default)]
    pub provider_artifacts: Vec<String>,
    #[serde(default = "default_match_confidence")]
    pub confidence: MatchConfidence,
    #[serde(default = "default_evidence_kind")]
    pub evidence_kind: EvidenceKind,
}

/// Aggregate validation report for a package against one or more inventories.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReport {
    #[serde(default = "default_validation_phases")]
    pub phases: Vec<ValidationPhaseReport>,
    #[serde(default)]
    pub entries: Vec<ValidationEntry>,
    #[serde(default)]
    pub summary: ValidationSummary,
    pub matches: Vec<SymbolMatch>,
}

impl ValidationReport {
    pub fn matched(&self) -> Vec<&SymbolMatch> {
        self.matches
            .iter()
            .filter(|m| m.status == MatchStatus::Matched)
            .collect()
    }

    pub fn missing(&self) -> Vec<&SymbolMatch> {
        self.matches
            .iter()
            .filter(|m| m.status == MatchStatus::Missing)
            .collect()
    }

    pub fn hidden(&self) -> Vec<&SymbolMatch> {
        self.matches
            .iter()
            .filter(|m| m.status == MatchStatus::Hidden)
            .collect()
    }

    pub fn all_matched(&self) -> bool {
        self.matches
            .iter()
            .all(|m| m.status == MatchStatus::Matched)
    }

    pub fn duplicate_providers(&self) -> Vec<&SymbolMatch> {
        self.matches
            .iter()
            .filter(|m| m.status == MatchStatus::DuplicateProviders)
            .collect()
    }

    pub fn unresolved_declared(&self) -> Vec<&SymbolMatch> {
        self.matches
            .iter()
            .filter(|m| m.status == MatchStatus::UnresolvedDeclaredLinkInputs)
            .collect()
    }

    pub fn weak_matches(&self) -> Vec<&SymbolMatch> {
        self.matches
            .iter()
            .filter(|m| m.status == MatchStatus::WeakMatch)
            .collect()
    }

    pub fn layout_backed_entries(&self) -> Vec<&ValidationEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.has_layout_backed_confidence())
            .collect()
    }

    pub fn resolved_provider_entries(&self) -> Vec<&ValidationEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.has_resolved_provider_state())
            .collect()
    }

    pub fn unresolved_provider_entries(&self) -> Vec<&ValidationEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.has_unresolved_provider_state())
            .collect()
    }

    pub fn ambiguous_provider_entries(&self) -> Vec<&ValidationEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.has_ambiguous_provider_state())
            .collect()
    }
}

pub fn validate(package: &BindingPackage, inventory: &SymbolInventory) -> ValidationReport {
    validate_many(package, std::slice::from_ref(inventory))
}

pub fn validate_many(
    package: &BindingPackage,
    inventories: &[SymbolInventory],
) -> ValidationReport {
    let mut matches = Vec::new();
    let mut entries = Vec::new();
    let mut any_abi_shape_evidence = false;

    for item in &package.items {
        let (name, kind, expect_function, variable_ty, function_hint) = match item {
            BindingItem::Function(f) => (
                &f.name,
                ItemKind::Function,
                true,
                None,
                Some(FunctionAbiHint {
                    parameter_count: Some(f.parameters.len()),
                    return_size: expected_abi_size(package, &f.return_type),
                    parameter_sizes: f
                        .parameters
                        .iter()
                        .map(|parameter| expected_abi_size(package, &parameter.ty))
                        .collect(),
                }),
            ),
            BindingItem::Variable(v) => (
                &v.name,
                ItemKind::Variable,
                false,
                Some(&v.ty),
                None,
            ),
            _ => continue,
        };

        let candidates: Vec<_> = inventories
            .iter()
            .flat_map(|inventory| {
                inventory
                    .symbols
                    .iter()
                    .filter(move |symbol| {
                        symbol.name == *name && symbol.direction == SymbolDirection::Exported
                    })
                    .map(move |symbol| (inventory, symbol))
            })
            .collect();
        let provider_artifacts = candidates
            .iter()
            .map(|(inventory, symbol)| format_provider(inventory, symbol))
            .collect::<Vec<_>>();
        let decorated_candidates: Vec<_> = inventories
            .iter()
            .flat_map(|inventory| {
                inventory
                    .symbols
                    .iter()
                    .filter(move |symbol| {
                        symbol
                            .raw_name
                            .as_deref()
                            .map(normalize_decorated_name)
                            .as_deref()
                            == Some(name.as_str())
                            && symbol.name != *name
                    })
                    .map(move |symbol| (inventory, symbol))
            })
            .collect();
        let imported_candidates: Vec<_> = inventories
            .iter()
            .flat_map(|inventory| {
                inventory
                    .symbols
                    .iter()
                    .filter(move |symbol| {
                        symbol.name == *name && symbol.direction == SymbolDirection::Imported
                    })
                    .map(move |symbol| (inventory, symbol))
            })
            .collect();
        let raw_symbol_names = candidates
            .iter()
            .chain(imported_candidates.iter())
            .chain(decorated_candidates.iter())
            .filter_map(|(_, symbol)| symbol.raw_name.clone())
            .collect::<Vec<_>>();
        let (mut status, visibility) = match candidates.first() {
            Some(_) => {
                let visible: Vec<_> = candidates
                    .iter()
                    .copied()
                    .filter(|(_, symbol)| {
                        !matches!(
                            symbol.visibility,
                            SymbolVisibility::Hidden | SymbolVisibility::Internal
                        )
                    })
                    .collect();
                if visible.is_empty() {
                    (MatchStatus::Hidden, Some(candidates[0].1.visibility.clone()))
                } else {
                    let typed: Vec<_> = visible
                        .iter()
                        .copied()
                        .filter(|(_, symbol)| symbol.is_function == expect_function)
                        .collect();
                    if typed.is_empty() {
                        if expect_function {
                            (MatchStatus::NotAFunction, Some(visible[0].1.visibility.clone()))
                        } else {
                            (MatchStatus::NotAVariable, Some(visible[0].1.visibility.clone()))
                        }
                    } else if typed
                        .iter()
                        .map(|(inventory, symbol)| format_provider(inventory, symbol))
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        > 1
                    {
                        (
                            MatchStatus::DuplicateProviders,
                            Some(typed[0].1.visibility.clone()),
                        )
                    } else if typed[0].1.binding == SymbolBinding::Weak {
                        (MatchStatus::WeakMatch, Some(typed[0].1.visibility.clone()))
                    } else {
                        (MatchStatus::Matched, Some(typed[0].1.visibility.clone()))
                    }
                }
            }
            None => {
                let status = if !decorated_candidates.is_empty() {
                    MatchStatus::DecorationMismatch
                } else if has_declared_link_inputs(package) {
                    MatchStatus::UnresolvedDeclaredLinkInputs
                } else {
                    MatchStatus::Missing
                };
                (status, None)
            }
        };
        let abi_shape = if status == MatchStatus::Matched && !expect_function {
            variable_ty
                .and_then(|ty| expected_abi_size(package, ty))
                .zip(candidates.first().and_then(|(_, symbol)| symbol.size))
                .map(|(expected_size, observed_size)| {
                    any_abi_shape_evidence = true;
                    if expected_size != observed_size {
                        status = MatchStatus::AbiShapeMismatch;
                    }
                    AbiShapeEvidence {
                        expected_size: Some(expected_size),
                        observed_size: Some(observed_size),
                    }
                })
        } else {
            None
        };
        let routine_abi = if status == MatchStatus::Matched && expect_function {
            let expected_hint = function_hint.as_ref();
            let observed_hint = candidates
                .first()
                .and_then(|(_, symbol)| symbol.function_abi.as_ref());
            let parameter_count = expected_hint
                .and_then(|expected| expected.parameter_count)
                .zip(observed_hint.and_then(|observed| observed.parameter_count));
            let return_size = expected_hint
                .and_then(|expected| expected.return_size)
                .zip(observed_hint.and_then(|observed| observed.return_size));
            let parameter_sizes = match (expected_hint, observed_hint) {
                (Some(expected), Some(observed))
                    if !expected.parameter_sizes.is_empty()
                        && expected.parameter_sizes.len() == observed.parameter_sizes.len()
                        && expected.parameter_sizes.iter().any(|size| size.is_some())
                        && observed.parameter_sizes.iter().any(|size| size.is_some()) =>
                {
                    Some((
                        expected.parameter_sizes.clone(),
                        observed.parameter_sizes.clone(),
                    ))
                }
                _ => None,
            };
            if parameter_count.is_some() || return_size.is_some() {
                any_abi_shape_evidence = true;
            }
            if parameter_sizes.is_some() {
                any_abi_shape_evidence = true;
            }
            parameter_count
                .or_else(|| return_size.map(|_| (0, 0)))
                .or_else(|| parameter_sizes.as_ref().map(|_| (0, 0)))
                .map(|_| {
                    let has_parameter_count = parameter_count.is_some();
                    let has_return_shape = return_size.is_some();
                    let has_parameter_shapes = parameter_sizes.is_some();
                    let mismatch = parameter_count.is_some_and(
                        |(expected_parameter_count, observed_parameter_count)| {
                            expected_parameter_count != observed_parameter_count
                        },
                    ) || return_size.is_some_and(|(expected_return_size, observed_return_size)| {
                        expected_return_size != observed_return_size
                    }) || parameter_sizes.as_ref().is_some_and(|(expected, observed)| {
                        expected
                            .iter()
                            .zip(observed.iter())
                            .any(|(expected_size, observed_size)| {
                                expected_size
                                    .zip(*observed_size)
                                    .is_some_and(|(expected_size, observed_size)| {
                                        expected_size != observed_size
                                    })
                            })
                    });
                    if mismatch {
                        status = MatchStatus::AbiShapeMismatch;
                    }
                    RoutineAbiEvidence {
                        evidence_kind: Some(if mismatch {
                            RoutineAbiEvidenceKind::Mismatch
                        } else {
                            match (
                                has_parameter_count,
                                has_return_shape,
                                has_parameter_shapes,
                            ) {
                                (true, false, false) => {
                                    RoutineAbiEvidenceKind::ParameterCountOnly
                                }
                                (false, true, false) => RoutineAbiEvidenceKind::ReturnShapeOnly,
                                (false, false, true) => {
                                    RoutineAbiEvidenceKind::ParameterShapesOnly
                                }
                                (true, true, false) => {
                                    RoutineAbiEvidenceKind::ParameterCountAndReturnShape
                                }
                                (true, false, true) => {
                                    RoutineAbiEvidenceKind::ParameterCountAndParameterShapes
                                }
                                (false, true, true) => {
                                    RoutineAbiEvidenceKind::ReturnShapeAndParameterShapes
                                }
                                (true, true, true) => RoutineAbiEvidenceKind::FullyShaped,
                                (false, false, false) => RoutineAbiEvidenceKind::ParameterCountOnly,
                            }
                        }),
                        confidence: Some(if mismatch {
                            RoutineAbiConfidence::Mismatch
                        } else if has_parameter_shapes || (has_parameter_count && has_return_shape) {
                            RoutineAbiConfidence::Strong
                        } else {
                            RoutineAbiConfidence::Partial
                        }),
                        expected_parameter_count: parameter_count
                            .map(|(expected_parameter_count, _)| expected_parameter_count),
                        observed_parameter_count: parameter_count
                            .map(|(_, observed_parameter_count)| observed_parameter_count),
                        expected_return_size: return_size
                            .map(|(expected_return_size, _)| expected_return_size),
                        observed_return_size: return_size
                            .map(|(_, observed_return_size)| observed_return_size),
                        expected_parameter_sizes: parameter_sizes
                            .as_ref()
                            .map(|(expected, _)| expected.clone())
                            .unwrap_or_default(),
                        observed_parameter_sizes: parameter_sizes
                            .as_ref()
                            .map(|(_, observed)| observed.clone())
                            .unwrap_or_default(),
                    }
                })
        } else {
            None
        };
        let confidence = match status {
            MatchStatus::Matched => MatchConfidence::High,
            MatchStatus::AbiShapeMismatch => MatchConfidence::Low,
            MatchStatus::WeakMatch => MatchConfidence::Medium,
            MatchStatus::DecorationMismatch
            | MatchStatus::Hidden
            | MatchStatus::DuplicateProviders
            | MatchStatus::UnresolvedDeclaredLinkInputs
            | MatchStatus::NotAFunction
            | MatchStatus::NotAVariable => MatchConfidence::Low,
            MatchStatus::Missing => MatchConfidence::None,
        };
        let evidence_kind = match status {
            MatchStatus::Matched => {
                if abi_shape.is_some() || routine_abi.is_some() {
                    EvidenceKind::AbiShapeVerified
                } else {
                    EvidenceKind::ExactExported
                }
            }
            MatchStatus::AbiShapeMismatch => EvidenceKind::AbiShapeMismatch,
            MatchStatus::WeakMatch => EvidenceKind::WeakExported,
            MatchStatus::Hidden => EvidenceKind::HiddenProvider,
            MatchStatus::DecorationMismatch => EvidenceKind::DecoratedCandidate,
            MatchStatus::DuplicateProviders => EvidenceKind::DuplicateVisibleProviders,
            MatchStatus::UnresolvedDeclaredLinkInputs => {
                if imported_candidates
                    .iter()
                    .any(|(_, symbol)| !symbol.reexported_via.is_empty())
                    || inventories
                        .iter()
                        .any(|inventory| !inventory.dependency_edges.is_empty())
                {
                    EvidenceKind::ReexportedCandidate
                } else {
                    EvidenceKind::DeclaredLinkInputsWithoutProvider
                }
            }
            MatchStatus::Missing => {
                if imported_candidates
                    .iter()
                    .any(|(_, symbol)| !symbol.reexported_via.is_empty())
                {
                    EvidenceKind::ReexportedCandidate
                } else {
                    EvidenceKind::MissingProvider
                }
            }
            MatchStatus::NotAFunction | MatchStatus::NotAVariable => EvidenceKind::KindMismatch,
        };

        matches.push(SymbolMatch {
            name: name.clone(),
            item_kind: kind.clone(),
            status: status.clone(),
            visibility: visibility.clone(),
            provider_artifacts: provider_artifacts.clone(),
            confidence: confidence.clone(),
            evidence_kind: evidence_kind.clone(),
        });
        entries.push(ValidationEntry {
            declaration: ValidationDeclaration {
                name: name.clone(),
                item_kind: kind,
            },
            status,
            evidence: ValidationEvidence {
                provider_artifacts,
                raw_symbol_names,
                visibility,
                confidence,
                evidence_kind,
                abi_shape,
                routine_abi,
            },
        });
    }

    let summary = build_summary(&matches);
    ValidationReport {
        phases: build_validation_phases(any_abi_shape_evidence),
        entries,
        summary,
        matches,
    }
}

fn build_summary(matches: &[SymbolMatch]) -> ValidationSummary {
    let mut summary = ValidationSummary {
        total: matches.len(),
        ..ValidationSummary::default()
    };
    for entry in matches {
        match entry.status {
            MatchStatus::Matched => summary.matched += 1,
            MatchStatus::AbiShapeMismatch => summary.abi_shape_mismatches += 1,
            MatchStatus::Missing => summary.missing += 1,
            MatchStatus::UnresolvedDeclaredLinkInputs => {
                summary.unresolved_declared_link_inputs += 1
            }
            MatchStatus::Hidden => summary.hidden += 1,
            MatchStatus::WeakMatch => summary.weak_matches += 1,
            MatchStatus::DuplicateProviders => summary.duplicate_providers += 1,
            MatchStatus::DecorationMismatch => summary.decoration_mismatches += 1,
            MatchStatus::NotAFunction | MatchStatus::NotAVariable => summary.kind_mismatches += 1,
        }
    }
    summary
}

fn default_validation_phases() -> Vec<ValidationPhaseReport> {
    build_validation_phases(false)
}

fn build_validation_phases(abi_evidence_completed: bool) -> Vec<ValidationPhaseReport> {
    vec![
        ValidationPhaseReport {
            phase: ValidationPhase::ProviderDiscovery,
            completed: true,
        },
        ValidationPhaseReport {
            phase: ValidationPhase::SymbolIdentity,
            completed: true,
        },
        ValidationPhaseReport {
            phase: ValidationPhase::AbiEvidence,
            completed: abi_evidence_completed,
        },
    ]
}

fn expected_abi_size(package: &BindingPackage, ty: &crate::ir::BindingType) -> Option<u64> {
    expected_abi_size_inner(package, ty, &mut std::collections::HashSet::new())
}

fn expected_abi_size_inner(
    package: &BindingPackage,
    ty: &crate::ir::BindingType,
    seen_aliases: &mut std::collections::HashSet<String>,
) -> Option<u64> {
    use crate::ir::BindingType;

    match ty {
        BindingType::Bool | BindingType::Char | BindingType::SChar | BindingType::UChar => Some(1),
        BindingType::Short | BindingType::UShort => Some(2),
        BindingType::Int | BindingType::UInt | BindingType::Float => Some(4),
        BindingType::LongLong | BindingType::ULongLong | BindingType::Double => Some(8),
        BindingType::Long | BindingType::ULong => Some(8),
        BindingType::Qualified { ty, .. } => expected_abi_size_inner(package, ty, seen_aliases),
        BindingType::Array(element, Some(len)) => {
            expected_abi_size_inner(package, element, seen_aliases).map(|size| size * len)
        }
        BindingType::RecordRef(name) => {
            let qualified = format!("struct {}", name);
            find_layout_size(package, &[qualified.as_str(), name.as_str()]).or_else(|| {
                package
                    .find_record(name)
                    .and_then(|record| record.representation.as_ref())
                    .and_then(|representation| representation.size)
            })
        }
        BindingType::EnumRef(name) => {
            let qualified = format!("enum {}", name);
            find_layout_size(package, &[qualified.as_str(), name.as_str()]).or_else(|| {
                package
                    .find_enum(name)
                    .and_then(|item| item.representation.as_ref())
                    .and_then(|representation| representation.underlying_size)
            })
        }
        BindingType::Opaque(name) => find_layout_size(package, &[name]),
        BindingType::TypedefRef(name) => {
            if !seen_aliases.insert(name.clone()) {
                return None;
            }
            package
                .find_type_alias(name)
                .and_then(|alias| {
                    expected_typedef_size(package, alias, seen_aliases)
                        .or_else(|| expected_abi_size_inner(package, &alias.target, seen_aliases))
                })
        }
        _ => None,
    }
}

fn expected_typedef_size(
    package: &BindingPackage,
    alias: &crate::ir::TypeAliasBinding,
    seen_aliases: &mut std::collections::HashSet<String>,
) -> Option<u64> {
    let mut layout_names = vec![alias.name.as_str()];
    if let Some(resolution) = alias.canonical_resolution.as_ref() {
        for name in &resolution.alias_chain {
            layout_names.push(name);
        }
    }
    find_layout_size(package, &layout_names).or_else(|| {
        alias.canonical_resolution
            .as_ref()
            .and_then(|resolution| {
                expected_abi_size_inner(package, &resolution.terminal_target, seen_aliases)
            })
    })
}

fn find_layout_size(package: &BindingPackage, names: &[&str]) -> Option<u64> {
    package
        .layouts
        .iter()
        .find(|layout| names.iter().any(|name| layout.name == *name))
        .map(|layout| layout.size)
}

fn default_match_confidence() -> MatchConfidence {
    MatchConfidence::None
}

fn default_evidence_kind() -> EvidenceKind {
    EvidenceKind::MissingProvider
}

fn format_provider(inventory: &SymbolInventory, symbol: &crate::symbols::SymbolEntry) -> String {
    match &symbol.archive_member {
        Some(member) => format!("{}:{}", inventory.artifact_path, member),
        None => inventory.artifact_path.clone(),
    }
}

fn has_declared_link_inputs(package: &BindingPackage) -> bool {
    !package.link.libraries.is_empty()
        || !package.link.frameworks.is_empty()
        || !package.link.artifacts.is_empty()
        || !package.link.ordered_inputs.is_empty()
}

fn normalize_decorated_name(raw_name: &str) -> String {
    let mut name = raw_name;
    if let Some(stripped) = name.strip_prefix('_') {
        name = stripped;
    }
    if let Some((base, _)) = name.split_once('@') {
        name = base;
    }
    name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::*;
    use crate::symbols::*;

    fn make_inventory_with_vis(
        entries: &[(&str, bool, SymbolVisibility)],
    ) -> SymbolInventory {
        let symbols = entries
            .iter()
            .map(|(name, is_func, vis)| SymbolEntry {
                name: name.to_string(),
                raw_name: None,
                version: None,
                direction: SymbolDirection::Exported,
                reexported_via: Vec::new(),
                alias_of: None,
                visibility: vis.clone(),
                is_function: *is_func,
                binding: SymbolBinding::Global,
                size: None,
                section: None,
                archive_member: None,
                function_abi: None,
            })
            .collect();
        SymbolInventory {
            artifact_path: "test.o".into(),
            format: ArtifactFormat::ElfObject,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::Object,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols,
        }
    }

    fn make_inventory(funcs: &[&str], data: &[&str]) -> SymbolInventory {
        let mut entries: Vec<(&str, bool, SymbolVisibility)> = Vec::new();
        for name in funcs {
            entries.push((name, true, SymbolVisibility::Default));
        }
        for name in data {
            entries.push((name, false, SymbolVisibility::Default));
        }
        make_inventory_with_vis(&entries)
    }

    fn make_package(func_names: &[&str]) -> BindingPackage {
        let items = func_names
            .iter()
            .map(|name| {
                BindingItem::Function(FunctionBinding {
                    name: name.to_string(),
                    calling_convention: CallingConvention::C,
                    parameters: Vec::new(),
                    return_type: BindingType::Void,
                    variadic: false,
                    source_offset: None,
                })
            })
            .collect();
        BindingPackage {
            source_path: None,
            items,
            diagnostics: Vec::new(),
            ..BindingPackage::new()
        }
    }

    fn make_package_with_vars(
        func_names: &[&str],
        var_names: &[&str],
    ) -> BindingPackage {
        let mut items: Vec<BindingItem> = func_names
            .iter()
            .map(|name| {
                BindingItem::Function(FunctionBinding {
                    name: name.to_string(),
                    calling_convention: CallingConvention::C,
                    parameters: Vec::new(),
                    return_type: BindingType::Void,
                    variadic: false,
                    source_offset: None,
                })
            })
            .collect();
        for name in var_names {
            items.push(BindingItem::Variable(VariableBinding {
                name: name.to_string(),
                ty: BindingType::Int,
                source_offset: None,
            }));
        }
        BindingPackage {
            source_path: None,
            items,
            diagnostics: Vec::new(),
            ..BindingPackage::new()
        }
    }

    #[test]
    fn all_functions_matched() {
        let inv = make_inventory(&["foo", "bar"], &[]);
        let pkg = make_package(&["foo", "bar"]);
        let report = validate(&pkg, &inv);
        assert!(report.all_matched());
        assert_eq!(report.matched().len(), 2);
        assert_eq!(report.missing().len(), 0);
    }

    #[test]
    fn some_functions_missing() {
        let inv = make_inventory(&["foo"], &[]);
        let pkg = make_package(&["foo", "bar", "baz"]);
        let report = validate(&pkg, &inv);
        assert!(!report.all_matched());
        assert_eq!(report.matched().len(), 1);
        assert_eq!(report.missing().len(), 2);
    }

    #[test]
    fn missing_symbol_with_declared_link_inputs_is_distinguished() {
        let inv = make_inventory(&[], &[]);
        let mut pkg = make_package(&["foo"]);
        pkg.link.libraries.push(LinkLibrary {
            name: "demo".into(),
            kind: LinkLibraryKind::Default,
            source: LinkRequirementSource::Declared,
        });
        pkg.link.ordered_inputs.push(LinkInput::Library(LinkLibrary {
            name: "demo".into(),
            kind: LinkLibraryKind::Default,
            source: LinkRequirementSource::Declared,
        }));

        let report = validate(&pkg, &inv);
        assert_eq!(report.matches.len(), 1);
        assert_eq!(
            report.matches[0].status,
            MatchStatus::UnresolvedDeclaredLinkInputs
        );
    }

    #[test]
    fn decorated_symbol_is_reported_as_decoration_mismatch() {
        let pkg = make_package(&["foo"]);
        let inv = SymbolInventory {
            artifact_path: "decorated.o".into(),
            format: ArtifactFormat::ElfObject,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::Object,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![SymbolEntry {
                name: "_foo".into(),
                raw_name: Some("_foo".into()),
                version: None,
                direction: SymbolDirection::Exported,
                reexported_via: Vec::new(),
                alias_of: None,
                visibility: SymbolVisibility::Default,
                is_function: true,
                binding: SymbolBinding::Global,
                size: None,
                section: None,
                archive_member: None,
                function_abi: None,
            }],
        };

        let report = validate(&pkg, &inv);
        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].status, MatchStatus::DecorationMismatch);
    }

    #[test]
    fn decorated_symbol_fixture_matches_normalized_name() {
        let pkg = make_package(&["demo_init"]);
        let inv: SymbolInventory = serde_json::from_str(include_str!(
            "../test/contracts/decorated_symbol_inventory_fixture.json"
        ))
        .unwrap();

        let report = validate(&pkg, &inv);
        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].status, MatchStatus::Matched);
        assert_eq!(report.matches[0].provider_artifacts, vec!["demo.lib:demo.obj"]);
    }

    #[test]
    fn symbol_exists_but_not_function() {
        let inv = make_inventory(&[], &["data_sym"]);
        let pkg = make_package(&["data_sym"]);
        let report = validate(&pkg, &inv);
        assert!(!report.all_matched());
        assert_eq!(report.matches[0].status, MatchStatus::NotAFunction);
    }

    #[test]
    fn imported_symbol_does_not_count_as_provider_match() {
        let pkg = make_package(&["puts"]);
        let inv = SymbolInventory {
            artifact_path: "libdemo.so".into(),
            format: ArtifactFormat::ElfSharedLibrary,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::SharedLibrary,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: true,
            },
            dependency_edges: vec!["libc.so.6".into()],
            symbols: vec![SymbolEntry {
                name: "puts".into(),
                raw_name: Some("puts".into()),
                version: Some("GLIBC_2.2.5".into()),
                direction: SymbolDirection::Imported,
                reexported_via: vec!["libc.so.6".into()],
                alias_of: None,
                visibility: SymbolVisibility::Default,
                is_function: true,
                binding: SymbolBinding::Global,
                size: None,
                section: None,
                archive_member: None,
                function_abi: None,
            }],
        };

        let report = validate(&pkg, &inv);
        assert_eq!(report.matches[0].status, MatchStatus::Missing);
        assert_eq!(report.matches[0].evidence_kind, EvidenceKind::ReexportedCandidate);
    }

    #[test]
    fn empty_package() {
        let inv = make_inventory(&["foo"], &[]);
        let pkg = make_package(&[]);
        let report = validate(&pkg, &inv);
        assert!(report.all_matched()); // vacuously true
        assert_eq!(report.matches.len(), 0);
    }

    #[test]
    fn non_function_items_ignored() {
        let inv = make_inventory(&["foo"], &[]);
        let mut pkg = make_package(&["foo"]);
        pkg.items.push(BindingItem::TypeAlias(TypeAliasBinding {
            name: "my_type".into(),
            target: BindingType::Int,
            canonical_resolution: None,
            abi_confidence: None,
            source_offset: None,
        }));
        let report = validate(&pkg, &inv);
        assert_eq!(report.matches.len(), 1); // only the function
        assert!(report.all_matched());
    }

    #[test]
    fn report_serialization() {
        let inv = make_inventory(&["foo"], &[]);
        let pkg = make_package(&["foo", "missing"]);
        let report = validate(&pkg, &inv);
        let json = serde_json::to_string(&report).unwrap();
        let report2: ValidationReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report, report2);
    }

    #[test]
    fn report_exposes_validation_phases() {
        let inv = make_inventory(&["foo"], &[]);
        let pkg = make_package(&["foo"]);
        let report = validate(&pkg, &inv);
        assert_eq!(
            report.phases,
            vec![
                ValidationPhaseReport {
                    phase: ValidationPhase::ProviderDiscovery,
                    completed: true,
                },
                ValidationPhaseReport {
                    phase: ValidationPhase::SymbolIdentity,
                    completed: true,
                },
                ValidationPhaseReport {
                    phase: ValidationPhase::AbiEvidence,
                    completed: false,
                },
            ]
        );
    }

    #[test]
    fn report_entries_preserve_richer_validation_evidence() {
        let pkg = make_package(&["foo"]);
        let inv = SymbolInventory {
            artifact_path: "decorated.o".into(),
            format: ArtifactFormat::ElfObject,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::Object,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![SymbolEntry {
                name: "foo".into(),
                raw_name: Some("_foo".into()),
                version: None,
                direction: SymbolDirection::Exported,
                reexported_via: Vec::new(),
                alias_of: None,
                visibility: SymbolVisibility::Default,
                is_function: true,
                binding: SymbolBinding::Global,
                size: None,
                section: None,
                archive_member: None,
                function_abi: None,
            }],
        };

        let report = validate(&pkg, &inv);
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].declaration.name, "foo");
        assert_eq!(report.entries[0].evidence.provider_artifacts, vec!["decorated.o"]);
        assert_eq!(report.entries[0].evidence.raw_symbol_names, vec!["_foo"]);
        assert_eq!(
            report.entries[0].evidence.visibility,
            Some(SymbolVisibility::Default)
        );
        assert_eq!(report.entries[0].evidence.confidence, MatchConfidence::High);
        assert_eq!(report.matches[0].confidence, MatchConfidence::High);
        assert_eq!(report.entries[0].evidence.evidence_kind, EvidenceKind::ExactExported);
        assert_eq!(report.matches[0].evidence_kind, EvidenceKind::ExactExported);
    }

    #[test]
    fn weak_matches_are_marked_with_medium_confidence() {
        let inv = SymbolInventory {
            artifact_path: "test.o".into(),
            format: ArtifactFormat::ElfObject,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::Object,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![SymbolEntry {
                name: "foo".into(),
                raw_name: None,
                version: None,
                direction: SymbolDirection::Exported,
                reexported_via: Vec::new(),
                alias_of: None,
                visibility: SymbolVisibility::Default,
                is_function: true,
                binding: SymbolBinding::Weak,
                size: None,
                section: None,
                archive_member: None,
                function_abi: None,
            }],
        };
        let pkg = make_package(&["foo"]);
        let report = validate(&pkg, &inv);
        assert_eq!(report.matches[0].confidence, MatchConfidence::Medium);
        assert_eq!(report.entries[0].evidence.confidence, MatchConfidence::Medium);
        assert_eq!(report.matches[0].evidence_kind, EvidenceKind::WeakExported);
    }

    #[test]
    fn unresolved_declared_inputs_with_dependency_edges_are_marked_as_reexport_candidates() {
        let mut pkg = make_package(&["foo"]);
        pkg.link.libraries.push(LinkLibrary {
            name: "demo".into(),
            kind: LinkLibraryKind::Default,
            source: LinkRequirementSource::Declared,
        });

        let inv = SymbolInventory {
            artifact_path: "libdemo.so".into(),
            format: ArtifactFormat::ElfSharedLibrary,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::SharedLibrary,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: true,
            },
            dependency_edges: vec!["libc.so.6".into()],
            symbols: Vec::new(),
        };

        let report = validate(&pkg, &inv);
        assert_eq!(report.matches[0].status, MatchStatus::UnresolvedDeclaredLinkInputs);
        assert_eq!(report.matches[0].evidence_kind, EvidenceKind::ReexportedCandidate);
    }

    #[test]
    fn symbol_specific_imported_candidate_marks_unresolved_as_reexported() {
        let mut pkg = make_package(&["foo"]);
        pkg.link.libraries.push(LinkLibrary {
            name: "demo".into(),
            kind: LinkLibraryKind::Default,
            source: LinkRequirementSource::Declared,
        });

        let inv = SymbolInventory {
            artifact_path: "libdemo.so".into(),
            format: ArtifactFormat::ElfSharedLibrary,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::SharedLibrary,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: true,
            },
            dependency_edges: vec!["libdep.so".into()],
            symbols: vec![SymbolEntry {
                name: "foo".into(),
                raw_name: Some("foo".into()),
                version: None,
                direction: SymbolDirection::Imported,
                reexported_via: vec!["libdep.so".into()],
                alias_of: None,
                visibility: SymbolVisibility::Default,
                is_function: true,
                binding: SymbolBinding::Global,
                size: None,
                section: None,
                archive_member: None,
                function_abi: None,
            }],
        };

        let report = validate(&pkg, &inv);
        assert_eq!(report.matches[0].status, MatchStatus::UnresolvedDeclaredLinkInputs);
        assert_eq!(report.matches[0].evidence_kind, EvidenceKind::ReexportedCandidate);
        assert_eq!(report.entries[0].evidence.raw_symbol_names, vec!["foo"]);
    }

    #[test]
    fn report_summary_and_query_helpers_track_statuses() {
        let pkg = make_package(&["foo", "bar", "baz"]);
        let inv = SymbolInventory {
            artifact_path: "test.o".into(),
            format: ArtifactFormat::ElfObject,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::Object,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![
                SymbolEntry {
                    name: "foo".into(),
                    raw_name: None,
                    version: None,
                    direction: SymbolDirection::Exported,
                    reexported_via: Vec::new(),
                    alias_of: None,
                    visibility: SymbolVisibility::Default,
                    is_function: true,
                    binding: SymbolBinding::Global,
                    size: None,
                    section: None,
                    archive_member: None,
                    function_abi: None,
                },
                SymbolEntry {
                    name: "bar".into(),
                    raw_name: None,
                    version: None,
                    direction: SymbolDirection::Exported,
                    reexported_via: Vec::new(),
                    alias_of: None,
                    visibility: SymbolVisibility::Default,
                    is_function: true,
                    binding: SymbolBinding::Weak,
                    size: None,
                    section: None,
                    archive_member: None,
                    function_abi: None,
                },
            ],
        };

        let report = validate(&pkg, &inv);
        assert_eq!(report.summary.total, 3);
        assert_eq!(report.summary.matched, 1);
        assert_eq!(report.summary.weak_matches, 1);
        assert_eq!(report.summary.missing, 1);
        assert_eq!(report.weak_matches().len(), 1);
        assert_eq!(report.unresolved_declared().len(), 0);
        assert_eq!(report.duplicate_providers().len(), 0);
    }

    #[test]
    fn duplicate_provider_contract_fixture_is_consumable() {
        let report: ValidationReport = serde_json::from_str(include_str!(
            "../test/contracts/validation_duplicate_provider_report.json"
        ))
        .unwrap();
        assert_eq!(report.summary.duplicate_providers, 1);
        assert_eq!(report.matches[0].status, MatchStatus::DuplicateProviders);
        assert_eq!(
            report.entries[0].evidence.evidence_kind,
            EvidenceKind::DuplicateVisibleProviders
        );
    }

    #[test]
    fn validate_many_collects_provider_artifacts() {
        let pkg = make_package(&["foo", "bar"]);
        let inv1 = make_inventory(&["foo"], &[]);
        let inv2 = SymbolInventory {
            artifact_path: "libbar.a".into(),
            format: ArtifactFormat::ElfStaticLibrary,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::StaticLibrary,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![SymbolEntry {
                name: "bar".into(),
                raw_name: Some("bar".into()),
                version: None,
                direction: SymbolDirection::Exported,
                reexported_via: Vec::new(),
                alias_of: None,
                visibility: SymbolVisibility::Default,
                is_function: true,
                binding: SymbolBinding::Global,
                size: None,
                section: None,
                archive_member: Some("bar.o".into()),
                function_abi: None,
            }],
        };

        let report = validate_many(&pkg, &[inv1, inv2]);
        assert_eq!(report.matches.len(), 2);
        let foo = report.matches.iter().find(|entry| entry.name == "foo").unwrap();
        let bar = report.matches.iter().find(|entry| entry.name == "bar").unwrap();
        assert_eq!(foo.status, MatchStatus::Matched);
        assert_eq!(foo.provider_artifacts, vec!["test.o".to_string()]);
        assert_eq!(bar.status, MatchStatus::Matched);
        assert_eq!(bar.provider_artifacts, vec!["libbar.a:bar.o".to_string()]);
    }

    #[test]
    fn validate_many_reports_duplicate_providers() {
        let pkg = make_package(&["foo"]);
        let inv1 = SymbolInventory {
            artifact_path: "libfoo_one.a".into(),
            format: ArtifactFormat::ElfStaticLibrary,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::StaticLibrary,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![SymbolEntry {
                name: "foo".into(),
                raw_name: Some("foo".into()),
                version: None,
                direction: SymbolDirection::Exported,
                reexported_via: Vec::new(),
                alias_of: None,
                visibility: SymbolVisibility::Default,
                is_function: true,
                binding: SymbolBinding::Global,
                size: None,
                section: None,
                archive_member: Some("foo1.o".into()),
                function_abi: None,
            }],
        };
        let inv2 = SymbolInventory {
            artifact_path: "libfoo_two.a".into(),
            format: ArtifactFormat::ElfStaticLibrary,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::StaticLibrary,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![SymbolEntry {
                name: "foo".into(),
                raw_name: Some("foo".into()),
                version: None,
                direction: SymbolDirection::Exported,
                reexported_via: Vec::new(),
                alias_of: None,
                visibility: SymbolVisibility::Default,
                is_function: true,
                binding: SymbolBinding::Global,
                size: None,
                section: None,
                archive_member: Some("foo2.o".into()),
                function_abi: None,
            }],
        };

        let report = validate_many(&pkg, &[inv1, inv2]);
        assert_eq!(report.matches.len(), 1);
        let foo = &report.matches[0];
        assert_eq!(foo.status, MatchStatus::DuplicateProviders);
        assert_eq!(
            foo.provider_artifacts,
            vec![
                "libfoo_one.a:foo1.o".to_string(),
                "libfoo_two.a:foo2.o".to_string()
            ]
        );
    }

    // --- Phase 12 tests ---

    #[test]
    fn variable_matched() {
        let inv = make_inventory(&[], &["errno"]);
        let pkg = make_package_with_vars(&[], &["errno"]);
        let report = validate(&pkg, &inv);
        assert!(report.all_matched());
        assert_eq!(report.matched().len(), 1);
        assert_eq!(report.matches[0].item_kind, ItemKind::Variable);
    }

    #[test]
    fn variable_missing() {
        let inv = make_inventory(&[], &[]);
        let pkg = make_package_with_vars(&[], &["errno"]);
        let report = validate(&pkg, &inv);
        assert!(!report.all_matched());
        assert_eq!(report.missing().len(), 1);
        assert_eq!(report.missing()[0].item_kind, ItemKind::Variable);
    }

    #[test]
    fn variable_name_is_function() {
        let inv = make_inventory(&["errno"], &[]);
        let pkg = make_package_with_vars(&[], &["errno"]);
        let report = validate(&pkg, &inv);
        assert!(!report.all_matched());
        assert_eq!(report.matches[0].status, MatchStatus::NotAVariable);
    }

    #[test]
    fn variable_size_match_records_abi_shape_evidence() {
        let inv = SymbolInventory {
            artifact_path: "test.o".into(),
            format: ArtifactFormat::ElfObject,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::Object,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![SymbolEntry {
                name: "errno".into(),
                raw_name: None,
                version: None,
                direction: SymbolDirection::Exported,
                reexported_via: Vec::new(),
                alias_of: None,
                visibility: SymbolVisibility::Default,
                is_function: false,
                binding: SymbolBinding::Global,
                size: Some(4),
                section: Some(".data".into()),
                archive_member: None,
                function_abi: None,
            }],
        };
        let pkg = make_package_with_vars(&[], &["errno"]);
        let report = validate(&pkg, &inv);
        assert_eq!(report.matches[0].status, MatchStatus::Matched);
        assert_eq!(report.matches[0].evidence_kind, EvidenceKind::AbiShapeVerified);
        assert_eq!(
            report.entries[0].evidence.abi_shape,
            Some(AbiShapeEvidence {
                expected_size: Some(4),
                observed_size: Some(4),
            })
        );
        assert!(report.phases[2].completed);
    }

    #[test]
    fn function_parameter_count_match_records_routine_abi_evidence() {
        let inv = SymbolInventory {
            artifact_path: "test.o".into(),
            format: ArtifactFormat::ElfObject,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::Object,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![SymbolEntry {
                name: "add".into(),
                raw_name: None,
                version: None,
                direction: SymbolDirection::Exported,
                reexported_via: Vec::new(),
                alias_of: None,
                visibility: SymbolVisibility::Default,
                is_function: true,
                binding: SymbolBinding::Global,
                size: None,
                section: Some(".text".into()),
                archive_member: None,
                function_abi: Some(FunctionAbiHint {
                    parameter_count: Some(2),
                    return_size: Some(4),
                    parameter_sizes: vec![Some(4), Some(4)],
                }),
            }],
        };
        let pkg = BindingPackage {
            source_path: None,
            items: vec![BindingItem::Function(FunctionBinding {
                name: "add".into(),
                calling_convention: CallingConvention::C,
                parameters: vec![
                    ParameterBinding {
                        name: Some("a".into()),
                        ty: BindingType::Int,
                    },
                    ParameterBinding {
                        name: Some("b".into()),
                        ty: BindingType::Int,
                    },
                ],
                return_type: BindingType::Int,
                variadic: false,
                source_offset: None,
            })],
            diagnostics: Vec::new(),
            ..BindingPackage::new()
        };

        let report = validate(&pkg, &inv);
        assert_eq!(report.matches[0].status, MatchStatus::Matched);
        assert_eq!(report.matches[0].evidence_kind, EvidenceKind::AbiShapeVerified);
        assert_eq!(
            report.entries[0].evidence.routine_abi,
            Some(RoutineAbiEvidence {
                evidence_kind: Some(RoutineAbiEvidenceKind::FullyShaped),
                confidence: Some(RoutineAbiConfidence::Strong),
                expected_parameter_count: Some(2),
                observed_parameter_count: Some(2),
                expected_return_size: Some(4),
                observed_return_size: Some(4),
                expected_parameter_sizes: vec![Some(4), Some(4)],
                observed_parameter_sizes: vec![Some(4), Some(4)],
            })
        );
        assert!(report.phases[2].completed);
    }

    #[test]
    fn function_parameter_count_mismatch_is_reported_as_abi_shape_mismatch() {
        let inv = SymbolInventory {
            artifact_path: "test.o".into(),
            format: ArtifactFormat::ElfObject,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::Object,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![SymbolEntry {
                name: "add".into(),
                raw_name: None,
                version: None,
                direction: SymbolDirection::Exported,
                reexported_via: Vec::new(),
                alias_of: None,
                visibility: SymbolVisibility::Default,
                is_function: true,
                binding: SymbolBinding::Global,
                size: None,
                section: Some(".text".into()),
                archive_member: None,
                function_abi: Some(FunctionAbiHint {
                    parameter_count: Some(1),
                    return_size: Some(4),
                    parameter_sizes: vec![Some(4)],
                }),
            }],
        };
        let pkg = BindingPackage {
            source_path: None,
            items: vec![BindingItem::Function(FunctionBinding {
                name: "add".into(),
                calling_convention: CallingConvention::C,
                parameters: vec![
                    ParameterBinding {
                        name: Some("a".into()),
                        ty: BindingType::Int,
                    },
                    ParameterBinding {
                        name: Some("b".into()),
                        ty: BindingType::Int,
                    },
                ],
                return_type: BindingType::Int,
                variadic: false,
                source_offset: None,
            })],
            diagnostics: Vec::new(),
            ..BindingPackage::new()
        };

        let report = validate(&pkg, &inv);
        assert_eq!(report.matches[0].status, MatchStatus::AbiShapeMismatch);
        assert_eq!(report.matches[0].evidence_kind, EvidenceKind::AbiShapeMismatch);
        assert_eq!(report.summary.abi_shape_mismatches, 1);
        assert_eq!(
            report.entries[0].evidence.routine_abi,
            Some(RoutineAbiEvidence {
                evidence_kind: Some(RoutineAbiEvidenceKind::Mismatch),
                confidence: Some(RoutineAbiConfidence::Mismatch),
                expected_parameter_count: Some(2),
                observed_parameter_count: Some(1),
                expected_return_size: Some(4),
                observed_return_size: Some(4),
                expected_parameter_sizes: Vec::new(),
                observed_parameter_sizes: Vec::new(),
            })
        );
        assert!(report.phases[2].completed);
    }

    #[test]
    fn function_return_size_mismatch_is_reported_as_abi_shape_mismatch() {
        let inv = SymbolInventory {
            artifact_path: "test.o".into(),
            format: ArtifactFormat::ElfObject,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::Object,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![SymbolEntry {
                name: "status".into(),
                raw_name: None,
                version: None,
                direction: SymbolDirection::Exported,
                reexported_via: Vec::new(),
                alias_of: None,
                visibility: SymbolVisibility::Default,
                is_function: true,
                binding: SymbolBinding::Global,
                size: None,
                section: Some(".text".into()),
                archive_member: None,
                function_abi: Some(FunctionAbiHint {
                    parameter_count: Some(0),
                    return_size: Some(8),
                    parameter_sizes: Vec::new(),
                }),
            }],
        };
        let pkg = BindingPackage {
            source_path: None,
            items: vec![BindingItem::Function(FunctionBinding {
                name: "status".into(),
                calling_convention: CallingConvention::C,
                parameters: Vec::new(),
                return_type: BindingType::Int,
                variadic: false,
                source_offset: None,
            })],
            diagnostics: Vec::new(),
            ..BindingPackage::new()
        };

        let report = validate(&pkg, &inv);
        assert_eq!(report.matches[0].status, MatchStatus::AbiShapeMismatch);
        assert_eq!(
            report.entries[0].evidence.routine_abi,
            Some(RoutineAbiEvidence {
                evidence_kind: Some(RoutineAbiEvidenceKind::Mismatch),
                confidence: Some(RoutineAbiConfidence::Mismatch),
                expected_parameter_count: Some(0),
                observed_parameter_count: Some(0),
                expected_return_size: Some(4),
                observed_return_size: Some(8),
                expected_parameter_sizes: Vec::new(),
                observed_parameter_sizes: Vec::new(),
            })
        );
    }

    #[test]
    fn function_parameter_size_mismatch_is_reported_as_abi_shape_mismatch() {
        let inv = SymbolInventory {
            artifact_path: "test.o".into(),
            format: ArtifactFormat::ElfObject,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::Object,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![SymbolEntry {
                name: "mix".into(),
                raw_name: None,
                version: None,
                direction: SymbolDirection::Exported,
                reexported_via: Vec::new(),
                alias_of: None,
                visibility: SymbolVisibility::Default,
                is_function: true,
                binding: SymbolBinding::Global,
                size: None,
                section: Some(".text".into()),
                archive_member: None,
                function_abi: Some(FunctionAbiHint {
                    parameter_count: Some(2),
                    return_size: Some(4),
                    parameter_sizes: vec![Some(4), Some(8)],
                }),
            }],
        };
        let pkg = BindingPackage {
            source_path: None,
            items: vec![BindingItem::Function(FunctionBinding {
                name: "mix".into(),
                calling_convention: CallingConvention::C,
                parameters: vec![
                    ParameterBinding {
                        name: Some("left".into()),
                        ty: BindingType::Int,
                    },
                    ParameterBinding {
                        name: Some("right".into()),
                        ty: BindingType::Float,
                    },
                ],
                return_type: BindingType::Int,
                variadic: false,
                source_offset: None,
            })],
            diagnostics: Vec::new(),
            ..BindingPackage::new()
        };

        let report = validate(&pkg, &inv);
        assert_eq!(report.matches[0].status, MatchStatus::AbiShapeMismatch);
        assert_eq!(
            report.entries[0].evidence.routine_abi,
            Some(RoutineAbiEvidence {
                evidence_kind: Some(RoutineAbiEvidenceKind::Mismatch),
                confidence: Some(RoutineAbiConfidence::Mismatch),
                expected_parameter_count: Some(2),
                observed_parameter_count: Some(2),
                expected_return_size: Some(4),
                observed_return_size: Some(4),
                expected_parameter_sizes: vec![Some(4), Some(4)],
                observed_parameter_sizes: vec![Some(4), Some(8)],
            })
        );
    }

    #[test]
    fn function_parameter_count_only_evidence_is_marked_partial() {
        let inv = SymbolInventory {
            artifact_path: "test.o".into(),
            format: ArtifactFormat::ElfObject,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::Object,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![SymbolEntry {
                name: "ping".into(),
                raw_name: None,
                version: None,
                direction: SymbolDirection::Exported,
                reexported_via: Vec::new(),
                alias_of: None,
                visibility: SymbolVisibility::Default,
                is_function: true,
                binding: SymbolBinding::Global,
                size: None,
                section: Some(".text".into()),
                archive_member: None,
                function_abi: Some(FunctionAbiHint {
                    parameter_count: Some(1),
                    return_size: None,
                    parameter_sizes: Vec::new(),
                }),
            }],
        };
        let pkg = BindingPackage {
            source_path: None,
            items: vec![BindingItem::Function(FunctionBinding {
                name: "ping".into(),
                calling_convention: CallingConvention::C,
                parameters: vec![ParameterBinding {
                    name: Some("value".into()),
                    ty: BindingType::Int,
                }],
                return_type: BindingType::Void,
                variadic: false,
                source_offset: None,
            })],
            diagnostics: Vec::new(),
            ..BindingPackage::new()
        };

        let report = validate(&pkg, &inv);
        assert_eq!(report.matches[0].status, MatchStatus::Matched);
        assert_eq!(
            report.entries[0].evidence.routine_abi,
            Some(RoutineAbiEvidence {
                evidence_kind: Some(RoutineAbiEvidenceKind::ParameterCountOnly),
                confidence: Some(RoutineAbiConfidence::Partial),
                expected_parameter_count: Some(1),
                observed_parameter_count: Some(1),
                expected_return_size: None,
                observed_return_size: None,
                expected_parameter_sizes: Vec::new(),
                observed_parameter_sizes: Vec::new(),
            })
        );
    }

    #[test]
    fn typedef_layouts_drive_variable_abi_checks() {
        let inv = SymbolInventory {
            artifact_path: "test.o".into(),
            format: ArtifactFormat::ElfObject,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::Object,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![SymbolEntry {
                name: "widget_state".into(),
                raw_name: None,
                version: None,
                direction: SymbolDirection::Exported,
                reexported_via: Vec::new(),
                alias_of: None,
                visibility: SymbolVisibility::Default,
                is_function: false,
                binding: SymbolBinding::Global,
                size: Some(16),
                section: Some(".data".into()),
                archive_member: None,
                function_abi: None,
            }],
        };
        let pkg = BindingPackage {
            source_path: None,
            items: vec![
                BindingItem::TypeAlias(TypeAliasBinding {
                    name: "widget_state_t".into(),
                    target: BindingType::RecordRef("widget_state".into()),
                    canonical_resolution: Some(AliasResolution {
                        alias_chain: vec!["widget_state_t".into()],
                        terminal_target: BindingType::RecordRef("widget_state".into()),
                    }),
                    abi_confidence: None,
                    source_offset: None,
                }),
                BindingItem::Variable(VariableBinding {
                    name: "widget_state".into(),
                    ty: BindingType::TypedefRef("widget_state_t".into()),
                    source_offset: None,
                }),
            ],
            diagnostics: Vec::new(),
            layouts: vec![TypeLayout {
                name: "widget_state_t".into(),
                size: 16,
                align: 8,
            }],
            ..BindingPackage::new()
        };

        let report = validate(&pkg, &inv);
        assert_eq!(report.matches[0].status, MatchStatus::Matched);
        assert_eq!(report.matches[0].evidence_kind, EvidenceKind::AbiShapeVerified);
    }

    #[test]
    fn record_and_enum_representation_sizes_drive_abi_checks_without_layouts() {
        let inv = SymbolInventory {
            artifact_path: "test.o".into(),
            format: ArtifactFormat::ElfObject,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::Object,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![
                SymbolEntry {
                    name: "widget".into(),
                    raw_name: None,
                    version: None,
                    direction: SymbolDirection::Exported,
                    reexported_via: Vec::new(),
                    alias_of: None,
                    visibility: SymbolVisibility::Default,
                    is_function: false,
                    binding: SymbolBinding::Global,
                    size: Some(24),
                    section: Some(".data".into()),
                    archive_member: None,
                    function_abi: None,
                },
                SymbolEntry {
                    name: "mode".into(),
                    raw_name: None,
                    version: None,
                    direction: SymbolDirection::Exported,
                    reexported_via: Vec::new(),
                    alias_of: None,
                    visibility: SymbolVisibility::Default,
                    is_function: false,
                    binding: SymbolBinding::Global,
                    size: Some(4),
                    section: Some(".data".into()),
                    archive_member: None,
                    function_abi: None,
                },
            ],
        };
        let pkg = BindingPackage {
            source_path: None,
            items: vec![
                BindingItem::Record(RecordBinding {
                    kind: RecordKind::Struct,
                    name: Some("widget".into()),
                    fields: None,
                    representation: Some(RecordRepresentation {
                        size: Some(24),
                        align: Some(8),
                        completeness: Some("Complete".into()),
                    }),
                    abi_confidence: Some(AbiConfidence::RepresentationProbed),
                    source_offset: None,
                }),
                BindingItem::Enum(EnumBinding {
                    name: Some("mode".into()),
                    variants: Vec::new(),
                    representation: Some(EnumRepresentation {
                        underlying_size: Some(4),
                        is_signed: Some(true),
                    }),
                    abi_confidence: Some(AbiConfidence::RepresentationProbed),
                    source_offset: None,
                }),
                BindingItem::Variable(VariableBinding {
                    name: "widget".into(),
                    ty: BindingType::RecordRef("widget".into()),
                    source_offset: None,
                }),
                BindingItem::Variable(VariableBinding {
                    name: "mode".into(),
                    ty: BindingType::EnumRef("mode".into()),
                    source_offset: None,
                }),
            ],
            diagnostics: Vec::new(),
            ..BindingPackage::new()
        };

        let report = validate(&pkg, &inv);
        assert_eq!(report.matches.len(), 2);
        assert!(report.matches.iter().all(|entry| entry.status == MatchStatus::Matched));
        assert!(
            report
                .entries
                .iter()
                .all(|entry| entry.evidence.abi_shape.is_some())
        );
    }

    #[test]
    fn layout_backed_confidence_helpers_find_entries_with_size_evidence() {
        let inv = SymbolInventory {
            artifact_path: "test.o".into(),
            format: ArtifactFormat::ElfObject,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::Object,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![
                SymbolEntry {
                    name: "widget".into(),
                    raw_name: None,
                    version: None,
                    direction: SymbolDirection::Exported,
                    reexported_via: Vec::new(),
                    alias_of: None,
                    visibility: SymbolVisibility::Default,
                    is_function: false,
                    binding: SymbolBinding::Global,
                    size: Some(24),
                    section: Some(".data".into()),
                    archive_member: None,
                    function_abi: None,
                },
                SymbolEntry {
                    name: "ping".into(),
                    raw_name: None,
                    version: None,
                    direction: SymbolDirection::Exported,
                    reexported_via: Vec::new(),
                    alias_of: None,
                    visibility: SymbolVisibility::Default,
                    is_function: true,
                    binding: SymbolBinding::Global,
                    size: None,
                    section: Some(".text".into()),
                    archive_member: None,
                    function_abi: Some(FunctionAbiHint {
                        parameter_count: Some(1),
                        return_size: Some(4),
                        parameter_sizes: vec![Some(4)],
                    }),
                },
            ],
        };
        let pkg = BindingPackage {
            source_path: None,
            items: vec![
                BindingItem::Record(RecordBinding {
                    kind: RecordKind::Struct,
                    name: Some("widget".into()),
                    fields: None,
                    representation: Some(RecordRepresentation {
                        size: Some(24),
                        align: Some(8),
                        completeness: Some("Complete".into()),
                    }),
                    abi_confidence: Some(AbiConfidence::RepresentationProbed),
                    source_offset: None,
                }),
                BindingItem::Variable(VariableBinding {
                    name: "widget".into(),
                    ty: BindingType::RecordRef("widget".into()),
                    source_offset: None,
                }),
                BindingItem::Function(FunctionBinding {
                    name: "ping".into(),
                    calling_convention: CallingConvention::C,
                    parameters: vec![ParameterBinding {
                        name: Some("value".into()),
                        ty: BindingType::Int,
                    }],
                    return_type: BindingType::Int,
                    variadic: false,
                    source_offset: None,
                }),
            ],
            diagnostics: Vec::new(),
            ..BindingPackage::new()
        };

        let report = validate(&pkg, &inv);
        assert_eq!(report.layout_backed_entries().len(), 2);
        assert!(report.entries.iter().all(|entry| entry.has_layout_backed_confidence()));
        assert!(
            report
                .entries
                .iter()
                .all(|entry| entry.evidence.has_layout_backed_confidence())
        );
    }

    #[test]
    fn provider_state_helpers_group_resolved_unresolved_and_ambiguous_entries() {
        let pkg = make_package(&["ok", "missing", "dup"]);
        let inv1 = SymbolInventory {
            artifact_path: "libone.a".into(),
            format: ArtifactFormat::ElfStaticLibrary,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::StaticLibrary,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![
                SymbolEntry {
                    name: "ok".into(),
                    raw_name: None,
                    version: None,
                    direction: SymbolDirection::Exported,
                    reexported_via: Vec::new(),
                    alias_of: None,
                    visibility: SymbolVisibility::Default,
                    is_function: true,
                    binding: SymbolBinding::Global,
                    size: None,
                    section: None,
                    archive_member: Some("ok.o".into()),
                    function_abi: None,
                },
                SymbolEntry {
                    name: "dup".into(),
                    raw_name: None,
                    version: None,
                    direction: SymbolDirection::Exported,
                    reexported_via: Vec::new(),
                    alias_of: None,
                    visibility: SymbolVisibility::Default,
                    is_function: true,
                    binding: SymbolBinding::Global,
                    size: None,
                    section: None,
                    archive_member: Some("dup1.o".into()),
                    function_abi: None,
                },
            ],
        };
        let inv2 = SymbolInventory {
            artifact_path: "libtwo.a".into(),
            format: ArtifactFormat::ElfStaticLibrary,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::StaticLibrary,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![SymbolEntry {
                name: "dup".into(),
                raw_name: None,
                version: None,
                direction: SymbolDirection::Exported,
                reexported_via: Vec::new(),
                alias_of: None,
                visibility: SymbolVisibility::Default,
                is_function: true,
                binding: SymbolBinding::Global,
                size: None,
                section: None,
                archive_member: Some("dup2.o".into()),
                function_abi: None,
            }],
        };

        let report = validate_many(&pkg, &[inv1, inv2]);
        assert_eq!(report.resolved_provider_entries().len(), 1);
        assert_eq!(report.unresolved_provider_entries().len(), 1);
        assert_eq!(report.ambiguous_provider_entries().len(), 1);
        assert!(
            report
                .resolved_provider_entries()
                .iter()
                .all(|entry| entry.has_resolved_provider_state())
        );
        assert!(
            report
                .unresolved_provider_entries()
                .iter()
                .all(|entry| entry.has_unresolved_provider_state())
        );
        assert!(
            report
                .ambiguous_provider_entries()
                .iter()
                .all(|entry| entry.has_ambiguous_provider_state())
        );
    }

    #[test]
    fn variable_size_mismatch_is_reported_as_abi_shape_mismatch() {
        let inv = SymbolInventory {
            artifact_path: "test.o".into(),
            format: ArtifactFormat::ElfObject,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::Object,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![SymbolEntry {
                name: "errno".into(),
                raw_name: None,
                version: None,
                direction: SymbolDirection::Exported,
                reexported_via: Vec::new(),
                alias_of: None,
                visibility: SymbolVisibility::Default,
                is_function: false,
                binding: SymbolBinding::Global,
                size: Some(8),
                section: Some(".data".into()),
                archive_member: None,
                function_abi: None,
            }],
        };
        let pkg = make_package_with_vars(&[], &["errno"]);
        let report = validate(&pkg, &inv);
        assert_eq!(report.matches[0].status, MatchStatus::AbiShapeMismatch);
        assert_eq!(report.matches[0].evidence_kind, EvidenceKind::AbiShapeMismatch);
        assert_eq!(report.summary.abi_shape_mismatches, 1);
        assert_eq!(
            report.entries[0].evidence.abi_shape,
            Some(AbiShapeEvidence {
                expected_size: Some(4),
                observed_size: Some(8),
            })
        );
        assert!(report.phases[2].completed);
    }

    #[test]
    fn mixed_functions_and_variables() {
        let inv = make_inventory(&["foo"], &["bar"]);
        let pkg = make_package_with_vars(&["foo"], &["bar"]);
        let report = validate(&pkg, &inv);
        assert!(report.all_matched());
        assert_eq!(report.matched().len(), 2);
    }

    #[test]
    fn hidden_function_not_matched() {
        let inv = make_inventory_with_vis(&[("foo", true, SymbolVisibility::Hidden)]);
        let pkg = make_package(&["foo"]);
        let report = validate(&pkg, &inv);
        assert!(!report.all_matched());
        assert_eq!(report.hidden().len(), 1);
        assert_eq!(
            report.matches[0].visibility,
            Some(SymbolVisibility::Hidden)
        );
    }

    #[test]
    fn internal_variable_not_matched() {
        let inv = make_inventory_with_vis(&[("data", false, SymbolVisibility::Internal)]);
        let pkg = make_package_with_vars(&[], &["data"]);
        let report = validate(&pkg, &inv);
        assert!(!report.all_matched());
        assert_eq!(report.hidden().len(), 1);
    }

    #[test]
    fn weak_function_match() {
        let inv = SymbolInventory {
            artifact_path: "test.o".into(),
            format: ArtifactFormat::ElfObject,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::Object,
            capabilities: ArtifactCapabilities {
                exports_symbols: true,
                imports_symbols: false,
            },
            dependency_edges: Vec::new(),
            symbols: vec![SymbolEntry {
                name: "foo".into(),
                raw_name: None,
                version: None,
                direction: SymbolDirection::Exported,
                reexported_via: Vec::new(),
                alias_of: None,
                visibility: SymbolVisibility::Default,
                is_function: true,
                binding: SymbolBinding::Weak,
                size: None,
                section: None,
                archive_member: None,
                function_abi: None,
            }],
        };
        let pkg = make_package(&["foo"]);
        let report = validate(&pkg, &inv);
        assert_eq!(report.matches[0].status, MatchStatus::WeakMatch);
        // WeakMatch is not the same as Matched
        assert!(!report.all_matched());
    }

    #[test]
    fn default_visibility_matched() {
        let inv = make_inventory_with_vis(&[("foo", true, SymbolVisibility::Default)]);
        let pkg = make_package(&["foo"]);
        let report = validate(&pkg, &inv);
        assert!(report.all_matched());
        assert_eq!(
            report.matches[0].visibility,
            Some(SymbolVisibility::Default)
        );
    }

    #[test]
    fn match_has_item_kind() {
        let inv = make_inventory(&["foo"], &["bar"]);
        let pkg = make_package_with_vars(&["foo"], &["bar"]);
        let report = validate(&pkg, &inv);
        let func_match = report.matches.iter().find(|m| m.name == "foo").unwrap();
        let var_match = report.matches.iter().find(|m| m.name == "bar").unwrap();
        assert_eq!(func_match.item_kind, ItemKind::Function);
        assert_eq!(var_match.item_kind, ItemKind::Variable);
    }

    /// End-to-end: parse C, compile it, validate symbols.
    #[test]
    fn end_to_end_validation() {
        let c_src = "int add(int a, int b) { return a + b; }\nint mul(int a, int b) { return a * b; }\n";
        let dir = std::env::temp_dir().join("linc_validate_test");
        std::fs::create_dir_all(&dir).unwrap();
        let c_path = dir.join("funcs.c");
        let o_path = dir.join("funcs.o");
        std::fs::write(&c_path, c_src).unwrap();

        let status = std::process::Command::new("cc")
            .args(["-c", "-o"])
            .arg(&o_path)
            .arg(&c_path)
            .status()
            .expect("cc not found");
        assert!(status.success());

        // Parse declarations
        let header = "int add(int a, int b); int mul(int a, int b); int missing_func(void);";
        let pkg = crate::extract::extract_from_source(header).unwrap();

        // Inspect symbols
        let inv = crate::symbols::inspect_file(&o_path).unwrap();

        // Validate
        let report = validate(&pkg, &inv);
        assert_eq!(report.matched().len(), 2);
        assert_eq!(report.missing().len(), 1);
        assert_eq!(report.missing()[0].name, "missing_func");

        std::fs::remove_file(&c_path).ok();
        std::fs::remove_file(&o_path).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[test]
    fn validation_report_json_roundtrip() {
        let pkg = make_package(&["init", "shutdown", "missing_fn"]);
        let inv = make_inventory(&["init", "shutdown"], &[]);
        let report = validate(&pkg, &inv);

        let json = serde_json::to_string_pretty(&report).unwrap();
        let restored: ValidationReport = serde_json::from_str(&json).unwrap();

        assert_eq!(report.entries.len(), restored.entries.len());
        assert_eq!(report.matched().len(), restored.matched().len());
        assert_eq!(report.missing().len(), restored.missing().len());

        // Verify specific content survived the roundtrip
        let missing: Vec<&str> = restored.missing().iter().map(|m| m.name.as_str()).collect();
        assert!(missing.contains(&"missing_fn"));
    }

    #[test]
    fn validation_summary_reflects_report() {
        let pkg = make_package_with_vars(&["foo", "bar"], &["global_x"]);
        let inv = make_inventory(&["foo"], &["global_x"]);
        let report = validate(&pkg, &inv);

        assert_eq!(report.summary.total, 3);
        assert_eq!(report.summary.matched, 2);
        assert_eq!(report.summary.missing, 1);
    }

    #[test]
    fn validation_entry_provider_state_helpers() {
        let matched_entry = ValidationEntry {
            declaration: ValidationDeclaration { name: "foo".into(), item_kind: ItemKind::Function },
            status: MatchStatus::Matched,
            evidence: ValidationEvidence {
                provider_artifacts: vec!["lib.so".into()],
                raw_symbol_names: vec!["foo".into()],
                visibility: Some(SymbolVisibility::Default),
                confidence: MatchConfidence::High,
                evidence_kind: EvidenceKind::ExactExported,
                abi_shape: None,
                routine_abi: None,
            },
        };
        assert!(matched_entry.has_resolved_provider_state());
        assert!(!matched_entry.has_unresolved_provider_state());
        assert!(!matched_entry.has_ambiguous_provider_state());
        assert!(!matched_entry.has_layout_backed_confidence());

        let missing_entry = ValidationEntry {
            declaration: ValidationDeclaration { name: "bar".into(), item_kind: ItemKind::Function },
            status: MatchStatus::Missing,
            evidence: ValidationEvidence {
                provider_artifacts: vec![],
                raw_symbol_names: vec![],
                visibility: None,
                confidence: MatchConfidence::None,
                evidence_kind: EvidenceKind::MissingProvider,
                abi_shape: None,
                routine_abi: None,
            },
        };
        assert!(!missing_entry.has_resolved_provider_state());
        assert!(missing_entry.has_unresolved_provider_state());

        let dup_entry = ValidationEntry {
            declaration: ValidationDeclaration { name: "baz".into(), item_kind: ItemKind::Function },
            status: MatchStatus::DuplicateProviders,
            evidence: ValidationEvidence {
                provider_artifacts: vec!["a.so".into(), "b.so".into()],
                raw_symbol_names: vec!["baz".into()],
                visibility: Some(SymbolVisibility::Default),
                confidence: MatchConfidence::Low,
                evidence_kind: EvidenceKind::DuplicateVisibleProviders,
                abi_shape: None,
                routine_abi: None,
            },
        };
        assert!(dup_entry.has_ambiguous_provider_state());
        assert!(!dup_entry.has_resolved_provider_state());
    }

    #[test]
    fn validation_evidence_layout_backed_detection() {
        let with_abi_shape = ValidationEvidence {
            provider_artifacts: vec!["lib.so".into()],
            raw_symbol_names: vec!["x".into()],
            visibility: Some(SymbolVisibility::Default),
            confidence: MatchConfidence::High,
            evidence_kind: EvidenceKind::AbiShapeVerified,
            abi_shape: Some(AbiShapeEvidence { expected_size: Some(4), observed_size: Some(4) }),
            routine_abi: None,
        };
        assert!(with_abi_shape.has_layout_backed_confidence());

        let with_routine_sizes = ValidationEvidence {
            provider_artifacts: vec!["lib.so".into()],
            raw_symbol_names: vec!["f".into()],
            visibility: Some(SymbolVisibility::Default),
            confidence: MatchConfidence::High,
            evidence_kind: EvidenceKind::ExactExported,
            abi_shape: None,
            routine_abi: Some(RoutineAbiEvidence {
                evidence_kind: Some(RoutineAbiEvidenceKind::FullyShaped),
                confidence: Some(RoutineAbiConfidence::Strong),
                expected_parameter_count: Some(2),
                observed_parameter_count: Some(2),
                expected_return_size: Some(4),
                observed_return_size: Some(4),
                expected_parameter_sizes: vec![Some(4), Some(8)],
                observed_parameter_sizes: vec![Some(4), Some(8)],
            }),
        };
        assert!(with_routine_sizes.has_layout_backed_confidence());

        let no_layout = ValidationEvidence {
            provider_artifacts: vec!["lib.so".into()],
            raw_symbol_names: vec!["g".into()],
            visibility: Some(SymbolVisibility::Default),
            confidence: MatchConfidence::High,
            evidence_kind: EvidenceKind::ExactExported,
            abi_shape: None,
            routine_abi: None,
        };
        assert!(!no_layout.has_layout_backed_confidence());
    }

    #[test]
    fn validation_report_helper_accessors() {
        let pkg = make_package_with_vars(&["alpha", "beta"], &["data"]);
        let inv = make_inventory_with_vis(&[
            ("alpha", true, SymbolVisibility::Default),
            ("beta", true, SymbolVisibility::Hidden),
            ("data", false, SymbolVisibility::Default),
        ]);
        let report = validate(&pkg, &inv);

        assert_eq!(report.matched().len(), 2); // alpha + data
        assert_eq!(report.hidden().len(), 1); // beta hidden
        assert_eq!(report.hidden()[0].name, "beta");
        assert_eq!(report.resolved_provider_entries().len(), 2); // alpha + data
    }

    #[test]
    fn validate_many_entries_cover_mixed_states() {
        let pkg = make_package_with_vars(&["found_func", "missing_func"], &["found_var"]);
        let inv1 = make_inventory(&["found_func"], &[]);
        let inv2 = SymbolInventory {
            artifact_path: "data.o".into(),
            format: ArtifactFormat::ElfObject,
            platform: ArtifactPlatform::Elf,
            kind: ArtifactKind::Object,
            capabilities: ArtifactCapabilities { exports_symbols: true, imports_symbols: false },
            dependency_edges: Vec::new(),
            symbols: vec![SymbolEntry {
                name: "found_var".into(),
                raw_name: None, version: None,
                direction: SymbolDirection::Exported,
                reexported_via: Vec::new(), alias_of: None,
                visibility: SymbolVisibility::Default,
                is_function: false,
                binding: SymbolBinding::Global,
                size: Some(4), section: None, archive_member: None, function_abi: None,
            }],
        };

        let report = validate_many(&pkg, &[inv1, inv2]);

        // Summary should reflect mixed state
        assert_eq!(report.summary.total, 3);
        assert_eq!(report.summary.matched, 2);
        assert_eq!(report.summary.missing, 1);

        // Entries surface should have all 3 declarations
        assert_eq!(report.entries.len(), 3);
        assert_eq!(report.resolved_provider_entries().len(), 2);
        assert_eq!(report.unresolved_provider_entries().len(), 1);

        // Helper methods agree
        assert_eq!(report.matched().len(), 2);
        assert_eq!(report.missing().len(), 1);
        assert_eq!(report.missing()[0].name, "missing_func");
        assert!(!report.all_matched());
    }
}
