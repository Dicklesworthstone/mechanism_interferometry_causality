#![forbid(unsafe_code)]
//! Proposal-only contracts for many-environment shift discovery.
//!
//! These types deliberately do not depend on `mic-audit`, `mic-design`, or
//! `mic-stats` verdict types. A frozen scout artifact may order later work; it
//! cannot be converted into a certificate gate, target, edge, or invariant.

use super::{ProposalAuthority, ProposalError};
use core::fmt::Write as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const SCHEMA_VERSION: &str = "1.0.0";

/// Why a declared unit column may be used for splitting and descriptive scores.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnitBasis {
    /// External design evidence says this is the assignment unit.
    DeclaredAssignmentUnit,
    /// External time or spatial evidence says this is the dependence block.
    DeclaredDependenceBlock,
    /// The column merely looks identifier-like; no calibrated inference is allowed.
    UnverifiedIdentifier,
    /// Each row is provisionally its own unit; no calibrated inference is allowed.
    Row,
}

/// Unit information visible to the discovery process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UnitContract {
    /// Neutralized unit-column identifier.
    pub column: String,
    /// Basis for treating rows with the same value as dependent.
    pub basis: UnitBasis,
    /// Optional content-addressed external receipt for the basis.
    pub evidence_ref: Option<String>,
}

/// Content binding for an upstream audit of the outer unit partition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PartitionReceipt {
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// SHA-256 of the partition receipt.
    pub receipt_sha256: String,
    /// Total number of distinct units.
    pub total_units: usize,
    /// Number of discovery units.
    pub discovery_units: usize,
    /// Number of sealed confirmation units.
    pub confirmation_units: usize,
    /// Whether the receipt's validator established disjointness.
    pub disjoint: bool,
    /// Whether the receipt's validator established exhaustive coverage.
    pub exhaustive: bool,
}

/// Availability of a resource to the discovery process.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryAccess {
    /// The resource is absent from the discovery mount.
    Unavailable,
    /// The resource is visible; strict discovery must refuse.
    Available,
}

/// Context that must be absent from the router's discovery mount.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SealedContext {
    /// Source URL access.
    pub source_url: DiscoveryAccess,
    /// Study-title access.
    pub study_title: DiscoveryAccess,
    /// Original-file-name access.
    pub file_names: DiscoveryAccess,
    /// Semantic-codebook access.
    pub codebook_labels: DiscoveryAccess,
    /// Study-paper access.
    pub paper: DiscoveryAccess,
    /// Replication-script access.
    pub replication_scripts: DiscoveryAccess,
    /// Published-result access.
    pub published_results: DiscoveryAccess,
    /// Confirmation-outcome access.
    pub confirmation_outcomes: DiscoveryAccess,
    /// Oracle access.
    pub oracle: DiscoveryAccess,
    /// Network access.
    pub network: DiscoveryAccess,
}

impl SealedContext {
    fn is_sealed(&self) -> bool {
        [
            self.source_url,
            self.study_title,
            self.file_names,
            self.codebook_labels,
            self.paper,
            self.replication_scripts,
            self.published_results,
            self.confirmation_outcomes,
            self.oracle,
            self.network,
        ]
        .into_iter()
        .all(|access| access == DiscoveryAccess::Unavailable)
    }
}

/// Immutable request for discovery-only shift scouting.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SelfDrivingRequest {
    /// Request schema version. Currently `1.0.0`.
    pub schema_version: String,
    /// Stable neutral request identifier.
    pub request_id: String,
    /// SHA-256 of the immutable source table.
    pub source_table_sha256: String,
    /// SHA-256 of the neutralized discovery table.
    pub discovery_table_sha256: String,
    /// SHA-256 of the sealed confirmation table.
    pub confirmation_table_sha256: String,
    /// SHA-256 of the transformation from source to neutralized views.
    pub transformation_sha256: String,
    /// SHA-256 of the ordered discovery-unit list.
    pub discovery_units_sha256: String,
    /// SHA-256 of the ordered confirmation-unit list.
    pub confirmation_units_sha256: String,
    /// Content-bound audit of the outer unit partition.
    pub partition: PartitionReceipt,
    /// Unit used before any environment, transform, support, or model search.
    pub unit: UnitContract,
    /// Caller-supplied seed independent of outcome bytes.
    pub seed: u64,
    /// Fixed fold algorithm identifier.
    pub split_algorithm: String,
    /// Complete candidate enumeration policy.
    pub candidate_enumeration_policy: String,
    /// Maximum number of candidates; enumeration order must not alter membership.
    pub candidate_budget: usize,
    /// Policy that fixes the cohort shared by all candidate comparisons.
    pub common_cohort_policy: String,
    /// Equivalence tolerance for later confirmation, not used as a discovery p-value.
    pub equivalence_tolerance: f64,
    /// Minimum full-change magnitude required before a later orientation audit.
    pub detection_floor: f64,
    /// Learner families to run as a disagreement battery.
    pub learner_families: Vec<String>,
    /// Context withheld from discovery.
    pub sealed_context: SealedContext,
}

impl SelfDrivingRequest {
    /// Validates only the proposal/isolation contract. It does not establish the
    /// truth of any external receipt.
    pub fn validate(&self) -> Result<(), ProposalError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ProposalError::UnsupportedSchemaVersion(
                self.schema_version.clone(),
            ));
        }
        for (name, value) in [
            ("request_id", self.request_id.as_str()),
            ("split_algorithm", self.split_algorithm.as_str()),
            (
                "candidate_enumeration_policy",
                self.candidate_enumeration_policy.as_str(),
            ),
            ("common_cohort_policy", self.common_cohort_policy.as_str()),
            ("unit.column", self.unit.column.as_str()),
            ("partition.receipt_id", self.partition.receipt_id.as_str()),
        ] {
            require_nonempty(name, value)?;
        }
        for (name, value) in [
            ("source_table_sha256", self.source_table_sha256.as_str()),
            (
                "discovery_table_sha256",
                self.discovery_table_sha256.as_str(),
            ),
            (
                "confirmation_table_sha256",
                self.confirmation_table_sha256.as_str(),
            ),
            ("transformation_sha256", self.transformation_sha256.as_str()),
            (
                "discovery_units_sha256",
                self.discovery_units_sha256.as_str(),
            ),
            (
                "confirmation_units_sha256",
                self.confirmation_units_sha256.as_str(),
            ),
            (
                "partition.receipt_sha256",
                self.partition.receipt_sha256.as_str(),
            ),
        ] {
            require_sha256(name, value)?;
        }
        if self.candidate_budget == 0 {
            return Err(ProposalError::InvalidScoutContract(
                "candidate_budget must be positive".into(),
            ));
        }
        if !self.equivalence_tolerance.is_finite() || self.equivalence_tolerance <= 0.0 {
            return Err(ProposalError::InvalidScoutContract(
                "equivalence_tolerance must be finite and positive".into(),
            ));
        }
        if !self.detection_floor.is_finite() || self.detection_floor <= 0.0 {
            return Err(ProposalError::InvalidScoutContract(
                "detection_floor must be finite and positive".into(),
            ));
        }
        if self.partition.total_units == 0
            || self.partition.discovery_units == 0
            || self.partition.confirmation_units == 0
            || self.partition.discovery_units + self.partition.confirmation_units
                != self.partition.total_units
            || !self.partition.disjoint
            || !self.partition.exhaustive
        {
            return Err(ProposalError::InvalidScoutContract(
                "partition receipt must describe positive, disjoint, exhaustive discovery and confirmation units".into(),
            ));
        }
        if !self.sealed_context.is_sealed() {
            return Err(ProposalError::InvalidScoutContract(
                "discovery context exposes sealed study, confirmation, oracle, or network information"
                    .into(),
            ));
        }
        if matches!(
            self.unit.basis,
            UnitBasis::DeclaredAssignmentUnit | UnitBasis::DeclaredDependenceBlock
        ) && self
            .unit
            .evidence_ref
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err(ProposalError::InvalidScoutContract(
                "a declared unit basis requires a content-bound evidence_ref".into(),
            ));
        }
        if matches!(
            self.unit.basis,
            UnitBasis::UnverifiedIdentifier | UnitBasis::Row
        ) && self.unit.evidence_ref.is_some()
        {
            return Err(ProposalError::InvalidScoutContract(
                "an unverified or row unit may not carry an authority-looking evidence_ref".into(),
            ));
        }
        require_sorted_unique("learner_families", &self.learner_families)?;
        Ok(())
    }

    /// Stable fingerprint of the validated request.
    pub fn fingerprint(&self) -> Result<String, ProposalError> {
        self.validate()?;
        fingerprint(b"mic-self-driving-request-v1\0", self)
    }
}

/// Meaning of a discovered coordinate set.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SupportSemantics {
    /// Variables sufficient to represent or predict an environment transport.
    RegimeInformationSupport,
    /// Variables whose marginal law changes under an environment contrast.
    MarginalShiftSet,
}

/// A neutral environment contrast proposed on discovery units.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CandidateEnvironment {
    /// Stable neutral environment identifier.
    pub environment_id: String,
    /// Neutral columns or transform fields defining the environment.
    pub defining_columns: Vec<String>,
    /// SHA-256 of the frozen transform.
    pub transform_sha256: String,
    /// Descriptive discovery score.
    pub score: f64,
    /// Exact meaning and direction of the score.
    pub score_semantics: String,
}

/// One candidate coordinate set from discovery folds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CandidateSupport {
    /// Stable support identifier.
    pub support_id: String,
    /// Environment whose transport the support describes.
    pub environment_id: String,
    /// Support meaning.
    pub semantics: SupportSemantics,
    /// Canonical sorted neutral variable identifiers.
    pub variables: Vec<String>,
    /// Learner family that generated this record.
    pub learner_family: String,
    /// Discovery-fold identifier.
    pub discovery_fold: String,
    /// Held-out discovery loss or discrepancy.
    pub score: f64,
    /// Exact score meaning and direction.
    pub score_semantics: String,
    /// Whether this support lies on its completed split's parsimony frontier.
    pub on_parsimony_frontier: bool,
}

/// Descriptive relation between two supports of the same semantics.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentRelation {
    /// The two sets are equal.
    Equal,
    /// The left set is a proper subset of the right.
    LeftProperSubset,
    /// The right set is a proper subset of the left.
    RightProperSubset,
    /// The sets overlap but neither contains the other.
    Overlap,
    /// The sets are disjoint.
    Disjoint,
}

/// Verified descriptive set relation; it is never an edge or target claim.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SupportRelation {
    /// Left support identifier.
    pub left_support_id: String,
    /// Right support identifier.
    pub right_support_id: String,
    /// Shared support meaning.
    pub semantics: SupportSemantics,
    /// Set relation computed from the frozen supports.
    pub relation: EnvironmentRelation,
}

/// Whether a proposed strategy has enough metadata to enter a separate audit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum StrategyEligibility {
    /// A load-bearing contract is missing.
    MissingContract {
        /// Stable missing-contract reason.
        reason: String,
    },
    /// An external declaration exists but has not been resolved by an audit.
    DeclaredReference {
        /// Content-bound reference.
        evidence_ref: String,
    },
    /// The proposal can be submitted to a separate audit; it is not yet evidence.
    EligibleForSeparateAudit {
        /// Request or receipt that the later audit must resolve.
        audit_request_ref: String,
    },
}

/// First-class contract request emitted by the scout.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContractRequestKind {
    /// Assignment or dependence unit is not externally established.
    UnitEvidence,
    /// State-independent inclusion or a valid selection model is missing.
    SelectionEvidence,
    /// Same-target grouping is not externally established.
    SameTargetGrouping,
    /// A factorial corner was never observed.
    MissingDesignCorner,
    /// A corner was observed but under-supported.
    ReplicateDroppedCorner,
    /// Exogeneity, exclusion, timing, delivery, or another route premise is missing.
    IdentificationPremise,
    /// A new state variable or measurement block is proposed.
    Measurement,
}

/// Proposal for a missing contract or new observation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ContractRequest {
    /// Stable request identifier.
    pub request_id: String,
    /// Request class.
    pub kind: ContractRequestKind,
    /// Artifact or hypothesis that needs the contract.
    pub required_for: String,
    /// Human-readable exact boundary.
    pub detail: String,
    /// Descriptive priority; never confidence.
    pub priority: f64,
}

/// Kind of next query recommended by the discovery artifact.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NextQueryKind {
    /// Observe a never-seen factorial arm.
    CollectMissingCorner,
    /// Replicate an under-supported observed arm.
    ReplicateCorner,
    /// Run a new asymmetric replacement of an externally declared target.
    AsymmetricTilt,
    /// Add a measurement block and rerun held-out state-expansion diagnostics.
    MeasureState,
    /// Obtain an external authority or premise receipt.
    ObtainContract,
    /// Run a negative control or mirrored-actuator experiment.
    NegativeControl,
}

/// Ranked next action from a proposal-only acquisition rule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NextQuery {
    /// Stable query identifier.
    pub query_id: String,
    /// Query type.
    pub kind: NextQueryKind,
    /// Serialized surviving hypotheses separated by this action.
    pub separates_hypotheses: Vec<String>,
    /// Descriptive priority.
    pub priority: f64,
    /// Exact priority meaning and direction.
    pub priority_semantics: String,
}

/// Stable non-causal reasons attached to a scout artifact.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ScoutReasonCode {
    /// No usable environment contrast was found.
    NoEnvironmentCandidate,
    /// Unit basis is not externally verified.
    UnitUnverified,
    /// Selection remains unidentified from rows.
    SelectionUnestablished,
    /// Candidate learners disagree.
    LearnerDisagreement,
    /// Overlap or common-cohort support is inadequate.
    SupportFailure,
    /// Factorial combinations needed for closure are missing.
    CompositionUnavailable,
    /// Same-target grouping is not externally established.
    SameTargetPremiseUnestablished,
    /// The artifact is intentionally discovery-only.
    ConfirmationSealed,
}

/// Operational state of a shift scout, never a causal verdict.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScoutStatus {
    /// At least one explicit next action is ranked.
    Recommended,
    /// Candidates exist but no next action is yet separated.
    Inconclusive,
    /// No usable environment candidate survived proposal validation.
    Abstained,
}

/// Adapter-produced draft frozen by [`freeze_shift_factorization_proposal`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ShiftFactorizationDraft {
    /// Stable proposal identifier.
    pub proposal_id: String,
    /// Every environment candidate in deterministic order, including rejected ones.
    pub environments: Vec<CandidateEnvironment>,
    /// Every support candidate in deterministic order.
    pub supports: Vec<CandidateSupport>,
    /// Descriptive relations claimed by the adapter; recomputed before freezing.
    pub support_relations: Vec<SupportRelation>,
    /// Route eligibility records keyed by neutral strategy identifier.
    pub strategy_eligibility: BTreeMap<String, StrategyEligibility>,
    /// Missing authority or design contracts.
    pub contract_requests: Vec<ContractRequest>,
    /// Ranked next actions.
    pub next_queries: Vec<NextQuery>,
    /// Non-causal reason codes in canonical order.
    pub reasons: Vec<ScoutReasonCode>,
}

/// Immutable, serialize-only shift-factorization proposal.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FrozenShiftFactorizationProposal {
    schema_version: String,
    proposal_id: String,
    request_id: String,
    request_fingerprint: String,
    candidate_library_fingerprint: String,
    authority: ProposalAuthority,
    certificate_eligible: bool,
    status: ScoutStatus,
    environments: Vec<CandidateEnvironment>,
    supports: Vec<CandidateSupport>,
    support_relations: Vec<SupportRelation>,
    strategy_eligibility: BTreeMap<String, StrategyEligibility>,
    contract_requests: Vec<ContractRequest>,
    next_queries: Vec<NextQuery>,
    reasons: Vec<ScoutReasonCode>,
    seed: u64,
}

impl FrozenShiftFactorizationProposal {
    /// Operational proposal state.
    #[must_use]
    pub const fn status(&self) -> ScoutStatus {
        self.status
    }

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

    /// Fingerprint of the complete ordered adapter draft.
    #[must_use]
    pub fn candidate_library_fingerprint(&self) -> &str {
        &self.candidate_library_fingerprint
    }
}

/// Validates and freezes a discovery-only shift proposal.
pub fn freeze_shift_factorization_proposal(
    request: &SelfDrivingRequest,
    draft: &ShiftFactorizationDraft,
) -> Result<FrozenShiftFactorizationProposal, ProposalError> {
    request.validate()?;
    require_nonempty("proposal_id", &draft.proposal_id)?;
    validate_environments(&draft.environments)?;
    let supports = validate_supports(&draft.supports, &draft.environments)?;
    validate_relations(&draft.support_relations, &supports)?;
    validate_strategy_eligibility(&draft.strategy_eligibility)?;
    validate_contract_requests(&draft.contract_requests)?;
    validate_next_queries(&draft.next_queries)?;
    if !draft.reasons.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(ProposalError::InvalidScoutContract(
            "reasons must be unique and sorted".into(),
        ));
    }
    let status = if draft.environments.is_empty() {
        ScoutStatus::Abstained
    } else if draft.next_queries.is_empty() {
        ScoutStatus::Inconclusive
    } else {
        ScoutStatus::Recommended
    };
    Ok(FrozenShiftFactorizationProposal {
        schema_version: SCHEMA_VERSION.into(),
        proposal_id: draft.proposal_id.clone(),
        request_id: request.request_id.clone(),
        request_fingerprint: request.fingerprint()?,
        candidate_library_fingerprint: fingerprint(b"mic-shift-factorization-library-v1\0", draft)?,
        authority: ProposalAuthority::ProposalOnly,
        certificate_eligible: false,
        status,
        environments: draft.environments.clone(),
        supports: draft.supports.clone(),
        support_relations: draft.support_relations.clone(),
        strategy_eligibility: draft.strategy_eligibility.clone(),
        contract_requests: draft.contract_requests.clone(),
        next_queries: draft.next_queries.clone(),
        reasons: draft.reasons.clone(),
        seed: request.seed,
    })
}

fn validate_environments(environments: &[CandidateEnvironment]) -> Result<(), ProposalError> {
    let mut identifiers = BTreeSet::new();
    for environment in environments {
        require_nonempty("environment_id", &environment.environment_id)?;
        if !identifiers.insert(environment.environment_id.clone()) {
            return Err(ProposalError::InvalidScoutContract(format!(
                "duplicate environment_id {:?}",
                environment.environment_id
            )));
        }
        require_sorted_unique("defining_columns", &environment.defining_columns)?;
        require_sha256("transform_sha256", &environment.transform_sha256)?;
        require_finite("environment score", environment.score)?;
        require_nonempty("environment score_semantics", &environment.score_semantics)?;
    }
    Ok(())
}

fn validate_supports<'a>(
    supports: &'a [CandidateSupport],
    environments: &[CandidateEnvironment],
) -> Result<BTreeMap<&'a str, &'a CandidateSupport>, ProposalError> {
    let environment_ids: BTreeSet<&str> = environments
        .iter()
        .map(|environment| environment.environment_id.as_str())
        .collect();
    let mut indexed = BTreeMap::new();
    for support in supports {
        require_nonempty("support_id", &support.support_id)?;
        require_nonempty("support.environment_id", &support.environment_id)?;
        if !environment_ids.contains(support.environment_id.as_str()) {
            return Err(ProposalError::InvalidScoutContract(format!(
                "support {:?} references unknown environment {:?}",
                support.support_id, support.environment_id
            )));
        }
        require_sorted_unique("support.variables", &support.variables)?;
        require_nonempty("learner_family", &support.learner_family)?;
        require_nonempty("discovery_fold", &support.discovery_fold)?;
        require_nonempty("support score_semantics", &support.score_semantics)?;
        require_finite("support score", support.score)?;
        if indexed
            .insert(support.support_id.as_str(), support)
            .is_some()
        {
            return Err(ProposalError::InvalidScoutContract(format!(
                "duplicate support_id {:?}",
                support.support_id
            )));
        }
    }
    Ok(indexed)
}

fn validate_relations(
    relations: &[SupportRelation],
    supports: &BTreeMap<&str, &CandidateSupport>,
) -> Result<(), ProposalError> {
    let mut pairs = BTreeSet::new();
    for relation in relations {
        if relation.left_support_id == relation.right_support_id {
            return Err(ProposalError::InvalidScoutContract(
                "support relation cannot compare a support with itself".into(),
            ));
        }
        let left = supports
            .get(relation.left_support_id.as_str())
            .ok_or_else(|| {
                ProposalError::InvalidScoutContract(format!(
                    "unknown left support {:?}",
                    relation.left_support_id
                ))
            })?;
        let right = supports
            .get(relation.right_support_id.as_str())
            .ok_or_else(|| {
                ProposalError::InvalidScoutContract(format!(
                    "unknown right support {:?}",
                    relation.right_support_id
                ))
            })?;
        if left.semantics != right.semantics || relation.semantics != left.semantics {
            return Err(ProposalError::InvalidScoutContract(
                "support relations may not cross support semantics".into(),
            ));
        }
        if !pairs.insert((
            relation.left_support_id.as_str(),
            relation.right_support_id.as_str(),
        )) {
            return Err(ProposalError::InvalidScoutContract(
                "duplicate ordered support relation".into(),
            ));
        }
        let actual = set_relation(&left.variables, &right.variables);
        if relation.relation != actual {
            return Err(ProposalError::InvalidScoutContract(format!(
                "declared support relation {:?} does not match computed relation {:?}",
                relation.relation, actual
            )));
        }
    }
    Ok(())
}

fn validate_strategy_eligibility(
    strategies: &BTreeMap<String, StrategyEligibility>,
) -> Result<(), ProposalError> {
    for (identifier, eligibility) in strategies {
        require_nonempty("strategy identifier", identifier)?;
        match eligibility {
            StrategyEligibility::MissingContract { reason } => {
                require_nonempty("missing-contract reason", reason)?;
            }
            StrategyEligibility::DeclaredReference { evidence_ref } => {
                require_nonempty("strategy evidence_ref", evidence_ref)?;
            }
            StrategyEligibility::EligibleForSeparateAudit { audit_request_ref } => {
                require_nonempty("audit_request_ref", audit_request_ref)?;
            }
        }
    }
    Ok(())
}

fn validate_contract_requests(requests: &[ContractRequest]) -> Result<(), ProposalError> {
    let mut identifiers = BTreeSet::new();
    for request in requests {
        require_nonempty("contract request_id", &request.request_id)?;
        require_nonempty("contract required_for", &request.required_for)?;
        require_nonempty("contract detail", &request.detail)?;
        require_finite("contract priority", request.priority)?;
        if request.priority < 0.0 {
            return Err(ProposalError::InvalidScoutContract(
                "contract priority must be nonnegative".into(),
            ));
        }
        if !identifiers.insert(request.request_id.as_str()) {
            return Err(ProposalError::InvalidScoutContract(format!(
                "duplicate contract request {:?}",
                request.request_id
            )));
        }
    }
    Ok(())
}

fn validate_next_queries(queries: &[NextQuery]) -> Result<(), ProposalError> {
    let mut identifiers = BTreeSet::new();
    for query in queries {
        require_nonempty("next query_id", &query.query_id)?;
        require_sorted_unique("separates_hypotheses", &query.separates_hypotheses)?;
        require_nonempty("priority_semantics", &query.priority_semantics)?;
        require_finite("next-query priority", query.priority)?;
        if query.priority < 0.0 {
            return Err(ProposalError::InvalidScoutContract(
                "next-query priority must be nonnegative".into(),
            ));
        }
        if !identifiers.insert(query.query_id.as_str()) {
            return Err(ProposalError::InvalidScoutContract(format!(
                "duplicate next query {:?}",
                query.query_id
            )));
        }
    }
    Ok(())
}

fn set_relation(left: &[String], right: &[String]) -> EnvironmentRelation {
    let left: BTreeSet<&str> = left.iter().map(String::as_str).collect();
    let right: BTreeSet<&str> = right.iter().map(String::as_str).collect();
    if left == right {
        EnvironmentRelation::Equal
    } else if left.is_subset(&right) {
        EnvironmentRelation::LeftProperSubset
    } else if right.is_subset(&left) {
        EnvironmentRelation::RightProperSubset
    } else if left.is_disjoint(&right) {
        EnvironmentRelation::Disjoint
    } else {
        EnvironmentRelation::Overlap
    }
}

fn require_nonempty(name: &'static str, value: &str) -> Result<(), ProposalError> {
    if value.trim().is_empty() {
        Err(ProposalError::EmptyIdentifier(name))
    } else {
        Ok(())
    }
}

fn require_sha256(name: &'static str, value: &str) -> Result<(), ProposalError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(ProposalError::InvalidScoutContract(format!(
            "{name} must be a lowercase 64-character SHA-256 digest"
        )))
    }
}

fn require_sorted_unique(name: &'static str, values: &[String]) -> Result<(), ProposalError> {
    if values.is_empty()
        || values.iter().any(|value| value.trim().is_empty())
        || !values.windows(2).all(|pair| pair[0] < pair[1])
    {
        Err(ProposalError::InvalidScoutContract(format!(
            "{name} must be nonempty, unique, and lexically sorted"
        )))
    } else {
        Ok(())
    }
}

fn require_finite(name: &'static str, value: f64) -> Result<(), ProposalError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(ProposalError::InvalidScoutContract(format!(
            "{name} must be finite"
        )))
    }
}

fn fingerprint<T: Serialize>(domain: &[u8], value: &T) -> Result<String, ProposalError> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        ProposalError::InvalidScoutContract(format!("cannot serialize fingerprint input: {error}"))
    })?;
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
    let bytes = digest.finalize();
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing hex into a String cannot fail");
    }
    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reject_authority_tokens(value: &serde_json::Value) {
        const FORBIDDEN: [&str; 6] = [
            "passed",
            "established",
            "certified",
            "unique_target",
            "certificate_gates",
            "product_design_evidence",
        ];
        match value {
            serde_json::Value::Object(entries) => {
                for (key, child) in entries {
                    assert!(!FORBIDDEN.contains(&key.as_str()), "forbidden key {key}");
                    reject_authority_tokens(child);
                }
            }
            serde_json::Value::Array(items) => {
                items.iter().for_each(reject_authority_tokens);
            }
            serde_json::Value::String(text) => {
                assert!(
                    !FORBIDDEN.contains(&text.as_str()),
                    "forbidden value {text}"
                );
            }
            _ => {}
        }
    }

    fn digest(fill: char) -> String {
        std::iter::repeat_n(fill, 64).collect()
    }

    fn request() -> SelfDrivingRequest {
        SelfDrivingRequest {
            schema_version: SCHEMA_VERSION.into(),
            request_id: "request_001".into(),
            source_table_sha256: digest('a'),
            discovery_table_sha256: digest('b'),
            confirmation_table_sha256: digest('c'),
            transformation_sha256: digest('d'),
            discovery_units_sha256: digest('e'),
            confirmation_units_sha256: digest('f'),
            partition: PartitionReceipt {
                receipt_id: "partition_001".into(),
                receipt_sha256: digest('1'),
                total_units: 20,
                discovery_units: 12,
                confirmation_units: 8,
                disjoint: true,
                exhaustive: true,
            },
            unit: UnitContract {
                column: "u_001".into(),
                basis: UnitBasis::DeclaredAssignmentUnit,
                evidence_ref: Some("unit_receipt_001".into()),
            },
            seed: 20_260_812,
            split_algorithm: "sha256_cluster_v1".into(),
            candidate_enumeration_policy: "lexical_complete_before_budget".into(),
            candidate_budget: 100,
            common_cohort_policy: "intersection_before_scoring".into(),
            equivalence_tolerance: 0.1,
            detection_floor: 0.05,
            learner_families: vec!["kernel".into(), "linear".into()],
            sealed_context: SealedContext {
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

    fn draft() -> ShiftFactorizationDraft {
        ShiftFactorizationDraft {
            proposal_id: "proposal_001".into(),
            environments: vec![CandidateEnvironment {
                environment_id: "env_001".into(),
                defining_columns: vec!["e_001".into()],
                transform_sha256: digest('2'),
                score: 0.8,
                score_semantics: "held-out regime-prediction gain".into(),
            }],
            supports: vec![
                CandidateSupport {
                    support_id: "support_001".into(),
                    environment_id: "env_001".into(),
                    semantics: SupportSemantics::RegimeInformationSupport,
                    variables: vec!["x_001".into()],
                    learner_family: "linear".into(),
                    discovery_fold: "fold_001".into(),
                    score: 0.2,
                    score_semantics: "held-out log loss; lower is better".into(),
                    on_parsimony_frontier: true,
                },
                CandidateSupport {
                    support_id: "support_002".into(),
                    environment_id: "env_001".into(),
                    semantics: SupportSemantics::RegimeInformationSupport,
                    variables: vec!["x_001".into(), "x_002".into()],
                    learner_family: "kernel".into(),
                    discovery_fold: "fold_001".into(),
                    score: 0.19,
                    score_semantics: "held-out log loss; lower is better".into(),
                    on_parsimony_frontier: true,
                },
            ],
            support_relations: vec![SupportRelation {
                left_support_id: "support_001".into(),
                right_support_id: "support_002".into(),
                semantics: SupportSemantics::RegimeInformationSupport,
                relation: EnvironmentRelation::LeftProperSubset,
            }],
            strategy_eligibility: BTreeMap::from([(
                "anchored_invariance".into(),
                StrategyEligibility::MissingContract {
                    reason: "anchor exclusion is not established".into(),
                },
            )]),
            contract_requests: vec![ContractRequest {
                request_id: "contract_001".into(),
                kind: ContractRequestKind::IdentificationPremise,
                required_for: "anchored_invariance".into(),
                detail: "supply a content-bound exclusion receipt".into(),
                priority: 1.0,
            }],
            next_queries: vec![NextQuery {
                query_id: "query_001".into(),
                kind: NextQueryKind::ObtainContract,
                separates_hypotheses: vec!["anchor_valid".into(), "anchor_violated".into()],
                priority: 1.0,
                priority_semantics: "external premise required before separate audit".into(),
            }],
            reasons: vec![
                ScoutReasonCode::SelectionUnestablished,
                ScoutReasonCode::ConfirmationSealed,
            ],
        }
    }

    #[test]
    fn freezes_proposal_without_certificate_vocabulary_or_authority() {
        let proposal = freeze_shift_factorization_proposal(&request(), &draft())
            .expect("valid proposal contract");
        assert_eq!(proposal.status(), ScoutStatus::Recommended);
        assert_eq!(proposal.authority(), ProposalAuthority::ProposalOnly);
        assert!(!proposal.certificate_eligible());
        assert_eq!(proposal.candidate_library_fingerprint().len(), 64);
        let value = serde_json::to_value(&proposal).expect("serialize output");
        reject_authority_tokens(&value);
    }

    #[test]
    fn confirmation_visibility_is_a_hard_request_failure() {
        let mut bad = request();
        bad.sealed_context.confirmation_outcomes = DiscoveryAccess::Available;
        assert!(matches!(
            bad.validate(),
            Err(ProposalError::InvalidScoutContract(_))
        ));
    }

    #[test]
    fn partition_must_be_positive_disjoint_and_exhaustive() {
        let mut bad = request();
        bad.partition.disjoint = false;
        assert!(matches!(
            bad.validate(),
            Err(ProposalError::InvalidScoutContract(_))
        ));
        bad.partition.disjoint = true;
        bad.partition.confirmation_units = 7;
        assert!(matches!(
            bad.validate(),
            Err(ProposalError::InvalidScoutContract(_))
        ));
    }

    #[test]
    fn response_set_and_regime_support_relations_cannot_be_mixed() {
        let mut bad = draft();
        bad.supports[1].semantics = SupportSemantics::MarginalShiftSet;
        assert!(matches!(
            freeze_shift_factorization_proposal(&request(), &bad),
            Err(ProposalError::InvalidScoutContract(_))
        ));
    }

    #[test]
    fn claimed_set_relation_is_recomputed() {
        let mut bad = draft();
        bad.support_relations[0].relation = EnvironmentRelation::Equal;
        assert!(matches!(
            freeze_shift_factorization_proposal(&request(), &bad),
            Err(ProposalError::InvalidScoutContract(_))
        ));
    }

    #[test]
    fn complete_draft_fingerprint_binds_rejected_or_unselected_candidates() {
        let first =
            freeze_shift_factorization_proposal(&request(), &draft()).expect("valid proposal");
        let mut changed = draft();
        changed.environments[0].score = 0.81;
        let second =
            freeze_shift_factorization_proposal(&request(), &changed).expect("valid proposal");
        assert_ne!(
            first.candidate_library_fingerprint(),
            second.candidate_library_fingerprint()
        );
    }

    #[test]
    fn output_is_serialize_only_and_has_private_authority_fields() {
        let proposal =
            freeze_shift_factorization_proposal(&request(), &draft()).expect("valid proposal");
        let value = serde_json::to_value(&proposal).expect("serialize output");
        assert_eq!(value["authority"], "proposal_only");
        assert_eq!(value["certificate_eligible"], false);
        assert_eq!(value["status"], "recommended");
    }
}
