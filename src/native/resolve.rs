use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
};

use crate::contract::{
    validate_native_inputs, AnalysisRequest, ArtifactKind, LinkAtom, NativeInput, ProviderId,
    ProviderProvenance, ProviderResolution, ResolutionPolicy, ResolvedLinkPlan, SymbolInventory,
    WeakSymbolPolicy,
};
use parc::contract::{SourceFingerprint, TargetFingerprint};

use super::{
    error::{io_error, NativeError, NativeResult},
    ArtifactInspection, NativeInspector,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryPreference {
    StaticOnly,
    DynamicOnly,
    PreferStatic,
    PreferDynamic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolverConfiguration {
    toolchain_search_paths: Vec<PathBuf>,
    library_preference: LibraryPreference,
    max_transitive_dependencies: usize,
}

impl ResolverConfiguration {
    pub fn new(
        toolchain_search_paths: Vec<PathBuf>,
        library_preference: LibraryPreference,
        max_transitive_dependencies: usize,
    ) -> NativeResult<Self> {
        if max_transitive_dependencies == 0 {
            return Err(NativeError::InvalidPolicy {
                detail: "transitive dependency limit must be nonzero".to_owned(),
            });
        }
        Ok(Self {
            toolchain_search_paths,
            library_preference,
            max_transitive_dependencies,
        })
    }

    pub fn toolchain_search_paths(&self) -> &[PathBuf] {
        &self.toolchain_search_paths
    }

    pub const fn library_preference(&self) -> LibraryPreference {
        self.library_preference
    }

    pub const fn max_transitive_dependencies(&self) -> usize {
        self.max_transitive_dependencies
    }
}

impl Default for ResolverConfiguration {
    fn default() -> Self {
        Self {
            toolchain_search_paths: Vec::new(),
            library_preference: LibraryPreference::DynamicOnly,
            max_transitive_dependencies: 4096,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeResolution {
    source_fingerprint: SourceFingerprint,
    target_fingerprint: TargetFingerprint,
    weak_symbol_policy: WeakSymbolPolicy,
    plan: ResolvedLinkPlan,
    inventories: Vec<SymbolInventory>,
}

impl NativeResolution {
    /// Exact source package against which this resolution was produced.
    pub const fn source_fingerprint(&self) -> SourceFingerprint {
        self.source_fingerprint
    }

    /// Exact requested target against which every provider was inspected.
    pub const fn target_fingerprint(&self) -> TargetFingerprint {
        self.target_fingerprint
    }

    /// Explicit weak-provider policy captured from the analysis request.
    pub const fn weak_symbol_policy(&self) -> WeakSymbolPolicy {
        self.weak_symbol_policy
    }

    pub fn plan(&self) -> &ResolvedLinkPlan {
        &self.plan
    }

    pub fn inventories(&self) -> &[SymbolInventory] {
        &self.inventories
    }

    pub fn into_parts(self) -> (ResolvedLinkPlan, Vec<SymbolInventory>) {
        (self.plan, self.inventories)
    }
}

#[derive(Debug, Clone, Default)]
pub struct NativeResolver {
    inspector: NativeInspector,
    configuration: ResolverConfiguration,
}

impl NativeResolver {
    pub fn new(
        inspector: NativeInspector,
        configuration: ResolverConfiguration,
    ) -> NativeResult<Self> {
        if configuration.max_transitive_dependencies == 0 {
            return Err(NativeError::InvalidPolicy {
                detail: "transitive dependency limit must be nonzero".to_owned(),
            });
        }
        Ok(Self {
            inspector,
            configuration,
        })
    }

    pub fn inspector(&self) -> &NativeInspector {
        &self.inspector
    }

    pub fn configuration(&self) -> &ResolverConfiguration {
        &self.configuration
    }

    /// Resolve one canonical analysis request. Name lookup never consults an
    /// ambient loader or linker configuration.
    pub fn resolve(&self, request: &AnalysisRequest<'_>) -> NativeResult<NativeResolution> {
        validate_native_inputs(request.native_inputs())?;
        let target = request.source().source().target();
        let mut search_paths = Vec::new();
        let mut inspections = Vec::<ArtifactInspection>::new();
        let mut path_to_index = BTreeMap::<PathBuf, usize>::new();
        let mut atoms = Vec::new();

        for (input_index, input) in request.native_inputs().iter().enumerate() {
            match input {
                NativeInput::SearchNative(path) => {
                    let path = canonical_directory(path)?;
                    push_distinct(&mut search_paths, path.clone());
                    atoms.push(LinkAtom::SearchNative(path));
                }
                NativeInput::ObjectPath(path) => {
                    let inspection = self.inspect_or_reuse(
                        path,
                        ArtifactKind::Object,
                        ProviderResolution::Explicit,
                        ProviderProvenance::User,
                        target,
                        &mut inspections,
                        &mut path_to_index,
                    )?;
                    atoms.push(LinkAtom::Object(inspection.artifact().clone()));
                }
                NativeInput::StaticLibraryPath(path) => {
                    let inspection = self.inspect_or_reuse(
                        path,
                        ArtifactKind::StaticLibrary,
                        ProviderResolution::Explicit,
                        ProviderProvenance::User,
                        target,
                        &mut inspections,
                        &mut path_to_index,
                    )?;
                    atoms.push(LinkAtom::StaticLibrary(inspection.artifact().clone()));
                }
                NativeInput::DynamicLibraryPath(path) => {
                    let inspection = self.inspect_or_reuse(
                        path,
                        ArtifactKind::DynamicLibrary,
                        ProviderResolution::Explicit,
                        ProviderProvenance::User,
                        target,
                        &mut inspections,
                        &mut path_to_index,
                    )?;
                    atoms.push(LinkAtom::DynamicLibrary(inspection.artifact().clone()));
                }
                NativeInput::StaticLibraryName(name) => {
                    let paths =
                        self.effective_search_paths(request.policy().resolution(), &search_paths)?;
                    let filename = static_filename(name)?;
                    let path = unique_candidate(&filename, &paths)?;
                    let inspection = self.inspect_or_reuse(
                        &path,
                        ArtifactKind::StaticLibrary,
                        ProviderResolution::SearchPath {
                            native_input_index: u32::try_from(input_index).map_err(|_| {
                                NativeError::InvalidPolicy {
                                    detail: "native input index exceeds the contract".to_owned(),
                                }
                            })?,
                        },
                        ProviderProvenance::User,
                        target,
                        &mut inspections,
                        &mut path_to_index,
                    )?;
                    atoms.push(LinkAtom::StaticLibrary(inspection.artifact().clone()));
                }
                NativeInput::DynamicLibraryName(name) => {
                    let paths =
                        self.effective_search_paths(request.policy().resolution(), &search_paths)?;
                    let filename = dynamic_filename(name)?;
                    let path = unique_candidate(&filename, &paths)?;
                    let inspection = self.inspect_or_reuse(
                        &path,
                        ArtifactKind::DynamicLibrary,
                        ProviderResolution::SearchPath {
                            native_input_index: u32::try_from(input_index).map_err(|_| {
                                NativeError::InvalidPolicy {
                                    detail: "native input index exceeds the contract".to_owned(),
                                }
                            })?,
                        },
                        ProviderProvenance::User,
                        target,
                        &mut inspections,
                        &mut path_to_index,
                    )?;
                    atoms.push(LinkAtom::DynamicLibrary(inspection.artifact().clone()));
                }
                NativeInput::ImportLibraryPath(_) | NativeInput::ImportLibraryName(_) => {
                    return Err(NativeError::UnsupportedInput {
                        detail: "ELF certification does not accept import libraries".to_owned(),
                    });
                }
                NativeInput::FrameworkPath(path) => {
                    let inspection = self.inspect_or_reuse(
                        path,
                        ArtifactKind::Framework,
                        ProviderResolution::Explicit,
                        ProviderProvenance::User,
                        target,
                        &mut inspections,
                        &mut path_to_index,
                    )?;
                    let name = framework_name_from_path(path)?;
                    let search_path = framework_search_root(path)?;
                    atoms.push(LinkAtom::Framework {
                        name,
                        search_path,
                        artifact: inspection.artifact().clone(),
                    });
                }
                NativeInput::FrameworkName { name, search_path } => {
                    let roots = if let Some(path) = search_path {
                        vec![canonical_directory(path)?]
                    } else {
                        self.effective_search_paths(request.policy().resolution(), &search_paths)?
                    };
                    let relative = framework_relative_path(name);
                    let path = unique_candidate(relative.as_os_str(), &roots)?;
                    let inspection = self.inspect_or_reuse(
                        &path,
                        ArtifactKind::Framework,
                        ProviderResolution::SearchPath {
                            native_input_index: u32::try_from(input_index).map_err(|_| {
                                NativeError::InvalidPolicy {
                                    detail: "native input index exceeds the contract".to_owned(),
                                }
                            })?,
                        },
                        ProviderProvenance::User,
                        target,
                        &mut inspections,
                        &mut path_to_index,
                    )?;
                    let root = path
                        .parent()
                        .and_then(Path::parent)
                        .ok_or_else(|| NativeError::UnsupportedInput {
                            detail: "framework path has no search root".to_owned(),
                        })?
                        .to_path_buf();
                    atoms.push(LinkAtom::Framework {
                        name: name.clone(),
                        search_path: root,
                        artifact: inspection.artifact().clone(),
                    });
                }
                NativeInput::GroupStart => atoms.push(LinkAtom::GroupStart),
                NativeInput::GroupEnd => atoms.push(LinkAtom::GroupEnd),
            }
        }

        let mut dependency_bindings = Vec::<Vec<Option<ProviderId>>>::new();
        let mut cursor = 0_usize;
        let mut transitive_count = 0_usize;
        while cursor < inspections.len() {
            if dependency_bindings.len() <= cursor {
                dependency_bindings.resize_with(cursor + 1, Vec::new);
            }
            let parent = inspections[cursor].artifact().provider_id();
            let dependencies = inspections[cursor].dependencies().to_vec();
            let mut bindings = Vec::with_capacity(dependencies.len());
            for dependency in dependencies {
                let existing = inspections
                    .iter()
                    .enumerate()
                    .filter(|(_, inspection)| inspection.provides(dependency.name()))
                    .map(|(index, _)| index)
                    .collect::<Vec<_>>();
                let child_index = match existing.as_slice() {
                    [index] => *index,
                    [_, _, ..] => {
                        return Err(NativeError::AmbiguousProvider {
                            requested: dependency.name().to_os_string(),
                            candidates: existing
                                .iter()
                                .map(|index| {
                                    inspections[*index]
                                        .artifact()
                                        .canonical_path()
                                        .to_path_buf()
                                })
                                .collect(),
                        });
                    }
                    [] if request.policy().resolution() == ResolutionPolicy::ExactPathsOnly => {
                        return Err(NativeError::MissingProvider {
                            requested: dependency.name().to_os_string(),
                        });
                    }
                    [] => {
                        let dependency_search_paths = self
                            .effective_search_paths(request.policy().resolution(), &search_paths)?;
                        let filenames = dependency_filenames(
                            dependency.name(),
                            self.configuration.library_preference,
                        )?;
                        let path =
                            unique_candidate_by_priority(&filenames, &dependency_search_paths)?;
                        let canonical = fs::canonicalize(&path)
                            .map_err(|error| io_error("canonicalize dependency", &path, error))?;
                        if let Some(index) = path_to_index.get(&canonical).copied() {
                            index
                        } else {
                            transitive_count =
                                transitive_count.checked_add(1).ok_or_else(|| {
                                    NativeError::InvalidPolicy {
                                        detail: "transitive dependency count overflow".to_owned(),
                                    }
                                })?;
                            if transitive_count > self.configuration.max_transitive_dependencies {
                                return Err(NativeError::InvalidPolicy {
                                    detail: format!(
                                        "resolution exceeds {} transitive providers",
                                        self.configuration.max_transitive_dependencies
                                    ),
                                });
                            }
                            let inspection = self.inspector.inspect(
                                &canonical,
                                ArtifactKind::DynamicLibrary,
                                ProviderResolution::Dependency { parent },
                                ProviderProvenance::Transitive,
                                target,
                            )?;
                            let index = inspections.len();
                            path_to_index.insert(
                                inspection.artifact().canonical_path().to_path_buf(),
                                index,
                            );
                            atoms.push(LinkAtom::DynamicLibrary(inspection.artifact().clone()));
                            inspections.push(inspection);
                            index
                        }
                    }
                };
                bindings.push(Some(inspections[child_index].artifact().provider_id()));
            }
            dependency_bindings[cursor] = bindings;
            cursor += 1;
        }

        ensure_acyclic(&inspections, &dependency_bindings)?;
        ensure_dependency_plan_order(&atoms, &inspections, &dependency_bindings)?;
        let mut inventories = inspections
            .iter()
            .enumerate()
            .map(|(index, inspection)| {
                let bindings = dependency_bindings
                    .get(index)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                inspection.inventory_with_dependencies(bindings)
            })
            .collect::<NativeResult<Vec<_>>>()?;
        inventories.sort_by_key(|inventory| inventory.artifact().provider_id());
        Ok(NativeResolution {
            source_fingerprint: request.source().source().fingerprint(),
            target_fingerprint: request.source().source().target_fingerprint(),
            weak_symbol_policy: request.policy().weak_symbols(),
            plan: ResolvedLinkPlan::try_new(atoms)?,
            inventories,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn inspect_or_reuse<'a>(
        &self,
        path: &Path,
        kind: ArtifactKind,
        resolution: ProviderResolution,
        provenance: ProviderProvenance,
        target: &parc::contract::TargetSpec,
        inspections: &'a mut Vec<ArtifactInspection>,
        path_to_index: &mut BTreeMap<PathBuf, usize>,
    ) -> NativeResult<&'a ArtifactInspection> {
        let canonical = fs::canonicalize(path)
            .map_err(|error| io_error("canonicalize provider", path, error))?;
        if let Some(index) = path_to_index.get(&canonical).copied() {
            let existing = &mut inspections[index];
            if existing.artifact().kind() != kind {
                return Err(NativeError::ArtifactKindMismatch {
                    path: canonical,
                    expected: kind_label(kind),
                    observed: kind_label(existing.artifact().kind()),
                });
            }
            existing.add_provided_name(path.file_name());
            return Ok(&*existing);
        }
        let mut inspection = self
            .inspector
            .inspect(&canonical, kind, resolution, provenance, target)?;
        inspection.add_provided_name(path.file_name());
        let index = inspections.len();
        path_to_index.insert(inspection.artifact().canonical_path().to_path_buf(), index);
        inspections.push(inspection);
        Ok(&inspections[index])
    }

    fn effective_search_paths(
        &self,
        policy: ResolutionPolicy,
        declared: &[PathBuf],
    ) -> NativeResult<Vec<PathBuf>> {
        let mut paths = declared.to_vec();
        match policy {
            ResolutionPolicy::ExactPathsOnly => {
                return Err(NativeError::InvalidPolicy {
                    detail: "exact-path policy cannot perform provider lookup".to_owned(),
                });
            }
            ResolutionPolicy::HermeticSearch => {}
            ResolutionPolicy::ToolchainSearch => {
                for path in &self.configuration.toolchain_search_paths {
                    push_distinct(&mut paths, canonical_directory(path)?);
                }
            }
        }
        if paths.is_empty() {
            return Err(NativeError::MissingProvider {
                requested: OsString::from("<no declared search path>"),
            });
        }
        Ok(paths)
    }
}

fn canonical_directory(path: &Path) -> NativeResult<PathBuf> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| io_error("canonicalize search path", path, error))?;
    if !canonical.is_dir() {
        return Err(NativeError::UnsupportedInput {
            detail: format!("search path {} is not a directory", canonical.display()),
        });
    }
    Ok(canonical)
}

fn push_distinct(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.contains(&path) {
        paths.push(path);
    }
}

fn unique_candidate(filename: &OsStr, search_paths: &[PathBuf]) -> NativeResult<PathBuf> {
    unique_candidate_by_priority(&[filename.to_os_string()], search_paths)
}

fn unique_candidate_by_priority(
    filenames: &[OsString],
    search_paths: &[PathBuf],
) -> NativeResult<PathBuf> {
    let requested = filenames.to_vec();
    for filename in filenames {
        let mut candidates = Vec::new();
        for directory in search_paths {
            let candidate = directory.join(filename);
            if !candidate.is_file() {
                continue;
            }
            let canonical = fs::canonicalize(&candidate)
                .map_err(|error| io_error("canonicalize provider candidate", &candidate, error))?;
            if !candidates.contains(&canonical) {
                candidates.push(canonical);
            }
        }
        match candidates.len() {
            0 => {}
            1 => return Ok(candidates.remove(0)),
            _ => {
                return Err(NativeError::AmbiguousProvider {
                    requested: requested[0].clone(),
                    candidates,
                });
            }
        }
    }
    Err(NativeError::MissingProvider {
        requested: requested
            .first()
            .cloned()
            .unwrap_or_else(|| OsString::from("<empty candidate set>")),
    })
}

fn static_filename(name: &OsStr) -> NativeResult<OsString> {
    let text = name.to_str().ok_or_else(|| NativeError::UnsupportedInput {
        detail: "ELF library names must be UTF-8; non-UTF8 input is never normalized lossily"
            .to_owned(),
    })?;
    if text.len() > 4096 || text.contains(".so") {
        return Err(NativeError::UnsupportedInput {
            detail: "static-library name is oversized or carries a dynamic-library suffix"
                .to_owned(),
        });
    }
    let stem = text.strip_prefix("lib").unwrap_or(text);
    let stem = stem.strip_suffix(".a").unwrap_or(stem);
    Ok(OsString::from(format!("lib{stem}.a")))
}

fn dynamic_filename(name: &OsStr) -> NativeResult<OsString> {
    let text = name.to_str().ok_or_else(|| NativeError::UnsupportedInput {
        detail: "ELF library names must be UTF-8; non-UTF8 input is never normalized lossily"
            .to_owned(),
    })?;
    if text.len() > 4096 || text.ends_with(".a") {
        return Err(NativeError::UnsupportedInput {
            detail: "dynamic-library name is oversized or carries a static-library suffix"
                .to_owned(),
        });
    }
    let stem = text.strip_prefix("lib").unwrap_or(text);
    if stem.ends_with(".so") || stem.contains(".so.") {
        Ok(OsString::from(format!("lib{stem}")))
    } else {
        Ok(OsString::from(format!("lib{stem}.so")))
    }
}

fn dependency_filenames(
    name: &OsStr,
    preference: LibraryPreference,
) -> NativeResult<Vec<OsString>> {
    let text = name.to_str().ok_or_else(|| NativeError::UnsupportedInput {
        detail: "ELF dependency names must be UTF-8; non-UTF8 input is never normalized lossily"
            .to_owned(),
    })?;
    if text.ends_with(".a") || text.ends_with(".so") || text.contains(".so.") {
        return Ok(vec![name.to_os_string()]);
    }
    let static_name = static_filename(name)?;
    let dynamic_name = dynamic_filename(name)?;
    Ok(match preference {
        LibraryPreference::StaticOnly => vec![static_name],
        LibraryPreference::DynamicOnly => vec![dynamic_name],
        LibraryPreference::PreferStatic => vec![static_name, dynamic_name],
        LibraryPreference::PreferDynamic => vec![dynamic_name, static_name],
    })
}

fn framework_relative_path(name: &OsStr) -> PathBuf {
    let mut path = PathBuf::from(name);
    path.set_extension("framework");
    path.push(name);
    path
}

fn framework_name_from_path(path: &Path) -> NativeResult<OsString> {
    path.file_name()
        .map(OsStr::to_os_string)
        .ok_or_else(|| NativeError::UnsupportedInput {
            detail: "framework binary has no filename".to_owned(),
        })
}

fn framework_search_root(path: &Path) -> NativeResult<PathBuf> {
    let canonical =
        fs::canonicalize(path).map_err(|error| io_error("canonicalize framework", path, error))?;
    canonical
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| NativeError::UnsupportedInput {
            detail: "framework binary has no search root".to_owned(),
        })
}

fn ensure_acyclic(
    inspections: &[ArtifactInspection],
    bindings: &[Vec<Option<ProviderId>>],
) -> NativeResult<()> {
    let index_by_provider = inspections
        .iter()
        .enumerate()
        .map(|(index, inspection)| (inspection.artifact().provider_id(), index))
        .collect::<BTreeMap<_, _>>();
    let mut states = vec![0_u8; inspections.len()];
    for index in 0..inspections.len() {
        visit(
            index,
            inspections,
            bindings,
            &index_by_provider,
            &mut states,
        )?;
    }
    Ok(())
}

fn visit(
    index: usize,
    inspections: &[ArtifactInspection],
    bindings: &[Vec<Option<ProviderId>>],
    index_by_provider: &BTreeMap<ProviderId, usize>,
    states: &mut [u8],
) -> NativeResult<()> {
    match states[index] {
        2 => return Ok(()),
        1 => {
            return Err(NativeError::DependencyCycle {
                path: inspections[index].artifact().canonical_path().to_path_buf(),
            });
        }
        _ => {}
    }
    states[index] = 1;
    for provider in bindings.get(index).into_iter().flatten().flatten() {
        let child =
            index_by_provider
                .get(provider)
                .copied()
                .ok_or_else(|| NativeError::InvalidPolicy {
                    detail: format!("dependency provider {provider} has no inspection"),
                })?;
        visit(child, inspections, bindings, index_by_provider, states)?;
    }
    states[index] = 2;
    Ok(())
}

fn ensure_dependency_plan_order(
    atoms: &[LinkAtom],
    inspections: &[ArtifactInspection],
    bindings: &[Vec<Option<ProviderId>>],
) -> NativeResult<()> {
    let positions = atoms
        .iter()
        .enumerate()
        .filter_map(|(index, atom)| {
            atom.artifact()
                .map(|artifact| (artifact.provider_id(), index))
        })
        .fold(BTreeMap::new(), |mut positions, (provider, index)| {
            positions.entry(provider).or_insert(index);
            positions
        });
    for (index, inspection) in inspections.iter().enumerate() {
        let parent = inspection.artifact().provider_id();
        let parent_position =
            positions
                .get(&parent)
                .copied()
                .ok_or_else(|| NativeError::InvalidPolicy {
                    detail: format!("provider {parent} is absent from the resolved plan"),
                })?;
        for child in bindings.get(index).into_iter().flatten().flatten() {
            let child_position =
                positions
                    .get(child)
                    .copied()
                    .ok_or_else(|| NativeError::InvalidPolicy {
                        detail: format!("dependency {child} is absent from the resolved plan"),
                    })?;
            if parent_position >= child_position {
                return Err(NativeError::InvalidPolicy {
                    detail: format!(
                        "dependency provider {child} must follow parent {parent} without reordering explicit inputs"
                    ),
                });
            }
        }
    }
    Ok(())
}

fn kind_label(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Object => "object",
        ArtifactKind::StaticLibrary => "static_library",
        ArtifactKind::DynamicLibrary => "dynamic_library",
        ArtifactKind::ImportLibrary => "import_library",
        ArtifactKind::Framework => "framework",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    use std::os::unix::ffi::OsStringExt;

    #[test]
    fn exact_library_names_and_preference_order_are_deterministic() {
        assert_eq!(static_filename(OsStr::new("one")).unwrap(), "libone.a");
        assert_eq!(static_filename(OsStr::new("one.a")).unwrap(), "libone.a");
        assert_eq!(dynamic_filename(OsStr::new("one")).unwrap(), "libone.so");
        assert_eq!(
            dynamic_filename(OsStr::new("one.so.2")).unwrap(),
            "libone.so.2"
        );
        assert_eq!(
            dependency_filenames(OsStr::new("one"), LibraryPreference::PreferDynamic).unwrap(),
            [OsString::from("libone.so"), OsString::from("libone.a")]
        );
        assert_eq!(
            dependency_filenames(OsStr::new("libone.so.2"), LibraryPreference::StaticOnly).unwrap(),
            [OsString::from("libone.so.2")]
        );
        assert_eq!(
            dependency_filenames(OsStr::new("one.so.2"), LibraryPreference::StaticOnly).unwrap(),
            [OsString::from("one.so.2")]
        );

        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("libone.a"), b"archive").unwrap();
        fs::write(root.path().join("libone.so"), b"dynamic").unwrap();
        let names =
            dependency_filenames(OsStr::new("one"), LibraryPreference::PreferDynamic).unwrap();
        assert_eq!(
            unique_candidate_by_priority(&names, &[root.path().to_path_buf()]).unwrap(),
            fs::canonicalize(root.path().join("libone.so")).unwrap(),
            "preference selects a kind before ambiguity is considered"
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_library_names_are_rejected_without_lossy_normalization() {
        let name = OsString::from_vec(vec![b'o', b'n', b'e', 0xff]);
        assert!(matches!(
            static_filename(&name),
            Err(NativeError::UnsupportedInput { .. })
        ));
        assert!(matches!(
            dynamic_filename(&name),
            Err(NativeError::UnsupportedInput { .. })
        ));
        assert!(matches!(
            dependency_filenames(&name, LibraryPreference::DynamicOnly),
            Err(NativeError::UnsupportedInput { .. })
        ));

        let error = unique_candidate_by_priority(std::slice::from_ref(&name), &[]).unwrap_err();
        assert!(matches!(
            error,
            NativeError::MissingProvider { requested } if requested == name
        ));
    }
}
