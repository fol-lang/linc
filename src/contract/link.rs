use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use super::{
    model::{normalized_absolute_path, validate_native_name},
    ArtifactKind, ContractError, ResolvedArtifact,
};

/// One lossless native-link action. Sequence order and repetition are part of
/// the contract and are never normalized away.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkAtom {
    SearchNative(PathBuf),
    Object(ResolvedArtifact),
    StaticLibrary(ResolvedArtifact),
    DynamicLibrary(ResolvedArtifact),
    ImportLibrary(ResolvedArtifact),
    Framework {
        name: OsString,
        search_path: PathBuf,
        artifact: ResolvedArtifact,
    },
    GroupStart,
    GroupEnd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLinkPlan(Vec<LinkAtom>);

impl ResolvedLinkPlan {
    pub fn try_new(atoms: Vec<LinkAtom>) -> Result<Self, ContractError> {
        let mut depth = 0_usize;
        for (index, atom) in atoms.iter().enumerate() {
            match atom {
                LinkAtom::SearchNative(path) => {
                    normalized_absolute_path("link_atom.search_native", path.clone())?;
                }
                LinkAtom::Object(artifact) => {
                    require_kind("object", ArtifactKind::Object, artifact.kind())?;
                }
                LinkAtom::StaticLibrary(artifact) => {
                    require_kind(
                        "static_library",
                        ArtifactKind::StaticLibrary,
                        artifact.kind(),
                    )?;
                }
                LinkAtom::DynamicLibrary(artifact) => {
                    require_kind(
                        "dynamic_library",
                        ArtifactKind::DynamicLibrary,
                        artifact.kind(),
                    )?;
                }
                LinkAtom::ImportLibrary(artifact) => {
                    require_kind(
                        "import_library",
                        ArtifactKind::ImportLibrary,
                        artifact.kind(),
                    )?;
                }
                LinkAtom::Framework {
                    name,
                    search_path,
                    artifact,
                } => {
                    validate_native_name("link_atom.framework.name", name)?;
                    normalized_absolute_path(
                        "link_atom.framework.search_path",
                        search_path.clone(),
                    )?;
                    require_kind("framework", ArtifactKind::Framework, artifact.kind())?;
                }
                LinkAtom::GroupStart => depth += 1,
                LinkAtom::GroupEnd => {
                    if depth == 0 {
                        return Err(ContractError::UnexpectedGroupEnd { index });
                    }
                    depth -= 1;
                }
            }
        }
        if depth != 0 {
            return Err(ContractError::UnclosedGroups { depth });
        }
        Ok(Self(atoms))
    }

    pub fn atoms(&self) -> &[LinkAtom] {
        &self.0
    }

    pub fn into_atoms(self) -> Vec<LinkAtom> {
        self.0
    }
}

impl LinkAtom {
    pub fn artifact(&self) -> Option<&ResolvedArtifact> {
        match self {
            Self::Object(artifact)
            | Self::StaticLibrary(artifact)
            | Self::DynamicLibrary(artifact)
            | Self::ImportLibrary(artifact)
            | Self::Framework { artifact, .. } => Some(artifact),
            Self::SearchNative(_) | Self::GroupStart | Self::GroupEnd => None,
        }
    }

    pub fn search_path(&self) -> Option<&Path> {
        match self {
            Self::SearchNative(path) => Some(path),
            Self::Framework { search_path, .. } => Some(search_path),
            _ => None,
        }
    }
}

fn require_kind(
    atom_kind: &'static str,
    expected: ArtifactKind,
    actual: ArtifactKind,
) -> Result<(), ContractError> {
    if expected == actual {
        Ok(())
    } else {
        Err(ContractError::ArtifactKindMismatch {
            atom_kind,
            artifact_kind: actual.label(),
        })
    }
}
