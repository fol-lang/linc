use std::path::PathBuf;

use parc::contract::CompleteSourcePackage;
use serde::{Deserialize, Serialize};

use super::{
    model::{native_has_nul, normalized_absolute_path},
    validate_native_inputs, ArtifactFingerprint, ContractError, NativeInput, ProbeExecutionPolicy,
    ProbeRunnerArgument,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionPolicy {
    /// Only exact artifact paths are accepted.
    ExactPathsOnly,
    /// Name lookup is limited to explicitly supplied search paths.
    HermeticSearch,
    /// The captured target toolchain search configuration may also be used.
    ToolchainSearch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbePolicy {
    Disabled,
    CompileOnly,
    CompileAndRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceAcceptancePolicy {
    MeasuredOnly,
    AllowInferred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeakSymbolPolicy {
    Reject,
    AllowUnique,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerCommand {
    program: PathBuf,
    executable_fingerprint: ArtifactFingerprint,
    arguments: Vec<ProbeRunnerArgument>,
}

impl RunnerCommand {
    pub fn try_new(
        program: PathBuf,
        executable_fingerprint: ArtifactFingerprint,
        arguments: Vec<ProbeRunnerArgument>,
    ) -> Result<Self, ContractError> {
        let program = normalized_absolute_path("runner.program", program)?;
        if arguments.iter().any(|argument| {
            matches!(argument, ProbeRunnerArgument::Literal(value) if native_has_nul(value))
        }) {
            return Err(ContractError::InvalidNativeString {
                field: "runner.argument",
            });
        }
        if arguments
            .iter()
            .filter(|argument| matches!(argument, ProbeRunnerArgument::ProbeExecutable))
            .count()
            != 1
        {
            return Err(ContractError::InvalidPolicy {
                reason: "runner command requires exactly one logical probe executable",
            });
        }
        Ok(Self {
            program,
            executable_fingerprint,
            arguments,
        })
    }

    pub fn program(&self) -> &std::path::Path {
        &self.program
    }

    pub fn arguments(&self) -> &[ProbeRunnerArgument] {
        &self.arguments
    }

    pub const fn executable_fingerprint(&self) -> ArtifactFingerprint {
        self.executable_fingerprint
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerPolicy {
    Unavailable,
    Explicit(RunnerCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisPolicyInput {
    pub resolution: ResolutionPolicy,
    pub probe: ProbePolicy,
    pub runner: RunnerPolicy,
    pub layout_evidence: EvidenceAcceptancePolicy,
    pub callable_abi_evidence: EvidenceAcceptancePolicy,
    pub weak_symbols: WeakSymbolPolicy,
    pub probe_execution: ProbeExecutionPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisPolicy {
    resolution: ResolutionPolicy,
    probe: ProbePolicy,
    runner: RunnerPolicy,
    layout_evidence: EvidenceAcceptancePolicy,
    callable_abi_evidence: EvidenceAcceptancePolicy,
    weak_symbols: WeakSymbolPolicy,
    probe_execution: ProbeExecutionPolicy,
}

impl AnalysisPolicy {
    pub fn try_new(input: AnalysisPolicyInput) -> Result<Self, ContractError> {
        let policy = Self {
            resolution: input.resolution,
            probe: input.probe,
            runner: input.runner,
            layout_evidence: input.layout_evidence,
            callable_abi_evidence: input.callable_abi_evidence,
            weak_symbols: input.weak_symbols,
            probe_execution: input.probe_execution,
        };
        policy.validate()?;
        Ok(policy)
    }

    pub fn strict(
        resolution: ResolutionPolicy,
        probe: ProbePolicy,
        runner: RunnerPolicy,
        probe_execution: ProbeExecutionPolicy,
    ) -> Result<Self, ContractError> {
        Self::try_new(AnalysisPolicyInput {
            resolution,
            probe,
            runner,
            layout_evidence: EvidenceAcceptancePolicy::MeasuredOnly,
            callable_abi_evidence: EvidenceAcceptancePolicy::MeasuredOnly,
            weak_symbols: WeakSymbolPolicy::Reject,
            probe_execution,
        })
    }

    pub fn with_evidence_acceptance(
        mut self,
        layout_evidence: EvidenceAcceptancePolicy,
        callable_abi_evidence: EvidenceAcceptancePolicy,
    ) -> Self {
        self.layout_evidence = layout_evidence;
        self.callable_abi_evidence = callable_abi_evidence;
        self
    }

    pub fn with_weak_symbol_policy(mut self, weak_symbols: WeakSymbolPolicy) -> Self {
        self.weak_symbols = weak_symbols;
        self
    }

    pub fn with_probe_execution_policy(mut self, probe_execution: ProbeExecutionPolicy) -> Self {
        self.probe_execution = probe_execution;
        self
    }

    pub const fn resolution(&self) -> ResolutionPolicy {
        self.resolution
    }

    pub const fn probe(&self) -> ProbePolicy {
        self.probe
    }

    pub fn runner(&self) -> &RunnerPolicy {
        &self.runner
    }

    pub const fn layout_evidence(&self) -> EvidenceAcceptancePolicy {
        self.layout_evidence
    }

    pub const fn callable_abi_evidence(&self) -> EvidenceAcceptancePolicy {
        self.callable_abi_evidence
    }

    pub const fn weak_symbols(&self) -> WeakSymbolPolicy {
        self.weak_symbols
    }

    pub const fn probe_execution(&self) -> &ProbeExecutionPolicy {
        &self.probe_execution
    }

    pub(crate) fn validate(&self) -> Result<(), ContractError> {
        if self.probe == ProbePolicy::CompileAndRun
            && !matches!(&self.runner, RunnerPolicy::Explicit(_))
        {
            return Err(ContractError::InvalidPolicy {
                reason: "compile-and-run probes require an explicit runner command",
            });
        }
        if self.probe != ProbePolicy::CompileAndRun
            && !matches!(&self.runner, RunnerPolicy::Unavailable)
        {
            return Err(ContractError::InvalidPolicy {
                reason: "only compile-and-run probes may carry a runner command",
            });
        }
        Ok(())
    }
}

/// Typed strict-analysis request boundary. Operational analysis will consume
/// this value; only a complete PARC closure can be supplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisRequest<'a> {
    source: &'a CompleteSourcePackage,
    native_inputs: &'a [NativeInput],
    policy: AnalysisPolicy,
}

impl<'a> AnalysisRequest<'a> {
    pub fn try_new(
        source: &'a CompleteSourcePackage,
        native_inputs: &'a [NativeInput],
        policy: AnalysisPolicy,
    ) -> Result<Self, ContractError> {
        validate_native_inputs(native_inputs)?;
        policy.validate()?;
        if policy.resolution == ResolutionPolicy::ExactPathsOnly
            && native_inputs.iter().any(is_search_input)
        {
            return Err(ContractError::InvalidPolicy {
                reason: "exact-path resolution cannot accept search paths or name requests",
            });
        }
        Ok(Self {
            source,
            native_inputs,
            policy,
        })
    }

    pub const fn source(&self) -> &'a CompleteSourcePackage {
        self.source
    }

    pub const fn native_inputs(&self) -> &'a [NativeInput] {
        self.native_inputs
    }

    pub const fn policy(&self) -> &AnalysisPolicy {
        &self.policy
    }
}

fn is_search_input(input: &NativeInput) -> bool {
    matches!(
        input,
        NativeInput::SearchNative(_)
            | NativeInput::StaticLibraryName(_)
            | NativeInput::DynamicLibraryName(_)
            | NativeInput::ImportLibraryName(_)
            | NativeInput::FrameworkName { .. }
    )
}
