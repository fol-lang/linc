use std::{collections::BTreeSet, ffi::OsString, path::PathBuf};

use parc::contract::{
    CallingConvention, ChildId, CompilerIdentity, ContentFingerprint, DeclarationId, ExactInteger,
    NormalizedCompilerArg, Signedness, SourceFingerprint, TargetFingerprint,
};
use serde::{Deserialize, Serialize};

use super::{
    model::{native_has_nul, native_units, normalized_absolute_path, validate_text},
    ArtifactFingerprint, ArtifactSymbolId, ContractError, LincCode, ProbeEvidenceId, ProviderId,
    SymbolDecoration, SymbolKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProbeSubject {
    RecordLayout { declaration: DeclarationId },
    EnumRepresentation { declaration: DeclarationId },
    CallableAbi { declaration: DeclarationId },
}

impl ProbeSubject {
    pub const fn declaration(self) -> DeclarationId {
        match self {
            Self::RecordLayout { declaration }
            | Self::EnumRepresentation { declaration }
            | Self::CallableAbi { declaration } => declaration,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::RecordLayout { .. } => "record_layout",
            Self::EnumRepresentation { .. } => "enum_representation",
            Self::CallableAbi { .. } => "callable_abi",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeMethod {
    CompilerLayoutDump,
    CompileTimeAssertion,
    ExecutedHarness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeEnvironmentPolicy {
    Empty,
    Explicit,
    InheritAllowlisted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProbeEnvironmentValue {
    Unset,
    Set {
        value_fingerprint: ContentFingerprint,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeEnvironmentEntry {
    name: String,
    value: ProbeEnvironmentValue,
}

impl ProbeEnvironmentEntry {
    pub fn try_new(name: String, value: ProbeEnvironmentValue) -> Result<Self, ContractError> {
        validate_text("probe.environment.name", &name)?;
        if name.contains('=') {
            return Err(ContractError::InvalidProbeEnvironment {
                reason: "environment variable names cannot contain '='",
            });
        }
        Ok(Self { name, value })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn value(&self) -> ProbeEnvironmentValue {
        self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeEnvironmentIdentity {
    policy: ProbeEnvironmentPolicy,
    entries: Vec<ProbeEnvironmentEntry>,
    fingerprint: ContentFingerprint,
}

impl ProbeEnvironmentIdentity {
    pub fn try_new(
        policy: ProbeEnvironmentPolicy,
        mut entries: Vec<ProbeEnvironmentEntry>,
    ) -> Result<Self, ContractError> {
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        if entries.windows(2).any(|pair| pair[0].name == pair[1].name) {
            return Err(ContractError::InvalidProbeEnvironment {
                reason: "environment variable entries must be unique",
            });
        }
        match policy {
            ProbeEnvironmentPolicy::Empty if !entries.is_empty() => {
                return Err(ContractError::InvalidProbeEnvironment {
                    reason: "empty environment policy cannot carry entries",
                });
            }
            ProbeEnvironmentPolicy::Explicit | ProbeEnvironmentPolicy::InheritAllowlisted
                if entries.is_empty() =>
            {
                return Err(ContractError::InvalidProbeEnvironment {
                    reason: "explicit environment policies require a captured entry set",
                });
            }
            _ => {}
        }
        let fingerprint = derive_environment_fingerprint(policy, &entries)?;
        Ok(Self {
            policy,
            entries,
            fingerprint,
        })
    }

    pub(crate) fn try_from_stored(
        policy: ProbeEnvironmentPolicy,
        entries: Vec<ProbeEnvironmentEntry>,
        stored: ContentFingerprint,
    ) -> Result<Self, ContractError> {
        let identity = Self::try_new(policy, entries)?;
        if identity.fingerprint != stored {
            return Err(ContractError::ProbeEnvironmentFingerprintMismatch {
                stored,
                derived: identity.fingerprint,
            });
        }
        Ok(identity)
    }

    pub const fn policy(&self) -> ProbeEnvironmentPolicy {
        self.policy
    }

    pub const fn fingerprint(&self) -> ContentFingerprint {
        self.fingerprint
    }

    pub fn entries(&self) -> &[ProbeEnvironmentEntry] {
        &self.entries
    }
}

fn derive_environment_fingerprint(
    policy: ProbeEnvironmentPolicy,
    entries: &[ProbeEnvironmentEntry],
) -> Result<ContentFingerprint, ContractError> {
    #[derive(Serialize)]
    struct EnvironmentFingerprintPayload<'a> {
        domain: &'static str,
        policy: ProbeEnvironmentPolicy,
        entries: &'a [ProbeEnvironmentEntry],
    }
    serde_json::to_vec(&EnvironmentFingerprintPayload {
        domain: "follang.linc.probe-environment.v1",
        policy,
        entries,
    })
    .map(|bytes| ContentFingerprint::from_content(&bytes))
    .map_err(canonical_error)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeResourceLimits {
    wall_time_millis: u64,
    max_memory_bytes: u64,
    max_output_bytes: u64,
    max_processes: u32,
}

impl ProbeResourceLimits {
    pub fn try_new(
        wall_time_millis: u64,
        max_memory_bytes: u64,
        max_output_bytes: u64,
        max_processes: u32,
    ) -> Result<Self, ContractError> {
        if wall_time_millis == 0
            || max_memory_bytes == 0
            || max_output_bytes == 0
            || max_processes == 0
        {
            return Err(ContractError::InvalidPolicy {
                reason: "probe resource limits must all be nonzero",
            });
        }
        Ok(Self {
            wall_time_millis,
            max_memory_bytes,
            max_output_bytes,
            max_processes,
        })
    }

    pub const fn wall_time_millis(&self) -> u64 {
        self.wall_time_millis
    }

    pub const fn max_memory_bytes(&self) -> u64 {
        self.max_memory_bytes
    }

    pub const fn max_output_bytes(&self) -> u64 {
        self.max_output_bytes
    }

    pub const fn max_processes(&self) -> u32 {
        self.max_processes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeExecutionPolicy {
    temporary_parent: PathBuf,
    environment: ProbeEnvironmentIdentity,
    limits: ProbeResourceLimits,
}

impl ProbeExecutionPolicy {
    pub fn try_new(
        temporary_parent: PathBuf,
        environment: ProbeEnvironmentIdentity,
        limits: ProbeResourceLimits,
    ) -> Result<Self, ContractError> {
        let temporary_parent =
            normalized_absolute_path("probe.temporary_parent", temporary_parent)?;
        if temporary_parent.parent().is_none() {
            return Err(ContractError::InvalidPolicy {
                reason: "probe temporary parent cannot be the filesystem root",
            });
        }
        Ok(Self {
            temporary_parent,
            environment,
            limits,
        })
    }

    pub fn temporary_parent(&self) -> &std::path::Path {
        &self.temporary_parent
    }

    pub const fn environment(&self) -> &ProbeEnvironmentIdentity {
        &self.environment
    }

    pub const fn limits(&self) -> ProbeResourceLimits {
        self.limits
    }

    pub(crate) fn from_checked_parts(
        temporary_parent: PathBuf,
        environment: ProbeEnvironmentIdentity,
        limits: ProbeResourceLimits,
    ) -> Result<Self, ContractError> {
        Self::try_new(temporary_parent, environment, limits)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProbeProcessStatus {
    Exited { code: i32 },
    Signaled { signal: i32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeProcessResult {
    status: ProbeProcessStatus,
    stdout_fingerprint: ContentFingerprint,
    stderr_fingerprint: ContentFingerprint,
    output_artifact: Option<ArtifactFingerprint>,
}

impl ProbeProcessResult {
    pub const fn new(
        status: ProbeProcessStatus,
        stdout_fingerprint: ContentFingerprint,
        stderr_fingerprint: ContentFingerprint,
        output_artifact: Option<ArtifactFingerprint>,
    ) -> Self {
        Self {
            status,
            stdout_fingerprint,
            stderr_fingerprint,
            output_artifact,
        }
    }

    pub const fn status(&self) -> ProbeProcessStatus {
        self.status
    }

    pub const fn stdout_fingerprint(&self) -> ContentFingerprint {
        self.stdout_fingerprint
    }

    pub const fn stderr_fingerprint(&self) -> ContentFingerprint {
        self.stderr_fingerprint
    }

    pub const fn output_artifact(&self) -> Option<ArtifactFingerprint> {
        self.output_artifact
    }

    pub const fn succeeded(&self) -> bool {
        matches!(self.status, ProbeProcessStatus::Exited { code: 0 })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProbeSubjectStatus {
    Verified {
        evidence_fingerprint: ContentFingerprint,
    },
    Rejected {
        code: LincCode,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeSubjectOutcome {
    subject: ProbeSubject,
    status: ProbeSubjectStatus,
}

impl ProbeSubjectOutcome {
    pub fn try_new(
        subject: ProbeSubject,
        status: ProbeSubjectStatus,
    ) -> Result<Self, ContractError> {
        let outcome = Self { subject, status };
        validate_probe_subject_outcome(&outcome)?;
        Ok(outcome)
    }

    pub const fn subject(&self) -> ProbeSubject {
        self.subject
    }

    pub const fn status(&self) -> &ProbeSubjectStatus {
        &self.status
    }

    pub const fn verified(&self) -> bool {
        matches!(self.status, ProbeSubjectStatus::Verified { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeCompilerArgument {
    Literal(OsString),
    ProbeSource,
    OutputArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeRunnerArgument {
    Literal(OsString),
    ProbeExecutable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeRunnerEvidence {
    NotExecuted,
    Executed {
        executable_path: PathBuf,
        executable_fingerprint: ArtifactFingerprint,
        arguments: Vec<ProbeRunnerArgument>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiProbeEvidenceInput {
    pub source_fingerprint: SourceFingerprint,
    pub target_fingerprint: TargetFingerprint,
    pub compiler: CompilerIdentity,
    pub compiler_executable: PathBuf,
    /// Exact compiler arguments after the executable, in invocation order and with native
    /// units and repetition preserved.
    pub compiler_arguments: Vec<ProbeCompilerArgument>,
    /// ABI-affecting compiler arguments in invocation order, with repetition.
    pub abi_flags: Vec<NormalizedCompilerArg>,
    pub probe_source_fingerprint: ContentFingerprint,
    pub subjects: Vec<ProbeSubject>,
    pub method: ProbeMethod,
    pub execution_policy: ProbeExecutionPolicy,
    pub compile_result: ProbeProcessResult,
    pub runner: ProbeRunnerEvidence,
    pub execution_result: Option<ProbeProcessResult>,
    pub subject_outcomes: Vec<ProbeSubjectOutcome>,
}

/// Immutable provenance for measured layout or callable-ABI evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiProbeEvidence {
    id: ProbeEvidenceId,
    source_fingerprint: SourceFingerprint,
    target_fingerprint: TargetFingerprint,
    compiler: CompilerIdentity,
    compiler_executable: PathBuf,
    compiler_arguments: Vec<ProbeCompilerArgument>,
    abi_flags: Vec<NormalizedCompilerArg>,
    probe_source_fingerprint: ContentFingerprint,
    subjects: Vec<ProbeSubject>,
    method: ProbeMethod,
    execution_policy: ProbeExecutionPolicy,
    compile_result: ProbeProcessResult,
    runner: ProbeRunnerEvidence,
    execution_result: Option<ProbeProcessResult>,
    subject_outcomes: Vec<ProbeSubjectOutcome>,
}

impl AbiProbeEvidence {
    pub fn try_new(mut input: AbiProbeEvidenceInput) -> Result<Self, ContractError> {
        canonicalize_probe_subjects(&mut input.subjects)?;
        canonicalize_subject_outcomes(&input.subjects, &mut input.subject_outcomes)?;
        validate_probe_invocation(&input)?;
        let id = derive_probe_id(&input)?;
        Ok(Self {
            id,
            source_fingerprint: input.source_fingerprint,
            target_fingerprint: input.target_fingerprint,
            compiler: input.compiler,
            compiler_executable: input.compiler_executable,
            compiler_arguments: input.compiler_arguments,
            abi_flags: input.abi_flags,
            probe_source_fingerprint: input.probe_source_fingerprint,
            subjects: input.subjects,
            method: input.method,
            execution_policy: input.execution_policy,
            compile_result: input.compile_result,
            runner: input.runner,
            execution_result: input.execution_result,
            subject_outcomes: input.subject_outcomes,
        })
    }

    pub(crate) fn try_from_stored(
        stored: ProbeEvidenceId,
        input: AbiProbeEvidenceInput,
    ) -> Result<Self, ContractError> {
        let evidence = Self::try_new(input)?;
        if evidence.id != stored {
            return Err(ContractError::ProbeIdMismatch {
                stored,
                derived: evidence.id,
            });
        }
        Ok(evidence)
    }

    pub const fn id(&self) -> ProbeEvidenceId {
        self.id
    }

    pub const fn source_fingerprint(&self) -> SourceFingerprint {
        self.source_fingerprint
    }

    pub const fn target_fingerprint(&self) -> TargetFingerprint {
        self.target_fingerprint
    }

    pub fn compiler(&self) -> &CompilerIdentity {
        &self.compiler
    }

    pub fn compiler_executable(&self) -> &std::path::Path {
        &self.compiler_executable
    }

    pub fn compiler_arguments(&self) -> &[ProbeCompilerArgument] {
        &self.compiler_arguments
    }

    pub fn abi_flags(&self) -> &[NormalizedCompilerArg] {
        &self.abi_flags
    }

    pub const fn probe_source_fingerprint(&self) -> ContentFingerprint {
        self.probe_source_fingerprint
    }

    pub fn subjects(&self) -> &[ProbeSubject] {
        &self.subjects
    }

    pub const fn method(&self) -> ProbeMethod {
        self.method
    }

    pub const fn execution_policy(&self) -> &ProbeExecutionPolicy {
        &self.execution_policy
    }

    pub const fn compile_result(&self) -> &ProbeProcessResult {
        &self.compile_result
    }

    pub fn runner(&self) -> &ProbeRunnerEvidence {
        &self.runner
    }

    pub fn execution_result(&self) -> Option<&ProbeProcessResult> {
        self.execution_result.as_ref()
    }

    pub fn subject_outcomes(&self) -> &[ProbeSubjectOutcome] {
        &self.subject_outcomes
    }

    pub fn verified(&self, subject: ProbeSubject) -> bool {
        self.subject_outcomes
            .binary_search_by_key(&subject, ProbeSubjectOutcome::subject)
            .is_ok_and(|index| self.subject_outcomes[index].verified())
    }

    pub fn supports(&self, subject: ProbeSubject) -> bool {
        self.subjects.binary_search(&subject).is_ok()
    }

    pub(crate) fn input(&self) -> AbiProbeEvidenceInput {
        AbiProbeEvidenceInput {
            source_fingerprint: self.source_fingerprint,
            target_fingerprint: self.target_fingerprint,
            compiler: self.compiler.clone(),
            compiler_executable: self.compiler_executable.clone(),
            compiler_arguments: self.compiler_arguments.clone(),
            abi_flags: self.abi_flags.clone(),
            probe_source_fingerprint: self.probe_source_fingerprint,
            subjects: self.subjects.clone(),
            method: self.method,
            execution_policy: self.execution_policy.clone(),
            compile_result: self.compile_result.clone(),
            runner: self.runner.clone(),
            execution_result: self.execution_result.clone(),
            subject_outcomes: self.subject_outcomes.clone(),
        }
    }
}

fn canonicalize_probe_subjects(subjects: &mut [ProbeSubject]) -> Result<(), ContractError> {
    if subjects.is_empty() {
        return Err(ContractError::EmptyProbeSubjects);
    }
    subjects.sort_unstable();
    for pair in subjects.windows(2) {
        if pair[0] == pair[1] {
            return Err(ContractError::DuplicateProbeSubject {
                subject: format!("{}:{}", pair[0].label(), pair[0].declaration()),
            });
        }
    }
    Ok(())
}

fn canonicalize_subject_outcomes(
    subjects: &[ProbeSubject],
    outcomes: &mut [ProbeSubjectOutcome],
) -> Result<(), ContractError> {
    for outcome in outcomes.iter() {
        validate_probe_subject_outcome(outcome)?;
    }
    outcomes.sort_by_key(ProbeSubjectOutcome::subject);
    if outcomes.len() != subjects.len()
        || outcomes
            .iter()
            .map(ProbeSubjectOutcome::subject)
            .ne(subjects.iter().copied())
    {
        return Err(ContractError::ProbeSubjectCountMismatch {
            subjects: subjects.len(),
            outcomes: outcomes.len(),
        });
    }
    Ok(())
}

fn validate_probe_subject_outcome(outcome: &ProbeSubjectOutcome) -> Result<(), ContractError> {
    if let ProbeSubjectStatus::Rejected { code } = outcome.status() {
        if !code.is_rejection() {
            return Err(ContractError::InvalidProbeRejectionCode {
                code: code.to_string(),
            });
        }
    }
    Ok(())
}

fn validate_probe_invocation(input: &AbiProbeEvidenceInput) -> Result<(), ContractError> {
    normalized_absolute_path(
        "probe.compiler_executable",
        input.compiler_executable.clone(),
    )?;
    if input
        .compiler_arguments
        .iter()
        .any(|argument| {
            matches!(argument, ProbeCompilerArgument::Literal(value) if native_has_nul(value))
        })
    {
        return Err(ContractError::InvalidNativeString {
            field: "probe.compiler_argument",
        });
    }
    let source_roles = input
        .compiler_arguments
        .iter()
        .filter(|argument| matches!(argument, ProbeCompilerArgument::ProbeSource))
        .count();
    let output_roles = input
        .compiler_arguments
        .iter()
        .filter(|argument| matches!(argument, ProbeCompilerArgument::OutputArtifact))
        .count();
    if source_roles != 1 || output_roles != 1 {
        return Err(ContractError::InvalidPolicy {
            reason: "compiler invocation requires exactly one logical probe source and output",
        });
    }
    match (&input.method, &input.runner, &input.execution_result) {
        (
            ProbeMethod::ExecutedHarness,
            ProbeRunnerEvidence::Executed {
                executable_path,
                arguments,
                ..
            },
            Some(_),
        ) => {
            normalized_absolute_path("probe.runner.executable", executable_path.clone())?;
            if arguments.iter().any(|argument| {
                matches!(argument, ProbeRunnerArgument::Literal(value) if native_has_nul(value))
            }) {
                return Err(ContractError::InvalidNativeString {
                    field: "probe.runner.argument",
                });
            }
            if arguments
                .iter()
                .filter(|argument| matches!(argument, ProbeRunnerArgument::ProbeExecutable))
                .count()
                != 1
            {
                return Err(ContractError::InvalidPolicy {
                    reason: "runner invocation requires exactly one logical probe executable",
                });
            }
        }
        (
            ProbeMethod::CompilerLayoutDump | ProbeMethod::CompileTimeAssertion,
            ProbeRunnerEvidence::NotExecuted,
            None,
        ) => {}
        _ => {
            return Err(ContractError::InvalidPolicy {
                reason: "probe method, runner evidence, and execution result are incoherent",
            });
        }
    }

    let has_verified_outcome = input
        .subject_outcomes
        .iter()
        .any(ProbeSubjectOutcome::verified);
    if has_verified_outcome
        && (!input.compile_result.succeeded() || input.compile_result.output_artifact().is_none())
    {
        return Err(ContractError::InvalidProbeResult {
            reason: "verified probe subjects require a successful compile and output artifact",
        });
    }
    if has_verified_outcome
        && input.method == ProbeMethod::ExecutedHarness
        && !input
            .execution_result
            .as_ref()
            .is_some_and(|result| result.succeeded())
    {
        return Err(ContractError::InvalidProbeResult {
            reason: "verified executed-harness subjects require successful execution",
        });
    }
    Ok(())
}

fn derive_probe_id(input: &AbiProbeEvidenceInput) -> Result<ProbeEvidenceId, ContractError> {
    let mut fields = vec![
        input.source_fingerprint.as_bytes().to_vec(),
        input.target_fingerprint.as_bytes().to_vec(),
        serde_json::to_vec(&input.compiler).map_err(canonical_error)?,
    ];
    push_native_string_fields(&mut fields, input.compiler_executable.as_os_str());
    fields.push(
        (input.compiler_arguments.len() as u64)
            .to_le_bytes()
            .to_vec(),
    );
    for argument in &input.compiler_arguments {
        match argument {
            ProbeCompilerArgument::Literal(value) => {
                fields.push(b"compiler-literal".to_vec());
                push_native_string_fields(&mut fields, value);
            }
            ProbeCompilerArgument::ProbeSource => fields.push(b"compiler-probe-source".to_vec()),
            ProbeCompilerArgument::OutputArtifact => {
                fields.push(b"compiler-output-artifact".to_vec());
            }
        }
    }
    fields.extend([
        serde_json::to_vec(&input.abi_flags).map_err(canonical_error)?,
        input.probe_source_fingerprint.as_bytes().to_vec(),
        serde_json::to_vec(&input.subjects).map_err(canonical_error)?,
        serde_json::to_vec(&input.method).map_err(canonical_error)?,
    ]);
    push_native_string_fields(
        &mut fields,
        input.execution_policy.temporary_parent().as_os_str(),
    );
    fields.push(serde_json::to_vec(input.execution_policy.environment()).map_err(canonical_error)?);
    fields.push(serde_json::to_vec(&input.execution_policy.limits()).map_err(canonical_error)?);
    fields.push(serde_json::to_vec(&input.compile_result).map_err(canonical_error)?);
    match &input.runner {
        ProbeRunnerEvidence::NotExecuted => fields.push(b"not-executed".to_vec()),
        ProbeRunnerEvidence::Executed {
            executable_path,
            executable_fingerprint,
            arguments,
        } => {
            fields.push(b"executed".to_vec());
            fields.push(executable_fingerprint.as_bytes().to_vec());
            push_native_string_fields(&mut fields, executable_path.as_os_str());
            for argument in arguments {
                match argument {
                    ProbeRunnerArgument::Literal(value) => {
                        fields.push(b"runner-literal".to_vec());
                        push_native_string_fields(&mut fields, value);
                    }
                    ProbeRunnerArgument::ProbeExecutable => {
                        fields.push(b"runner-probe-executable".to_vec());
                    }
                }
            }
        }
    }
    fields.push(serde_json::to_vec(&input.execution_result).map_err(canonical_error)?);
    fields.push(serde_json::to_vec(&input.subject_outcomes).map_err(canonical_error)?);
    Ok(ProbeEvidenceId::derive(&fields))
}

fn push_native_string_fields(fields: &mut Vec<Vec<u8>>, value: &std::ffi::OsStr) {
    let (platform, units) = native_units(value);
    fields.push(platform.to_vec());
    fields.push(units);
}

fn canonical_error(error: serde_json::Error) -> ContractError {
    ContractError::Canonical {
        message: error.to_string(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSource {
    CompilerProbe,
    DebugInfo,
    ObjectMetadata,
    Corroborated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceConfidence {
    Corroborated,
    Measured,
    Inferred,
}

impl EvidenceConfidence {
    pub const fn is_strictly_measured(self) -> bool {
        matches!(self, Self::Corroborated | Self::Measured)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldLayoutEvidence {
    child: ChildId,
    offset_bits: u64,
    size_bits: Option<u64>,
    alignment_bits: Option<u32>,
}

impl FieldLayoutEvidence {
    pub fn try_new(
        child: ChildId,
        offset_bits: u64,
        size_bits: Option<u64>,
        alignment_bits: Option<u32>,
    ) -> Result<Self, ContractError> {
        if alignment_bits.is_some_and(|alignment| !valid_alignment(alignment)) {
            return Err(ContractError::InvalidText {
                field: "field_layout.alignment_bits",
            });
        }
        if size_bits.is_some_and(|size| offset_bits.checked_add(size).is_none()) {
            return Err(ContractError::InvalidText {
                field: "field_layout.offset_plus_size",
            });
        }
        Ok(Self {
            child,
            offset_bits,
            size_bits,
            alignment_bits,
        })
    }

    pub const fn child(&self) -> ChildId {
        self.child
    }

    pub const fn offset_bits(&self) -> u64 {
        self.offset_bits
    }

    pub const fn size_bits(&self) -> Option<u64> {
        self.size_bits
    }

    pub const fn alignment_bits(&self) -> Option<u32> {
        self.alignment_bits
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnumVariantEvidence {
    child: ChildId,
    value: ExactInteger,
}

impl EnumVariantEvidence {
    pub fn new(child: ChildId, value: ExactInteger) -> Self {
        Self { child, value }
    }

    pub const fn child(&self) -> ChildId {
        self.child
    }

    pub fn value(&self) -> &ExactInteger {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecordLayoutEvidence {
    declaration: DeclarationId,
    source_fingerprint: SourceFingerprint,
    target_fingerprint: TargetFingerprint,
    size_bits: u64,
    alignment_bits: u32,
    fields: Vec<FieldLayoutEvidence>,
    probe: ProbeEvidenceId,
    source: EvidenceSource,
    confidence: EvidenceConfidence,
}

impl RecordLayoutEvidence {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        declaration: DeclarationId,
        source_fingerprint: SourceFingerprint,
        target_fingerprint: TargetFingerprint,
        size_bits: u64,
        alignment_bits: u32,
        mut fields: Vec<FieldLayoutEvidence>,
        probe: ProbeEvidenceId,
        source: EvidenceSource,
        confidence: EvidenceConfidence,
    ) -> Result<Self, ContractError> {
        validate_layout_shape(declaration, size_bits, alignment_bits)?;
        fields.sort_by_key(FieldLayoutEvidence::child);
        validate_unique_children(declaration, fields.iter().map(FieldLayoutEvidence::child))?;
        Ok(Self {
            declaration,
            source_fingerprint,
            target_fingerprint,
            size_bits,
            alignment_bits,
            fields,
            probe,
            source,
            confidence,
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

    pub const fn size_bits(&self) -> u64 {
        self.size_bits
    }

    pub const fn alignment_bits(&self) -> u32 {
        self.alignment_bits
    }

    pub fn fields(&self) -> &[FieldLayoutEvidence] {
        &self.fields
    }

    pub const fn probe(&self) -> ProbeEvidenceId {
        self.probe
    }

    pub const fn source(&self) -> EvidenceSource {
        self.source
    }

    pub const fn confidence(&self) -> EvidenceConfidence {
        self.confidence
    }

    /// Canonical fingerprint of the measured record shape.
    ///
    /// Probe identity is deliberately excluded. The probe outcome binds this
    /// fingerprint while the layout separately points back to that probe;
    /// including the probe ID would create a cryptographic cycle because the
    /// probe ID also commits to its subject outcomes.
    pub fn fingerprint(&self) -> Result<ContentFingerprint, ContractError> {
        #[derive(Serialize)]
        struct RecordLayoutFingerprint<'a> {
            domain: &'static str,
            declaration: DeclarationId,
            source_fingerprint: SourceFingerprint,
            target_fingerprint: TargetFingerprint,
            size_bits: u64,
            alignment_bits: u32,
            fields: &'a [FieldLayoutEvidence],
            source: EvidenceSource,
            confidence: EvidenceConfidence,
        }

        serde_json::to_vec(&RecordLayoutFingerprint {
            domain: "follang.linc.record-layout-shape.v1",
            declaration: self.declaration,
            source_fingerprint: self.source_fingerprint,
            target_fingerprint: self.target_fingerprint,
            size_bits: self.size_bits,
            alignment_bits: self.alignment_bits,
            fields: &self.fields,
            source: self.source,
            confidence: self.confidence,
        })
        .map(|bytes| ContentFingerprint::from_content(&bytes))
        .map_err(canonical_error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnumLayoutEvidence {
    declaration: DeclarationId,
    source_fingerprint: SourceFingerprint,
    target_fingerprint: TargetFingerprint,
    storage_bits: u64,
    alignment_bits: u32,
    signedness: Signedness,
    variants: Vec<EnumVariantEvidence>,
    probe: ProbeEvidenceId,
    source: EvidenceSource,
    confidence: EvidenceConfidence,
}

impl EnumLayoutEvidence {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        declaration: DeclarationId,
        source_fingerprint: SourceFingerprint,
        target_fingerprint: TargetFingerprint,
        storage_bits: u64,
        alignment_bits: u32,
        signedness: Signedness,
        mut variants: Vec<EnumVariantEvidence>,
        probe: ProbeEvidenceId,
        source: EvidenceSource,
        confidence: EvidenceConfidence,
    ) -> Result<Self, ContractError> {
        validate_layout_shape(declaration, storage_bits, alignment_bits)?;
        variants.sort_by_key(EnumVariantEvidence::child);
        validate_unique_children(declaration, variants.iter().map(EnumVariantEvidence::child))?;
        Ok(Self {
            declaration,
            source_fingerprint,
            target_fingerprint,
            storage_bits,
            alignment_bits,
            signedness,
            variants,
            probe,
            source,
            confidence,
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

    pub const fn storage_bits(&self) -> u64 {
        self.storage_bits
    }

    pub const fn alignment_bits(&self) -> u32 {
        self.alignment_bits
    }

    pub const fn signedness(&self) -> Signedness {
        self.signedness
    }

    pub fn variants(&self) -> &[EnumVariantEvidence] {
        &self.variants
    }

    pub const fn probe(&self) -> ProbeEvidenceId {
        self.probe
    }

    pub const fn source(&self) -> EvidenceSource {
        self.source
    }

    pub const fn confidence(&self) -> EvidenceConfidence {
        self.confidence
    }

    /// Canonical fingerprint of the measured enum representation.
    ///
    /// The probe back-reference is excluded for the same reason as
    /// [`RecordLayoutEvidence::fingerprint`].
    pub fn fingerprint(&self) -> Result<ContentFingerprint, ContractError> {
        #[derive(Serialize)]
        struct EnumLayoutFingerprint<'a> {
            domain: &'static str,
            declaration: DeclarationId,
            source_fingerprint: SourceFingerprint,
            target_fingerprint: TargetFingerprint,
            storage_bits: u64,
            alignment_bits: u32,
            signedness: Signedness,
            variants: &'a [EnumVariantEvidence],
            source: EvidenceSource,
            confidence: EvidenceConfidence,
        }

        serde_json::to_vec(&EnumLayoutFingerprint {
            domain: "follang.linc.enum-layout-shape.v1",
            declaration: self.declaration,
            source_fingerprint: self.source_fingerprint,
            target_fingerprint: self.target_fingerprint,
            storage_bits: self.storage_bits,
            alignment_bits: self.alignment_bits,
            signedness: self.signedness,
            variants: &self.variants,
            source: self.source,
            confidence: self.confidence,
        })
        .map(|bytes| ContentFingerprint::from_content(&bytes))
        .map_err(canonical_error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    content = "evidence",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum LayoutEvidence {
    Record(RecordLayoutEvidence),
    Enum(EnumLayoutEvidence),
}

impl LayoutEvidence {
    pub const fn declaration(&self) -> DeclarationId {
        match self {
            Self::Record(evidence) => evidence.declaration,
            Self::Enum(evidence) => evidence.declaration,
        }
    }

    pub const fn source_fingerprint(&self) -> SourceFingerprint {
        match self {
            Self::Record(evidence) => evidence.source_fingerprint,
            Self::Enum(evidence) => evidence.source_fingerprint,
        }
    }

    pub const fn target_fingerprint(&self) -> TargetFingerprint {
        match self {
            Self::Record(evidence) => evidence.target_fingerprint,
            Self::Enum(evidence) => evidence.target_fingerprint,
        }
    }

    pub const fn confidence(&self) -> EvidenceConfidence {
        match self {
            Self::Record(evidence) => evidence.confidence,
            Self::Enum(evidence) => evidence.confidence,
        }
    }

    pub const fn source(&self) -> EvidenceSource {
        match self {
            Self::Record(evidence) => evidence.source,
            Self::Enum(evidence) => evidence.source,
        }
    }

    pub const fn probe(&self) -> ProbeEvidenceId {
        match self {
            Self::Record(evidence) => evidence.probe,
            Self::Enum(evidence) => evidence.probe,
        }
    }

    /// Canonical fingerprint bound by this layout's verified probe outcome.
    pub fn fingerprint(&self) -> Result<ContentFingerprint, ContractError> {
        match self {
            Self::Record(evidence) => evidence.fingerprint(),
            Self::Enum(evidence) => evidence.fingerprint(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SymbolReference {
    provider: ProviderId,
    symbol: ArtifactSymbolId,
}

impl SymbolReference {
    pub const fn new(provider: ProviderId, symbol: ArtifactSymbolId) -> Self {
        Self { provider, symbol }
    }

    pub const fn provider(&self) -> ProviderId {
        self.provider
    }

    pub const fn symbol(&self) -> ArtifactSymbolId {
        self.symbol
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProviderAssessment {
    NotRequired,
    Resolved {
        provider: ProviderId,
        artifact_fingerprint: ArtifactFingerprint,
    },
    Unresolved,
    Ambiguous {
        providers: Vec<ProviderId>,
    },
    Rejected {
        code: LincCode,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum SymbolAssessment {
    NotRequired,
    Exact {
        symbol: SymbolReference,
        expected_name: String,
        actual_name: String,
        kind: SymbolKind,
        decoration: SymbolDecoration,
    },
    Missing {
        expected_name: String,
    },
    Ambiguous {
        candidates: Vec<SymbolReference>,
    },
    WrongKind {
        symbol: SymbolReference,
        expected: SymbolKind,
        actual: SymbolKind,
    },
    Rejected {
        code: LincCode,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum LayoutAssessment {
    NotRequired,
    Available {
        confidence: EvidenceConfidence,
        probe: ProbeEvidenceId,
    },
    Missing,
    Rejected {
        code: LincCode,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum CallableAbiAssessment {
    NotApplicable,
    Confirmed {
        calling_convention: CallingConvention,
        confidence: EvidenceConfidence,
        probe: ProbeEvidenceId,
    },
    Partial {
        calling_convention: Option<CallingConvention>,
        reason: String,
    },
    Missing,
    Rejected {
        code: LincCode,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarationEvidenceInput {
    pub declaration: DeclarationId,
    pub source_fingerprint: SourceFingerprint,
    pub target_fingerprint: TargetFingerprint,
    pub provider: ProviderAssessment,
    pub symbol: SymbolAssessment,
    pub layout: LayoutAssessment,
    pub callable_abi: CallableAbiAssessment,
}

/// Four independent evidence dimensions for one PARC declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeclarationEvidence {
    declaration: DeclarationId,
    source_fingerprint: SourceFingerprint,
    target_fingerprint: TargetFingerprint,
    provider: ProviderAssessment,
    symbol: SymbolAssessment,
    layout: LayoutAssessment,
    callable_abi: CallableAbiAssessment,
}

impl DeclarationEvidence {
    pub fn try_new(mut input: DeclarationEvidenceInput) -> Result<Self, ContractError> {
        canonicalize_assessments(&mut input)?;
        validate_provider_assessment(input.declaration, &input.provider)?;
        validate_symbol_assessment(input.declaration, &input.symbol)?;
        validate_callable_abi(&input.callable_abi)?;
        Ok(Self {
            declaration: input.declaration,
            source_fingerprint: input.source_fingerprint,
            target_fingerprint: input.target_fingerprint,
            provider: input.provider,
            symbol: input.symbol,
            layout: input.layout,
            callable_abi: input.callable_abi,
        })
    }

    pub(crate) fn try_from_wire(input: DeclarationEvidenceInput) -> Result<Self, ContractError> {
        if !assessments_are_canonical(&input) {
            return Err(ContractError::NonCanonicalOrder {
                collection: "ambiguous evidence candidates",
            });
        }
        Self::try_new(input)
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

    pub fn provider(&self) -> &ProviderAssessment {
        &self.provider
    }

    pub fn symbol(&self) -> &SymbolAssessment {
        &self.symbol
    }

    pub fn layout(&self) -> &LayoutAssessment {
        &self.layout
    }

    pub fn callable_abi(&self) -> &CallableAbiAssessment {
        &self.callable_abi
    }
}

fn canonicalize_assessments(input: &mut DeclarationEvidenceInput) -> Result<(), ContractError> {
    if let ProviderAssessment::Ambiguous { providers } = &mut input.provider {
        providers.sort_unstable();
        if providers.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ContractError::InvalidAmbiguity {
                declaration: input.declaration,
            });
        }
    }
    if let SymbolAssessment::Ambiguous { candidates } = &mut input.symbol {
        candidates.sort_unstable();
        if candidates.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ContractError::InvalidAmbiguity {
                declaration: input.declaration,
            });
        }
    }
    Ok(())
}

fn assessments_are_canonical(input: &DeclarationEvidenceInput) -> bool {
    let providers_canonical = match &input.provider {
        ProviderAssessment::Ambiguous { providers } => {
            providers.windows(2).all(|pair| pair[0] < pair[1])
        }
        _ => true,
    };
    let symbols_canonical = match &input.symbol {
        SymbolAssessment::Ambiguous { candidates } => {
            candidates.windows(2).all(|pair| pair[0] < pair[1])
        }
        _ => true,
    };
    providers_canonical && symbols_canonical
}

fn validate_provider_assessment(
    declaration: DeclarationId,
    assessment: &ProviderAssessment,
) -> Result<(), ContractError> {
    match assessment {
        ProviderAssessment::Ambiguous { providers } => {
            let unique: BTreeSet<_> = providers.iter().copied().collect();
            if unique.len() < 2 || unique.len() != providers.len() {
                return Err(ContractError::InvalidAmbiguity { declaration });
            }
        }
        ProviderAssessment::Rejected { .. } => {}
        ProviderAssessment::NotRequired
        | ProviderAssessment::Resolved { .. }
        | ProviderAssessment::Unresolved => {}
    }
    Ok(())
}

fn validate_symbol_assessment(
    declaration: DeclarationId,
    assessment: &SymbolAssessment,
) -> Result<(), ContractError> {
    match assessment {
        SymbolAssessment::Exact {
            expected_name,
            actual_name,
            ..
        } => {
            validate_text("declaration_evidence.symbol.expected_name", expected_name)?;
            validate_text("declaration_evidence.symbol.actual_name", actual_name)?;
        }
        SymbolAssessment::Missing { expected_name } => {
            validate_text("declaration_evidence.symbol.expected_name", expected_name)?;
        }
        SymbolAssessment::Ambiguous { candidates } => {
            let unique: BTreeSet<_> = candidates.iter().copied().collect();
            if unique.len() < 2 || unique.len() != candidates.len() {
                return Err(ContractError::InvalidAmbiguity { declaration });
            }
        }
        SymbolAssessment::Rejected { .. } => {}
        SymbolAssessment::NotRequired | SymbolAssessment::WrongKind { .. } => {}
    }
    Ok(())
}

fn validate_callable_abi(assessment: &CallableAbiAssessment) -> Result<(), ContractError> {
    match assessment {
        CallableAbiAssessment::Partial { reason, .. } => {
            validate_text("declaration_evidence.callable_abi.reason", reason)?;
        }
        CallableAbiAssessment::Rejected { .. } => {}
        CallableAbiAssessment::NotApplicable
        | CallableAbiAssessment::Confirmed { .. }
        | CallableAbiAssessment::Missing => {}
    }
    Ok(())
}

fn valid_alignment(alignment_bits: u32) -> bool {
    alignment_bits >= 8 && alignment_bits.is_power_of_two()
}

fn validate_layout_shape(
    declaration: DeclarationId,
    size_bits: u64,
    alignment_bits: u32,
) -> Result<(), ContractError> {
    if size_bits == 0 || !size_bits.is_multiple_of(8) || !valid_alignment(alignment_bits) {
        Err(ContractError::InvalidLayout { declaration })
    } else {
        Ok(())
    }
}

fn validate_unique_children(
    declaration: DeclarationId,
    children: impl Iterator<Item = ChildId>,
) -> Result<(), ContractError> {
    let mut seen = BTreeSet::new();
    for child in children {
        if !seen.insert(child) {
            return Err(ContractError::DuplicateLayoutChild { declaration, child });
        }
    }
    Ok(())
}
