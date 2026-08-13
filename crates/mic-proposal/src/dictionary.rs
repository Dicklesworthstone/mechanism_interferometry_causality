#![forbid(unsafe_code)]
//! Proposal-only contracts for algebraic transport-dictionary search.
//!
//! A frozen artifact records parameterizations of environment log transports.
//! It never establishes that an atom is a causal mechanism, identifies a
//! target, groups interventions, or authorizes an edge.

use super::{
    ContractRequest, NextQuery, ProposalAuthority, ProposalError, ProposalSource,
    SelfDrivingRequest, ShiftFactorizationDraft, freeze_shift_factorization_proposal,
};
use core::fmt::Write as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const SCHEMA_VERSION: &str = "1.0.0";
const MAX_FOLDS: usize = 32;
const MAX_ATTEMPTS: usize = 4_096;
const MAX_FACTORS: usize = 64;
const MAX_ENVIRONMENTS: usize = 4_096;
const MAX_CODE_CELLS: usize = 1_048_576;
const MAX_FIT_ITERATIONS: usize = 100_000;
const MAX_IDENTIFIER_CHARS: usize = 1_024;

/// Reference convention used by every environment transport in the search.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DictionaryReferencePolicy {
    /// The density-ratio reference is the marked factorial zero law.
    MarkedZeroCode,
    /// An arbitrary reference is used and the common intercept is modeled.
    ArbitraryReferenceWithIntercept,
}

/// Algebraic code family admitted by a preregistered dictionary search.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DictionaryCodePolicy {
    /// Environment codes are externally fixed before fitting.
    KnownIncidence,
    /// A marked baseline and pure single-change rows are externally supplied.
    MarkedPureAnchors,
    /// The candidate is an exact complete binary cube.
    CompleteBinaryCube,
    /// Codes are incomplete and learned from the discovery environments.
    UnknownSparseCodes,
}

/// Deterministic ordering rule for completed algebraic candidates.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DictionaryRankingRule {
    /// Lower untouched-discovery loss, then lower description length, then ID.
    HeldoutLossThenDescriptionLength,
}

/// Hard preregistered limits for one dictionary search.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DictionarySearchBudget {
    /// Maximum attempted parameterizations, including failures and not-run entries.
    pub max_attempts: usize,
    /// Maximum atoms in one parameterization.
    pub max_factors: usize,
    /// Maximum environment rows in one code matrix.
    pub max_environments: usize,
    /// Maximum code-matrix cells across the complete attempt library.
    pub max_code_cells: usize,
    /// Maximum fit iterations allowed for one attempted parameterization.
    pub max_fit_iterations_per_attempt: usize,
}

/// Immutable preregistered plan for an algebraic dictionary search.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DictionarySearchPlan {
    /// Plan schema version. Currently `1.0.0`.
    pub schema_version: String,
    /// Opaque plan identifier.
    pub plan_id: String,
    /// Fingerprint of the validated outer discovery request.
    pub self_driving_request_fingerprint: String,
    /// Fingerprint of the complete upstream shift library.
    pub shift_library_fingerprint: String,
    /// Reference-law convention.
    pub reference_policy: DictionaryReferencePolicy,
    /// Candidate code family.
    pub code_policy: DictionaryCodePolicy,
    /// Number of unit-level inner folds.
    pub n_inner_folds: usize,
    /// Frozen absolute pivot tolerance for the displayed f64 code matrix.
    pub algebraic_rank_tolerance: f64,
    /// Canonical increasing atom-count grid.
    pub rank_grid: Vec<usize>,
    /// Complete ordered attempt specification SHA-256 library fixed before fitting.
    pub attempt_specification_sha256s: Vec<String>,
    /// Candidate ordering rule.
    pub ranking_rule: DictionaryRankingRule,
    /// Hard search limits.
    pub budget: DictionarySearchBudget,
}

/// Realized discovery-only execution binding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DictionaryExecutionBinding {
    /// Unit-level fold-plan SHA-256.
    pub fold_plan_sha256: String,
    /// Common discovery-cohort SHA-256.
    pub common_cohort_sha256: String,
    /// Frozen state-representation SHA-256.
    pub state_representation_sha256: String,
    /// Reference-law artifact SHA-256.
    pub reference_law_sha256: String,
    /// Number of declared discovery units.
    pub n_units: usize,
    /// Number of discovery rows.
    pub n_rows: usize,
}

/// One externally stored fitted atom.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TransportAtomDraft {
    /// Opaque atom identifier; it is not a mechanism or target name.
    pub atom_id: String,
    /// SHA-256 of the external atom artifact.
    pub artifact_sha256: String,
    /// Exact byte length of the external artifact.
    pub artifact_bytes: u64,
    /// Closed media-type label.
    pub media_type: String,
    /// Upstream regime-information support identifier.
    pub support_id: String,
}

/// One environment row in a candidate code matrix.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentCodeRow {
    /// Upstream environment identifier.
    pub environment_id: String,
    /// Coefficients in canonical atom order.
    pub coefficients: Vec<f64>,
}

/// Exact algebraic case claimed for a completed parameterization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "case", rename_all = "snake_case")]
pub enum AlgebraicRecoveryCase {
    /// Codes are known; the observed augmented or baseline-subtracted rank is recorded.
    KnownCodes {
        /// Real matrix rank of the relevant design.
        observed_design_rank: usize,
    },
    /// Marked pure anchors span the proposed atom space.
    MarkedPureAnchors {
        /// Marked factorial-baseline environment.
        baseline_environment_id: String,
        /// One pure-anchor environment per atom, in atom order.
        anchor_environment_ids: Vec<String>,
        /// Rank of the observed anchor differences.
        observed_anchor_rank: usize,
        /// Whether physical anchor amplitudes are externally calibrated.
        amplitudes_calibrated: bool,
        /// Whether anchor-to-atom labels are externally assigned.
        anchor_labels_assigned: bool,
    },
    /// Every vertex of a binary cube is present.
    CompleteBinaryCube {
        /// Rank of the edge functions in the claimed representation.
        observed_edge_rank: usize,
        /// Whether the factorial zero vertex is externally marked.
        marked_zero: bool,
        /// Always false in this proposal layer because atom bytes are not opened.
        edge_function_independence_verified: bool,
    },
    /// Codes are incomplete or unknown, so general invertible mixing survives.
    IncompleteUnknownCodes {
        /// Rank of the displayed code matrix.
        observed_design_rank: usize,
    },
    /// The proposed code/atom representation is rank deficient.
    RankDegenerate {
        /// Rank of the displayed code matrix.
        observed_design_rank: usize,
    },
}

/// Descriptive scores for one completed parameterization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DictionaryFitDiagnostics {
    /// Training reconstruction loss; lower is better.
    pub training_reconstruction_loss: f64,
    /// Loss on untouched discovery subfolds; lower is better.
    pub heldout_discovery_loss: f64,
    /// Total dictionary-plus-code description length in declared units.
    pub description_length: f64,
    /// Number of fit iterations actually executed.
    pub fit_iterations: usize,
}

/// Completed adapter parameterization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DictionaryCandidateDraft {
    /// Adapter and discovery provenance.
    pub source: ProposalSource,
    /// Claimed algebraic recovery case.
    pub algebraic_case: AlgebraicRecoveryCase,
    /// Atoms in canonical identifier order.
    pub atoms: Vec<TransportAtomDraft>,
    /// Environment codes in canonical environment order.
    pub codes: Vec<EnvironmentCodeRow>,
    /// Discovery-only fit diagnostics.
    pub diagnostics: DictionaryFitDiagnostics,
    /// Whether deterministic/proxy support aliasing survived the search.
    pub support_aliasing_detected: bool,
}

/// Complete outcome of one preregistered attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DictionaryAttemptOutcome {
    /// Budget ordering prevented execution; the attempt remains fingerprinted.
    NotRun {
        /// Stable non-authoritative reason code.
        reason: String,
    },
    /// Validation or fitting rejected the attempt.
    Rejected {
        /// Stable non-authoritative reason code.
        reason: String,
    },
    /// A discovery-only algebraic parameterization completed.
    Completed {
        /// Full completed candidate.
        candidate: Box<DictionaryCandidateDraft>,
    },
}

/// One preregistered attempt, including rejected and unexecuted entries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DictionaryAttemptDraft {
    /// Opaque attempt identifier.
    pub attempt_id: String,
    /// SHA-256 of the complete preregistered attempt specification.
    pub specification_sha256: String,
    /// Attempt outcome.
    pub outcome: DictionaryAttemptOutcome,
}

/// Adapter-produced draft consumed by the freezer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TransportDictionaryDraft {
    /// Opaque proposal identifier.
    pub proposal_id: String,
    /// Realized discovery execution.
    pub execution: DictionaryExecutionBinding,
    /// Complete canonical attempt library, including failures and not-run entries.
    pub attempts: Vec<DictionaryAttemptDraft>,
    /// Missing external contracts.
    pub contract_requests: Vec<ContractRequest>,
    /// Ranked data-acquisition or contract actions.
    pub next_queries: Vec<NextQuery>,
}

/// Algebraic ambiguity that remains after the discovery search.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DictionaryAmbiguity {
    /// Atom labels may be permuted.
    CoordinatePermutation,
    /// A cube coordinate may be independently complemented when no zero is marked.
    BitComplement,
    /// Anchor amplitude fixes only a conventional atom scale.
    ConventionalScale,
    /// An arbitrary invertible mixing of atoms and codes remains observationally equivalent.
    GeneralLinearMixing,
    /// The displayed representation is rank deficient.
    RankDegeneracy,
    /// Deterministic or proxy coordinates leave atom support nonunique.
    AlmostEverywhereSupportAliasing,
    /// External atom content was not opened to establish functional independence.
    ExternalAtomIndependenceUnverified,
}

/// Operational state of the proposal artifact, never a causal verdict.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DictionaryProposalStatus {
    /// A completed parameterization and an explicit separate-audit action exist.
    RecommendedForSeparateAudit,
    /// At least one parameterization completed but no separate action is ranked.
    Inconclusive,
    /// No attempted parameterization completed.
    Abstained,
}

/// Fixed causal-family authority of this algebraic artifact.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CausalFamilyStatus {
    /// Locality, normalization, intervention semantics, and targets were not audited.
    NotEvaluated,
}

/// Fixed identity status for atoms and targets.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DictionaryIdentityStatus {
    /// The algebraic atom has no established mechanism or target identity.
    NotEstablished,
}

/// Fixed edge authority for a transport dictionary.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DictionaryEdgeAuthority {
    /// No ancestry or adjacency conclusion is authorized.
    None,
}

/// Fixed status of the source-selection premise.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DictionarySelectionStatus {
    /// Selected rows cannot establish the source inclusion law.
    Unestablished,
}

/// Confirmation access available to this discovery-only command.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DictionaryConfirmationAccess {
    /// Only an outer commitment exists; confirmation bytes were not accepted.
    SealedCommitmentOnly,
}

/// Immutable, serialize-only algebraic transport-dictionary proposal.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FrozenTransportDictionaryProposal {
    schema_version: String,
    artifact_kind: String,
    proposal_id: String,
    request_fingerprint: String,
    shift_library_fingerprint: String,
    search_plan_fingerprint: String,
    execution_binding_fingerprint: String,
    candidate_library_fingerprint: String,
    draft_fingerprint: String,
    authority: ProposalAuthority,
    certificate_eligible: bool,
    input_claims_verified: bool,
    algebraic_recovery_verified: bool,
    causal_family_status: CausalFamilyStatus,
    mechanism_identity: DictionaryIdentityStatus,
    target_identity: DictionaryIdentityStatus,
    same_target_grouping: DictionaryIdentityStatus,
    edge_authority: DictionaryEdgeAuthority,
    selection_status: DictionarySelectionStatus,
    confirmation_access: DictionaryConfirmationAccess,
    status: DictionaryProposalStatus,
    reference_policy: DictionaryReferencePolicy,
    code_policy: DictionaryCodePolicy,
    ranking_rule: DictionaryRankingRule,
    attempts: Vec<DictionaryAttemptDraft>,
    ranked_completed_attempt_ids: Vec<String>,
    ambiguities: Vec<DictionaryAmbiguity>,
    contract_requests: Vec<ContractRequest>,
    next_queries: Vec<NextQuery>,
    seed: u64,
}

impl FrozenTransportDictionaryProposal {
    /// Fixed proposal-only authority.
    #[must_use]
    pub const fn authority(&self) -> ProposalAuthority {
        self.authority
    }

    /// This artifact can never satisfy a certificate gate.
    #[must_use]
    pub const fn certificate_eligible(&self) -> bool {
        self.certificate_eligible
    }

    /// Fixed causal-family status.
    #[must_use]
    pub const fn causal_family_status(&self) -> CausalFamilyStatus {
        self.causal_family_status
    }

    /// Operational proposal state.
    #[must_use]
    pub const fn status(&self) -> DictionaryProposalStatus {
        self.status
    }

    /// Fingerprint of the complete ordered attempt library.
    #[must_use]
    pub fn candidate_library_fingerprint(&self) -> &str {
        &self.candidate_library_fingerprint
    }

    /// Algebraic ambiguities retained by the artifact.
    #[must_use]
    pub fn ambiguities(&self) -> &[DictionaryAmbiguity] {
        &self.ambiguities
    }
}

/// Dependency-inversion boundary implemented by external dictionary learners.
pub trait TransportDictionaryAdapter {
    /// Produces the complete discovery-only attempt library.
    fn draft(
        &self,
        request: &SelfDrivingRequest,
        plan: &DictionarySearchPlan,
    ) -> Result<TransportDictionaryDraft, ProposalError>;
}

/// Validates and freezes a proposal-only algebraic transport dictionary.
#[allow(clippy::too_many_lines)]
pub fn freeze_transport_dictionary_proposal(
    request: &SelfDrivingRequest,
    shift_draft: &ShiftFactorizationDraft,
    plan: &DictionarySearchPlan,
    draft: &TransportDictionaryDraft,
) -> Result<FrozenTransportDictionaryProposal, ProposalError> {
    request.validate()?;
    let request_fingerprint = request.fingerprint()?;
    let shift = freeze_shift_factorization_proposal(request, shift_draft)?;
    if plan.self_driving_request_fingerprint != request_fingerprint
        || plan.shift_library_fingerprint != shift.candidate_library_fingerprint()
    {
        return invalid("dictionary plan is detached from the discovery request or shift library");
    }
    validate_plan(plan)?;
    validate_execution(&draft.execution)?;
    if draft.execution.n_units != request.partition_claim.discovery_units {
        return invalid("dictionary execution unit count is detached from the discovery partition");
    }
    require_opaque("proposal_id", &draft.proposal_id, "dictionary_")?;
    if draft.attempts.len() != plan.attempt_specification_sha256s.len()
        || draft.attempts.len() > plan.budget.max_attempts
    {
        return invalid("attempt library is incomplete or exceeds its preregistered budget");
    }

    let environment_ids: BTreeSet<&str> = shift_draft
        .environments
        .iter()
        .map(|environment| environment.environment_id.as_str())
        .collect();
    let regime_support_ids: BTreeSet<&str> = shift_draft
        .supports
        .iter()
        .filter(|support| support.semantics == super::SupportSemantics::RegimeInformationSupport)
        .map(|support| support.support_id.as_str())
        .collect();

    let mut previous_attempt = None;
    preflight_code_cell_budget(&draft.attempts, plan.budget.max_code_cells)?;
    let mut total_code_cells = 0usize;
    let mut completed = 0usize;
    let mut ambiguities = BTreeSet::new();
    for (attempt, planned_sha256) in draft
        .attempts
        .iter()
        .zip(&plan.attempt_specification_sha256s)
    {
        require_opaque("attempt_id", &attempt.attempt_id, "attempt_")?;
        require_sha256("attempt specification", &attempt.specification_sha256)?;
        if attempt.specification_sha256 != *planned_sha256 {
            return invalid(
                "attempt outcome is detached from the preregistered specification library",
            );
        }
        if previous_attempt.is_some_and(|previous| previous >= attempt.attempt_id.as_str()) {
            return invalid("attempt identifiers must be unique and lexically sorted");
        }
        previous_attempt = Some(attempt.attempt_id.as_str());
        match &attempt.outcome {
            DictionaryAttemptOutcome::NotRun { reason }
            | DictionaryAttemptOutcome::Rejected { reason } => {
                require_reason(reason)?;
            }
            DictionaryAttemptOutcome::Completed { candidate } => {
                completed += 1;
                total_code_cells = total_code_cells
                    .checked_add(validate_candidate(
                        candidate,
                        plan,
                        request,
                        &draft.execution,
                        &environment_ids,
                        &regime_support_ids,
                        &mut ambiguities,
                    )?)
                    .ok_or_else(|| {
                        ProposalError::InvalidDictionaryContract(
                            "code-matrix work overflowed".into(),
                        )
                    })?;
            }
        }
    }
    if total_code_cells > plan.budget.max_code_cells {
        return invalid("complete code-matrix library exceeds its preregistered budget");
    }
    if completed > 1 {
        ambiguities.insert(DictionaryAmbiguity::CoordinatePermutation);
    }
    validate_actions(&draft.contract_requests, &draft.next_queries)?;

    let status = if completed == 0 {
        DictionaryProposalStatus::Abstained
    } else if draft.next_queries.is_empty() {
        DictionaryProposalStatus::Inconclusive
    } else {
        DictionaryProposalStatus::RecommendedForSeparateAudit
    };
    let mut ranked_completed_attempt_ids = draft
        .attempts
        .iter()
        .filter_map(|attempt| match &attempt.outcome {
            DictionaryAttemptOutcome::Completed { candidate } => Some((
                attempt.attempt_id.clone(),
                candidate.diagnostics.heldout_discovery_loss,
                candidate.diagnostics.description_length,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    ranked_completed_attempt_ids.sort_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| left.2.total_cmp(&right.2))
            .then_with(|| left.0.cmp(&right.0))
    });

    Ok(FrozenTransportDictionaryProposal {
        schema_version: SCHEMA_VERSION.into(),
        artifact_kind: "transport_dictionary_proposal".into(),
        proposal_id: draft.proposal_id.clone(),
        request_fingerprint,
        shift_library_fingerprint: plan.shift_library_fingerprint.clone(),
        search_plan_fingerprint: json_fingerprint(b"mic-dictionary-search-plan-v1\0", plan)?,
        execution_binding_fingerprint: json_fingerprint(
            b"mic-dictionary-execution-v1\0",
            &draft.execution,
        )?,
        candidate_library_fingerprint: attempt_library_fingerprint(&draft.attempts),
        draft_fingerprint: json_fingerprint(b"mic-transport-dictionary-draft-v1\0", draft)?,
        authority: ProposalAuthority::ProposalOnly,
        certificate_eligible: false,
        input_claims_verified: false,
        algebraic_recovery_verified: false,
        causal_family_status: CausalFamilyStatus::NotEvaluated,
        mechanism_identity: DictionaryIdentityStatus::NotEstablished,
        target_identity: DictionaryIdentityStatus::NotEstablished,
        same_target_grouping: DictionaryIdentityStatus::NotEstablished,
        edge_authority: DictionaryEdgeAuthority::None,
        selection_status: DictionarySelectionStatus::Unestablished,
        confirmation_access: DictionaryConfirmationAccess::SealedCommitmentOnly,
        status,
        reference_policy: plan.reference_policy,
        code_policy: plan.code_policy,
        ranking_rule: plan.ranking_rule,
        attempts: draft.attempts.clone(),
        ranked_completed_attempt_ids: ranked_completed_attempt_ids
            .into_iter()
            .map(|entry| entry.0)
            .collect(),
        ambiguities: ambiguities.into_iter().collect(),
        contract_requests: draft.contract_requests.clone(),
        next_queries: draft.next_queries.clone(),
        seed: request.seed,
    })
}

fn validate_plan(plan: &DictionarySearchPlan) -> Result<(), ProposalError> {
    if plan.schema_version != SCHEMA_VERSION {
        return Err(ProposalError::UnsupportedSchemaVersion(
            plan.schema_version.clone(),
        ));
    }
    require_opaque("plan_id", &plan.plan_id, "dict_plan_")?;
    for (name, value) in [
        (
            "self_driving_request_fingerprint",
            plan.self_driving_request_fingerprint.as_str(),
        ),
        (
            "shift_library_fingerprint",
            plan.shift_library_fingerprint.as_str(),
        ),
    ] {
        require_sha256(name, value)?;
    }
    if !(2..=MAX_FOLDS).contains(&plan.n_inner_folds) {
        return invalid("dictionary folds must be between 2 and 32");
    }
    if !plan.algebraic_rank_tolerance.is_finite()
        || plan.algebraic_rank_tolerance <= 0.0
        || plan.algebraic_rank_tolerance > 1e-6
    {
        return invalid("algebraic rank tolerance must be finite and in (0, 1e-6]");
    }
    if plan.rank_grid.is_empty()
        || plan
            .rank_grid
            .iter()
            .any(|rank| !(1..=MAX_FACTORS).contains(rank))
        || !plan.rank_grid.windows(2).all(|pair| pair[0] < pair[1])
    {
        return invalid("rank grid must be unique, increasing, and within 1..=64");
    }
    if plan.attempt_specification_sha256s.is_empty()
        || plan.attempt_specification_sha256s.len() > plan.budget.max_attempts
        || !plan
            .attempt_specification_sha256s
            .iter()
            .all(|value| require_sha256("attempt specification", value).is_ok())
        || !plan
            .attempt_specification_sha256s
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    {
        return invalid(
            "attempt specification library must be nonempty, sorted, unique, and within budget",
        );
    }
    let budget = &plan.budget;
    if !(1..=MAX_ATTEMPTS).contains(&budget.max_attempts)
        || !(1..=MAX_FACTORS).contains(&budget.max_factors)
        || !(1..=MAX_ENVIRONMENTS).contains(&budget.max_environments)
        || !(1..=MAX_CODE_CELLS).contains(&budget.max_code_cells)
        || !(1..=MAX_FIT_ITERATIONS).contains(&budget.max_fit_iterations_per_attempt)
    {
        return invalid("dictionary search budget exceeds a hard workspace ceiling");
    }
    Ok(())
}

fn preflight_code_cell_budget(
    attempts: &[DictionaryAttemptDraft],
    maximum: usize,
) -> Result<(), ProposalError> {
    let mut total = 0usize;
    for attempt in attempts {
        if let DictionaryAttemptOutcome::Completed { candidate } = &attempt.outcome {
            let cells = candidate
                .atoms
                .len()
                .checked_mul(candidate.codes.len())
                .ok_or_else(|| {
                    ProposalError::InvalidDictionaryContract(
                        "code-matrix size overflowed before candidate validation".into(),
                    )
                })?;
            total = total.checked_add(cells).ok_or_else(|| {
                ProposalError::InvalidDictionaryContract(
                    "code-matrix work overflowed before candidate validation".into(),
                )
            })?;
            if total > maximum {
                return invalid("complete code-matrix library exceeds its preregistered budget");
            }
        }
    }
    Ok(())
}

fn validate_execution(binding: &DictionaryExecutionBinding) -> Result<(), ProposalError> {
    for (name, value) in [
        ("fold_plan_sha256", binding.fold_plan_sha256.as_str()),
        (
            "common_cohort_sha256",
            binding.common_cohort_sha256.as_str(),
        ),
        (
            "state_representation_sha256",
            binding.state_representation_sha256.as_str(),
        ),
        (
            "reference_law_sha256",
            binding.reference_law_sha256.as_str(),
        ),
    ] {
        require_sha256(name, value)?;
    }
    if binding.n_units < 2 || binding.n_rows < binding.n_units {
        return invalid("dictionary execution requires at least two units and rows >= units");
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn validate_candidate(
    candidate: &DictionaryCandidateDraft,
    plan: &DictionarySearchPlan,
    request: &SelfDrivingRequest,
    execution: &DictionaryExecutionBinding,
    environment_ids: &BTreeSet<&str>,
    support_ids: &BTreeSet<&str>,
    ambiguities: &mut BTreeSet<DictionaryAmbiguity>,
) -> Result<usize, ProposalError> {
    validate_source(&candidate.source)?;
    if candidate.source.data_fingerprint != request.discovery_table_sha256
        || candidate.source.assignment_unit_fingerprint != request.discovery_units_sha256
        || candidate.source.fold_fingerprint != execution.fold_plan_sha256
    {
        return invalid("candidate provenance is detached from discovery data, units, or folds");
    }
    let n_atoms = candidate.atoms.len();
    if n_atoms == 0 || n_atoms > plan.budget.max_factors {
        return invalid("completed candidate atom count violates the search budget");
    }
    if !plan.rank_grid.contains(&n_atoms) {
        return invalid("completed candidate atom count was absent from the preregistered grid");
    }
    let mut previous_atom = None;
    let mut atom_artifacts = BTreeSet::new();
    for atom in &candidate.atoms {
        require_opaque("atom_id", &atom.atom_id, "atom_")?;
        if previous_atom.is_some_and(|previous| previous >= atom.atom_id.as_str()) {
            return invalid("atom identifiers must be unique and lexically sorted");
        }
        previous_atom = Some(atom.atom_id.as_str());
        require_sha256("atom artifact", &atom.artifact_sha256)?;
        if !atom_artifacts.insert(atom.artifact_sha256.as_str()) {
            return invalid("distinct atom identifiers cannot reference the same artifact bytes");
        }
        if atom.artifact_bytes == 0 || atom.media_type != "application/vnd.mic.transport-atom+json"
        {
            return invalid("atom artifact length or media type is invalid");
        }
        if !support_ids.contains(atom.support_id.as_str()) {
            return invalid("atom refers to a missing or non-regime-information support");
        }
    }
    let n_environments = candidate.codes.len();
    if n_environments == 0 || n_environments > plan.budget.max_environments {
        return invalid("completed candidate environment count violates the search budget");
    }
    let mut previous_environment = None;
    let mut candidate_environment_ids = BTreeSet::new();
    let mut binary_codes = BTreeSet::new();
    for row in &candidate.codes {
        require_opaque("environment_id", &row.environment_id, "env_")?;
        if previous_environment.is_some_and(|previous| previous >= row.environment_id.as_str()) {
            return invalid("code rows must be unique and lexically sorted by environment");
        }
        previous_environment = Some(row.environment_id.as_str());
        if !environment_ids.contains(row.environment_id.as_str()) {
            return invalid("code row refers to an environment absent from the shift library");
        }
        candidate_environment_ids.insert(row.environment_id.as_str());
        if row.coefficients.len() != n_atoms
            || row.coefficients.iter().any(|value| !value.is_finite())
        {
            return invalid("code row width or coefficient finiteness is invalid");
        }
        if row
            .coefficients
            .iter()
            .all(|value| *value == 0.0 || *value == 1.0)
        {
            binary_codes.insert(
                row.coefficients
                    .iter()
                    .map(|value| u8::from(*value == 1.0))
                    .collect::<Vec<_>>(),
            );
        }
    }
    if candidate_environment_ids != *environment_ids {
        return invalid("completed candidate must cover the complete frozen environment library");
    }
    for value in [
        candidate.diagnostics.training_reconstruction_loss,
        candidate.diagnostics.heldout_discovery_loss,
        candidate.diagnostics.description_length,
    ] {
        if !value.is_finite() || value < 0.0 {
            return invalid("dictionary diagnostic must be finite and nonnegative");
        }
    }
    if candidate.diagnostics.fit_iterations > plan.budget.max_fit_iterations_per_attempt {
        return invalid("fit iterations exceed the preregistered budget");
    }

    let code_matrix: Vec<Vec<f64>> = candidate
        .codes
        .iter()
        .map(|row| row.coefficients.clone())
        .collect();
    let code_rank = matrix_rank(&code_matrix, plan.algebraic_rank_tolerance);

    match &candidate.algebraic_case {
        AlgebraicRecoveryCase::KnownCodes {
            observed_design_rank,
        } => {
            if plan.code_policy != DictionaryCodePolicy::KnownIncidence {
                return invalid("known-code case conflicts with the preregistered code policy");
            }
            let augmented = if plan.reference_policy
                == DictionaryReferencePolicy::ArbitraryReferenceWithIntercept
            {
                code_matrix
                    .iter()
                    .map(|row| {
                        let mut augmented = Vec::with_capacity(row.len() + 1);
                        augmented.push(1.0);
                        augmented.extend(row);
                        augmented
                    })
                    .collect::<Vec<_>>()
            } else {
                code_matrix.clone()
            };
            let computed_rank = matrix_rank(&augmented, plan.algebraic_rank_tolerance);
            let required = augmented.first().map_or(0, Vec::len);
            if *observed_design_rank != computed_rank || computed_rank != required {
                return invalid("known-code design lacks the required full rank");
            }
        }
        AlgebraicRecoveryCase::MarkedPureAnchors {
            baseline_environment_id,
            anchor_environment_ids,
            observed_anchor_rank,
            amplitudes_calibrated,
            anchor_labels_assigned,
        } => {
            if plan.code_policy != DictionaryCodePolicy::MarkedPureAnchors
                || anchor_environment_ids.len() != n_atoms
                || *observed_anchor_rank != n_atoms
                || code_rank != n_atoms
            {
                return invalid("pure-anchor case lacks full-rank anchor differences");
            }
            require_anchor_codes(
                &candidate.codes,
                baseline_environment_id,
                anchor_environment_ids,
                n_atoms,
            )?;
            if !amplitudes_calibrated {
                ambiguities.insert(DictionaryAmbiguity::ConventionalScale);
            }
            if !anchor_labels_assigned {
                ambiguities.insert(DictionaryAmbiguity::CoordinatePermutation);
            }
        }
        AlgebraicRecoveryCase::CompleteBinaryCube {
            observed_edge_rank,
            marked_zero,
            edge_function_independence_verified,
        } => {
            if plan.code_policy != DictionaryCodePolicy::CompleteBinaryCube
                || *observed_edge_rank != n_atoms
            {
                return invalid("complete-cube case lacks full-rank edge functions");
            }
            let expected = 1usize
                .checked_shl(u32::try_from(n_atoms).map_err(|_| {
                    ProposalError::InvalidDictionaryContract("cube dimension overflowed".into())
                })?)
                .ok_or_else(|| {
                    ProposalError::InvalidDictionaryContract("cube dimension overflowed".into())
                })?;
            if n_environments != expected || binary_codes.len() != expected {
                return invalid("complete binary cube is missing or duplicating vertices");
            }
            if plan.reference_policy == DictionaryReferencePolicy::MarkedZeroCode && !marked_zero {
                return invalid("marked-zero reference contradicts an unmarked cube origin");
            }
            if *edge_function_independence_verified {
                return invalid("proposal layer cannot verify external atom-function independence");
            }
            ambiguities.insert(DictionaryAmbiguity::ExternalAtomIndependenceUnverified);
            ambiguities.insert(DictionaryAmbiguity::CoordinatePermutation);
            if !marked_zero {
                ambiguities.insert(DictionaryAmbiguity::BitComplement);
            }
        }
        AlgebraicRecoveryCase::IncompleteUnknownCodes {
            observed_design_rank,
        } => {
            if plan.code_policy != DictionaryCodePolicy::UnknownSparseCodes {
                return invalid("unknown-code case conflicts with the preregistered code policy");
            }
            if *observed_design_rank != code_rank {
                return invalid(
                    "unknown-code claimed rank disagrees with the displayed code matrix",
                );
            }
            ambiguities.insert(DictionaryAmbiguity::GeneralLinearMixing);
            if code_rank < n_atoms {
                ambiguities.insert(DictionaryAmbiguity::RankDegeneracy);
            }
        }
        AlgebraicRecoveryCase::RankDegenerate {
            observed_design_rank,
        } => {
            if *observed_design_rank != code_rank || code_rank >= n_atoms {
                return invalid("rank-degenerate case contradicts the displayed code matrix");
            }
            ambiguities.insert(DictionaryAmbiguity::RankDegeneracy);
            ambiguities.insert(DictionaryAmbiguity::GeneralLinearMixing);
        }
    }
    if candidate.support_aliasing_detected {
        ambiguities.insert(DictionaryAmbiguity::AlmostEverywhereSupportAliasing);
    }
    n_atoms.checked_mul(n_environments).ok_or_else(|| {
        ProposalError::InvalidDictionaryContract("code-matrix size overflowed".into())
    })
}

fn validate_source(source: &ProposalSource) -> Result<(), ProposalError> {
    for (name, value) in [
        ("adapter_id", source.adapter_id.as_str()),
        ("adapter_revision", source.adapter_revision.as_str()),
        ("model_family", source.model_family.as_str()),
        ("data_fingerprint", source.data_fingerprint.as_str()),
        (
            "assignment_unit_fingerprint",
            source.assignment_unit_fingerprint.as_str(),
        ),
        ("fold_fingerprint", source.fold_fingerprint.as_str()),
        (
            "hyperparameter_policy",
            source.hyperparameter_policy.as_str(),
        ),
    ] {
        require_text(name, value)?;
    }
    if !source
        .feature_flags
        .windows(2)
        .all(|pair| pair[0] < pair[1])
        || source
            .feature_flags
            .iter()
            .any(|flag| flag.trim().is_empty())
    {
        return Err(ProposalError::NonCanonicalFeatureFlags);
    }
    Ok(())
}

fn require_anchor_codes(
    rows: &[EnvironmentCodeRow],
    baseline_environment_id: &str,
    anchor_environment_ids: &[String],
    n_atoms: usize,
) -> Result<(), ProposalError> {
    require_opaque(
        "anchor baseline_environment_id",
        baseline_environment_id,
        "env_",
    )?;
    if anchor_environment_ids.len() != n_atoms
        || !anchor_environment_ids
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    {
        return invalid("pure-anchor environment identifiers must be unique and sorted");
    }
    let baseline = rows
        .iter()
        .find(|row| row.environment_id == baseline_environment_id)
        .ok_or_else(|| {
            ProposalError::InvalidDictionaryContract(
                "pure-anchor baseline environment is absent".into(),
            )
        })?;
    if baseline.coefficients.iter().any(|value| *value != 0.0) {
        return invalid("pure-anchor baseline code must be all zero");
    }
    for (index, identifier) in anchor_environment_ids.iter().enumerate() {
        require_opaque("anchor_environment_id", identifier, "env_")?;
        let row = rows
            .iter()
            .find(|row| row.environment_id == *identifier)
            .ok_or_else(|| {
                ProposalError::InvalidDictionaryContract("pure-anchor environment is absent".into())
            })?;
        if row
            .coefficients
            .iter()
            .enumerate()
            .any(|(column, value)| *value != f64::from(column == index))
        {
            return invalid("pure-anchor code rows must be the canonical basis vectors");
        }
    }
    Ok(())
}

fn matrix_rank(matrix: &[Vec<f64>], tolerance: f64) -> usize {
    if matrix.is_empty() || matrix[0].is_empty() {
        return 0;
    }
    let mut work = matrix.to_vec();
    let rows = work.len();
    let columns = work[0].len();
    let mut rank = 0usize;
    for column in 0..columns {
        let pivot = (rank..rows).max_by(|left, right| {
            work[*left][column]
                .abs()
                .total_cmp(&work[*right][column].abs())
        });
        let Some(pivot) = pivot.filter(|pivot| work[*pivot][column].abs() > tolerance) else {
            continue;
        };
        work.swap(rank, pivot);
        let pivot_value = work[rank][column];
        for value in &mut work[rank][column..] {
            *value /= pivot_value;
        }
        let pivot_tail = work[rank][column..].to_vec();
        for (row_index, row) in work.iter_mut().enumerate() {
            if row_index == rank {
                continue;
            }
            let factor = row[column];
            if factor.abs() <= tolerance {
                continue;
            }
            for (value, pivot_value) in row[column..].iter_mut().zip(&pivot_tail) {
                *value -= factor * pivot_value;
            }
        }
        rank += 1;
        if rank == rows {
            break;
        }
    }
    rank
}

fn validate_actions(
    contracts: &[ContractRequest],
    queries: &[NextQuery],
) -> Result<(), ProposalError> {
    let mut contract_ids = BTreeSet::new();
    for contract in contracts {
        require_opaque("contract request", &contract.request_id, "contract_")?;
        if !contract_ids.insert(contract.request_id.as_str())
            || !contract.priority.is_finite()
            || contract.priority < 0.0
        {
            return invalid("contract requests must be unique with finite nonnegative priority");
        }
        require_opaque("contract required_for", &contract.required_for, "strategy_")?;
        require_text("contract detail", &contract.detail)?;
    }
    let mut query_ids = BTreeSet::new();
    let mut previous_query: Option<(f64, &str)> = None;
    for query in queries {
        require_opaque("next query", &query.query_id, "query_")?;
        if !query_ids.insert(query.query_id.as_str())
            || !query.priority.is_finite()
            || query.priority < 0.0
        {
            return invalid("next queries must be unique with finite nonnegative priority");
        }
        require_text("priority semantics", &query.priority_semantics)?;
        if query.separates_hypotheses.is_empty()
            || !query
                .separates_hypotheses
                .windows(2)
                .all(|pair| pair[0] < pair[1])
            || query
                .separates_hypotheses
                .iter()
                .any(|identifier| require_opaque("hypothesis", identifier, "hyp_").is_err())
            || !query
                .contract_request_ids
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        {
            return invalid("query hypothesis and contract identifiers must be canonical");
        }
        if query
            .contract_request_ids
            .iter()
            .any(|identifier| !contract_ids.contains(identifier.as_str()))
        {
            return invalid("next query refers to an unknown contract request");
        }
        if previous_query.is_some_and(|(priority, identifier)| {
            priority < query.priority
                || (priority.total_cmp(&query.priority).is_eq()
                    && identifier >= query.query_id.as_str())
        }) {
            return invalid("next queries must be ranked by descending priority then identifier");
        }
        previous_query = Some((query.priority, query.query_id.as_str()));
    }
    Ok(())
}

fn attempt_library_fingerprint(attempts: &[DictionaryAttemptDraft]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"mic-transport-dictionary-library-v1\0");
    hash_usize(&mut digest, attempts.len());
    for attempt in attempts {
        hash_string(&mut digest, &attempt.attempt_id);
        hash_string(&mut digest, &attempt.specification_sha256);
        match &attempt.outcome {
            DictionaryAttemptOutcome::NotRun { reason } => {
                digest.update([0]);
                hash_string(&mut digest, reason);
            }
            DictionaryAttemptOutcome::Rejected { reason } => {
                digest.update([1]);
                hash_string(&mut digest, reason);
            }
            DictionaryAttemptOutcome::Completed { candidate } => {
                digest.update([2]);
                hash_candidate(&mut digest, candidate);
            }
        }
    }
    digest_hex(digest)
}

fn hash_candidate(digest: &mut Sha256, candidate: &DictionaryCandidateDraft) {
    hash_string(digest, &candidate.source.adapter_id);
    hash_string(digest, &candidate.source.adapter_revision);
    digest.update([match candidate.source.source_kind {
        super::ProposalSourceKind::PreregisteredDomainModel => 0,
        super::ProposalSourceKind::PassiveLearner => 1,
        super::ProposalSourceKind::PreviousIndependentAudit => 2,
        super::ProposalSourceKind::ExploratorySearch => 3,
    }]);
    hash_string(digest, &candidate.source.model_family);
    hash_string(digest, &candidate.source.data_fingerprint);
    hash_string(digest, &candidate.source.assignment_unit_fingerprint);
    hash_string(digest, &candidate.source.fold_fingerprint);
    hash_string(digest, &candidate.source.hyperparameter_policy);
    hash_usize(digest, candidate.source.feature_flags.len());
    for flag in &candidate.source.feature_flags {
        hash_string(digest, flag);
    }
    hash_string(
        digest,
        &serde_json::to_string(&candidate.algebraic_case).expect("enum serialization cannot fail"),
    );
    hash_usize(digest, candidate.atoms.len());
    for atom in &candidate.atoms {
        hash_string(digest, &atom.atom_id);
        hash_string(digest, &atom.artifact_sha256);
        digest.update(atom.artifact_bytes.to_le_bytes());
        hash_string(digest, &atom.media_type);
        hash_string(digest, &atom.support_id);
    }
    hash_usize(digest, candidate.codes.len());
    for row in &candidate.codes {
        hash_string(digest, &row.environment_id);
        hash_usize(digest, row.coefficients.len());
        for value in &row.coefficients {
            digest.update(value.to_bits().to_le_bytes());
        }
    }
    digest.update(
        candidate
            .diagnostics
            .training_reconstruction_loss
            .to_bits()
            .to_le_bytes(),
    );
    digest.update(
        candidate
            .diagnostics
            .heldout_discovery_loss
            .to_bits()
            .to_le_bytes(),
    );
    digest.update(
        candidate
            .diagnostics
            .description_length
            .to_bits()
            .to_le_bytes(),
    );
    hash_usize(digest, candidate.diagnostics.fit_iterations);
    digest.update([u8::from(candidate.support_aliasing_detected)]);
}

fn json_fingerprint<T: Serialize>(domain: &[u8], value: &T) -> Result<String, ProposalError> {
    let encoded = serde_json::to_vec(value).map_err(|error| {
        ProposalError::InvalidDictionaryContract(format!(
            "dictionary fingerprint serialization failed: {error}"
        ))
    })?;
    let mut digest = Sha256::new();
    digest.update(domain);
    hash_usize(&mut digest, encoded.len());
    digest.update(encoded);
    Ok(digest_hex(digest))
}

fn digest_hex(digest: Sha256) -> String {
    let bytes = digest.finalize();
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing hex into a String cannot fail");
    }
    encoded
}

fn hash_string(digest: &mut Sha256, value: &str) {
    hash_usize(digest, value.len());
    digest.update(value.as_bytes());
}

fn hash_usize(digest: &mut Sha256, value: usize) {
    let value = u64::try_from(value).expect("usize always fits in u64 on supported targets");
    digest.update(value.to_le_bytes());
}

fn require_sha256(name: &str, value: &str) -> Result<(), ProposalError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        invalid(&format!("{name} must be a lowercase SHA-256"))
    }
}

fn require_opaque(name: &str, value: &str, prefix: &str) -> Result<(), ProposalError> {
    require_text(name, value)?;
    if value.starts_with(prefix)
        && value[prefix.len()..].len() >= 3
        && value[prefix.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_digit())
    {
        Ok(())
    } else {
        invalid(&format!("{name} must be an opaque {prefix}NNN identifier"))
    }
}

fn require_text(name: &str, value: &str) -> Result<(), ProposalError> {
    if value.trim().is_empty() || value.chars().count() > MAX_IDENTIFIER_CHARS {
        invalid(&format!(
            "{name} must be nonempty and at most 1024 characters"
        ))
    } else {
        Ok(())
    }
}

fn require_reason(reason: &str) -> Result<(), ProposalError> {
    require_text("attempt reason", reason)?;
    if reason
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        Ok(())
    } else {
        invalid("attempt reason must be a stable lowercase snake-case code")
    }
}

fn invalid<T>(detail: &str) -> Result<T, ProposalError> {
    Err(ProposalError::InvalidDictionaryContract(detail.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CandidateEnvironment, CandidateSupport, ContractRequestKind, DiscoveryAccess,
        IsolationClaim, NextQueryKind, PartitionClaim, ProposalSourceKind, ShiftFactorizationDraft,
        StrategyEligibility, SupportRelation, SupportSemantics, UnitBasis, UnitContract,
    };
    use std::collections::BTreeMap;

    fn digest(fill: char) -> String {
        std::iter::repeat_n(fill, 64).collect()
    }

    fn request() -> SelfDrivingRequest {
        SelfDrivingRequest {
            schema_version: "1.0.0".into(),
            request_id: "request_001".into(),
            discovery_table_sha256: digest('a'),
            transformation_sha256: digest('b'),
            discovery_units_sha256: digest('c'),
            partition_claim: PartitionClaim {
                claim_id: "claim_001".into(),
                claim_sha256: digest('d'),
                total_units: 20,
                discovery_units: 12,
                confirmation_units: 8,
                declared_disjoint: true,
                declared_exhaustive: true,
            },
            unit_declaration: UnitContract {
                column: "u_001".into(),
                basis: UnitBasis::DeclaredAssignmentUnit,
                evidence_ref: Some("evidence_001".into()),
            },
            seed: 41,
            split_algorithm: "sha256_cluster_v1".into(),
            candidate_enumeration_policy: "lexical_complete_before_budget".into(),
            candidate_budget: 100,
            common_cohort_policy: "intersection_before_scoring".into(),
            equivalence_tolerance: 0.1,
            detection_floor: 0.05,
            learner_families: vec!["linear".into()],
            isolation_claim: IsolationClaim {
                source_url: DiscoveryAccess::Unavailable,
                study_title: DiscoveryAccess::Unavailable,
                file_names: DiscoveryAccess::Unavailable,
                codebook_labels: DiscoveryAccess::Unavailable,
                paper: DiscoveryAccess::Unavailable,
                replication_scripts: DiscoveryAccess::Unavailable,
                published_results: DiscoveryAccess::Unavailable,
                confirmation_outcomes: DiscoveryAccess::Unavailable,
                oracle: DiscoveryAccess::Unavailable,
                network: DiscoveryAccess::Unavailable,
            },
        }
    }

    fn shift_draft() -> ShiftFactorizationDraft {
        ShiftFactorizationDraft {
            proposal_id: "proposal_001".into(),
            environments: (0..3)
                .map(|index| CandidateEnvironment {
                    environment_id: format!("env_{:03}", index + 1),
                    defining_columns: vec!["c_001".into()],
                    transform_sha256: digest('e'),
                    score: 1.0,
                    score_semantics: "held-out regime prediction gain".into(),
                })
                .collect(),
            supports: vec![CandidateSupport {
                support_id: "support_001".into(),
                environment_id: "env_001".into(),
                semantics: SupportSemantics::RegimeInformationSupport,
                variables: vec!["c_001".into()],
                learner_family: "linear".into(),
                discovery_fold: "fold_001".into(),
                score: 0.1,
                score_semantics: "held-out loss; lower is better".into(),
                on_parsimony_frontier: true,
            }],
            support_relations: Vec::<SupportRelation>::new(),
            strategy_eligibility: BTreeMap::<String, StrategyEligibility>::new(),
            contract_requests: Vec::new(),
            next_queries: Vec::new(),
        }
    }

    fn source() -> ProposalSource {
        ProposalSource {
            adapter_id: "dictionary_linear_v1".into(),
            adapter_revision: digest('1'),
            source_kind: ProposalSourceKind::ExploratorySearch,
            model_family: "fixed_transport_fixture".into(),
            data_fingerprint: digest('a'),
            assignment_unit_fingerprint: digest('c'),
            fold_fingerprint: digest('5'),
            hyperparameter_policy: "fixed_fixture".into(),
            feature_flags: vec![],
        }
    }

    fn atom(identifier: &str, fill: char) -> TransportAtomDraft {
        TransportAtomDraft {
            atom_id: identifier.into(),
            artifact_sha256: digest(fill),
            artifact_bytes: 128,
            media_type: "application/vnd.mic.transport-atom+json".into(),
            support_id: "support_001".into(),
        }
    }

    fn plan(request: &SelfDrivingRequest, shift: &ShiftFactorizationDraft) -> DictionarySearchPlan {
        let shift = freeze_shift_factorization_proposal(request, shift).expect("shift freezes");
        DictionarySearchPlan {
            schema_version: "1.0.0".into(),
            plan_id: "dict_plan_001".into(),
            self_driving_request_fingerprint: request.fingerprint().expect("request fingerprint"),
            shift_library_fingerprint: shift.candidate_library_fingerprint().into(),
            reference_policy: DictionaryReferencePolicy::MarkedZeroCode,
            code_policy: DictionaryCodePolicy::UnknownSparseCodes,
            n_inner_folds: 2,
            algebraic_rank_tolerance: 1e-10,
            rank_grid: vec![2],
            attempt_specification_sha256s: vec![digest('9'), digest('c')],
            ranking_rule: DictionaryRankingRule::HeldoutLossThenDescriptionLength,
            budget: DictionarySearchBudget {
                max_attempts: 10,
                max_factors: 4,
                max_environments: 8,
                max_code_cells: 100,
                max_fit_iterations_per_attempt: 100,
            },
        }
    }

    fn candidate(atoms: Vec<TransportAtomDraft>, codes: Vec<Vec<f64>>) -> DictionaryCandidateDraft {
        DictionaryCandidateDraft {
            source: source(),
            algebraic_case: AlgebraicRecoveryCase::IncompleteUnknownCodes {
                observed_design_rank: 2,
            },
            atoms,
            codes: codes
                .into_iter()
                .enumerate()
                .map(|(index, coefficients)| EnvironmentCodeRow {
                    environment_id: format!("env_{:03}", index + 1),
                    coefficients,
                })
                .collect(),
            diagnostics: DictionaryFitDiagnostics {
                training_reconstruction_loss: 0.0,
                heldout_discovery_loss: 0.0,
                description_length: 2.0,
                fit_iterations: 1,
            },
            support_aliasing_detected: false,
        }
    }

    fn execution() -> DictionaryExecutionBinding {
        DictionaryExecutionBinding {
            fold_plan_sha256: digest('5'),
            common_cohort_sha256: digest('6'),
            state_representation_sha256: digest('7'),
            reference_law_sha256: digest('8'),
            n_units: 12,
            n_rows: 24,
        }
    }

    #[test]
    fn partial_cube_shear_retains_general_linear_ambiguity_and_no_causal_identity() {
        let request = request();
        let shift = shift_draft();
        let plan = plan(&request, &shift);
        // {0,f,g}: (f,g) with codes 00,10,01 and (f,g-f) with 00,10,11.
        let draft = TransportDictionaryDraft {
            proposal_id: "dictionary_001".into(),
            execution: execution(),
            attempts: vec![
                DictionaryAttemptDraft {
                    attempt_id: "attempt_001".into(),
                    specification_sha256: digest('9'),
                    outcome: DictionaryAttemptOutcome::Completed {
                        candidate: Box::new(candidate(
                            vec![atom("atom_001", 'a'), atom("atom_002", 'b')],
                            vec![vec![0.0, 0.0], vec![1.0, 0.0], vec![0.0, 1.0]],
                        )),
                    },
                },
                DictionaryAttemptDraft {
                    attempt_id: "attempt_002".into(),
                    specification_sha256: digest('c'),
                    outcome: DictionaryAttemptOutcome::Completed {
                        candidate: Box::new(candidate(
                            vec![atom("atom_001", 'a'), atom("atom_002", 'd')],
                            vec![vec![0.0, 0.0], vec![1.0, 0.0], vec![1.0, 1.0]],
                        )),
                    },
                },
            ],
            contract_requests: vec![ContractRequest {
                request_id: "contract_001".into(),
                kind: ContractRequestKind::SameTargetGrouping,
                required_for: "strategy_001".into(),
                detail: "external grouping remains unresolved".into(),
                priority: 1.0,
            }],
            next_queries: vec![NextQuery {
                query_id: "query_001".into(),
                kind: NextQueryKind::CollectMissingCorner,
                separates_hypotheses: vec!["hyp_001".into(), "hyp_002".into()],
                contract_request_ids: vec!["contract_001".into()],
                priority: 1.0,
                priority_semantics: "observe the law on which the shear-related completions differ"
                    .into(),
            }],
        };

        let frozen = freeze_transport_dictionary_proposal(&request, &shift, &plan, &draft)
            .expect("exact shear witness freezes as a proposal");
        assert_eq!(frozen.authority(), ProposalAuthority::ProposalOnly);
        assert!(!frozen.certificate_eligible());
        assert_eq!(
            frozen.causal_family_status(),
            CausalFamilyStatus::NotEvaluated
        );
        assert!(
            frozen
                .ambiguities()
                .contains(&DictionaryAmbiguity::GeneralLinearMixing)
        );
        let value = serde_json::to_value(frozen).expect("serialize proposal");
        assert_eq!(value["mechanism_identity"], "not_established");
        assert_eq!(value["target_identity"], "not_established");
        assert_eq!(value["same_target_grouping"], "not_established");
        assert_eq!(value["edge_authority"], "none");
        assert_eq!(value["selection_status"], "unestablished");
        assert_eq!(value["confirmation_access"], "sealed_commitment_only");
    }

    #[test]
    fn rejected_attempt_is_bound_into_the_library_fingerprint() {
        let request = request();
        let shift = shift_draft();
        let mut plan = plan(&request, &shift);
        plan.attempt_specification_sha256s = vec![digest('9')];
        let mut draft = TransportDictionaryDraft {
            proposal_id: "dictionary_001".into(),
            execution: execution(),
            attempts: vec![DictionaryAttemptDraft {
                attempt_id: "attempt_001".into(),
                specification_sha256: digest('9'),
                outcome: DictionaryAttemptOutcome::Rejected {
                    reason: "nonconverged".into(),
                },
            }],
            contract_requests: vec![],
            next_queries: vec![],
        };
        let first = freeze_transport_dictionary_proposal(&request, &shift, &plan, &draft)
            .expect("rejected library remains auditable");
        draft.attempts[0].outcome = DictionaryAttemptOutcome::Rejected {
            reason: "support_failure".into(),
        };
        let second = freeze_transport_dictionary_proposal(&request, &shift, &plan, &draft)
            .expect("changed rejected library remains auditable");
        assert_ne!(
            first.candidate_library_fingerprint(),
            second.candidate_library_fingerprint()
        );
    }

    #[test]
    fn arbitrary_reference_requires_augmented_known_code_rank() {
        let request = request();
        let shift = shift_draft();
        let mut plan = plan(&request, &shift);
        plan.reference_policy = DictionaryReferencePolicy::ArbitraryReferenceWithIntercept;
        plan.code_policy = DictionaryCodePolicy::KnownIncidence;
        plan.attempt_specification_sha256s = vec![digest('9')];
        let mut completed = candidate(
            vec![atom("atom_001", 'a'), atom("atom_002", 'b')],
            vec![vec![0.0, 0.0], vec![1.0, 0.0], vec![0.0, 1.0]],
        );
        completed.algebraic_case = AlgebraicRecoveryCase::KnownCodes {
            observed_design_rank: 2,
        };
        let draft = TransportDictionaryDraft {
            proposal_id: "dictionary_001".into(),
            execution: execution(),
            attempts: vec![DictionaryAttemptDraft {
                attempt_id: "attempt_001".into(),
                specification_sha256: digest('9'),
                outcome: DictionaryAttemptOutcome::Completed {
                    candidate: Box::new(completed),
                },
            }],
            contract_requests: vec![],
            next_queries: vec![],
        };
        assert!(freeze_transport_dictionary_proposal(&request, &shift, &plan, &draft).is_err());
    }

    #[test]
    fn complete_cube_requires_every_distinct_binary_vertex() {
        let request = request();
        let shift = shift_draft();
        let mut plan = plan(&request, &shift);
        plan.code_policy = DictionaryCodePolicy::CompleteBinaryCube;
        plan.attempt_specification_sha256s = vec![digest('9')];
        let mut incomplete = candidate(
            vec![atom("atom_001", 'a'), atom("atom_002", 'b')],
            vec![vec![0.0, 0.0], vec![1.0, 0.0], vec![0.0, 1.0]],
        );
        incomplete.algebraic_case = AlgebraicRecoveryCase::CompleteBinaryCube {
            observed_edge_rank: 2,
            marked_zero: true,
            edge_function_independence_verified: false,
        };
        let draft = TransportDictionaryDraft {
            proposal_id: "dictionary_001".into(),
            execution: execution(),
            attempts: vec![DictionaryAttemptDraft {
                attempt_id: "attempt_001".into(),
                specification_sha256: digest('9'),
                outcome: DictionaryAttemptOutcome::Completed {
                    candidate: Box::new(incomplete),
                },
            }],
            contract_requests: vec![],
            next_queries: vec![],
        };
        assert!(freeze_transport_dictionary_proposal(&request, &shift, &plan, &draft).is_err());
    }

    #[test]
    fn library_fingerprint_uses_fixed_width_framing() {
        let fingerprint = attempt_library_fingerprint(&[]);
        assert_eq!(fingerprint.len(), 64);
    }
}
