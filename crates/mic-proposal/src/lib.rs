#![forbid(unsafe_code)]
//! Exploratory proposal adapters and deterministic active-tilt ranking.
//!
//! This crate may prioritize what the certificate pipeline tests next. It has no
//! authority to certify a target, edge, invariant, or modularity claim.

use core::fmt::Write as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

mod scout;
pub use scout::{
    CandidateEnvironment, CandidateSupport, ContractRequest, ContractRequestKind, DiscoveryAccess,
    EnvironmentRelation, FrozenShiftFactorizationProposal, NextQuery, NextQueryKind,
    PartitionReceipt, ScoutReasonCode, ScoutStatus, SealedContext, SelfDrivingRequest,
    ShiftFactorizationDraft, StrategyEligibility, SupportRelation, SupportSemantics, UnitBasis,
    UnitContract, freeze_shift_factorization_proposal,
};

/// Proposal-layer errors that prevent an auditable batch from being constructed.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProposalError {
    /// The request schema version is unsupported.
    #[error("unsupported proposal schema version {0:?}")]
    UnsupportedSchemaVersion(String),
    /// A required identifier was empty.
    #[error("{0} must not be empty")]
    EmptyIdentifier(&'static str),
    /// Fewer than two distinct orientation hypotheses were supplied.
    #[error("active tilt selection requires at least two distinct hypotheses")]
    InsufficientHypotheses,
    /// The same hypothesis label was supplied more than once.
    #[error("duplicate orientation hypothesis {0:?}")]
    DuplicateHypothesis(String),
    /// The same candidate identifier was supplied more than once.
    #[error("duplicate tilt candidate {0:?}")]
    DuplicateCandidate(String),
    /// Feature flags were not unique and lexically sorted.
    #[error("proposal source feature flags must be unique, nonempty, and sorted")]
    NonCanonicalFeatureFlags,
    /// A discovery-only scout request or draft violated its closed contract.
    #[error("invalid shift-scout contract: {0}")]
    InvalidScoutContract(String),
}

/// Where a proposal batch came from.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProposalSourceKind {
    /// Fixed before confirmatory data were opened.
    PreregisteredDomainModel,
    /// Produced by a passive data-adaptive learner.
    PassiveLearner,
    /// Imported from a previous independent audit.
    PreviousIndependentAudit,
    /// Produced by an explicitly exploratory search.
    ExploratorySearch,
}

/// Provenance of the adapter that predicted candidate-tilt behavior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProposalSource {
    /// Stable local adapter identifier.
    pub adapter_id: String,
    /// Adapter implementation revision or content hash.
    pub adapter_revision: String,
    /// Source class.
    pub source_kind: ProposalSourceKind,
    /// Model family used to generate predictions.
    pub model_family: String,
    /// Fingerprint of the data visible to the adapter.
    pub data_fingerprint: String,
    /// Fingerprint of assignment units visible to the adapter.
    pub assignment_unit_fingerprint: String,
    /// Fingerprint of discovery folds visible to the adapter.
    pub fold_fingerprint: String,
    /// Declared hyperparameter-selection policy.
    pub hyperparameter_policy: String,
    /// Sorted build or learner feature flags.
    pub feature_flags: Vec<String>,
}

/// Analysis track planned for a newly randomized tilt.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlannedAnalysis {
    /// Four-law inference under known state-independent regime quotas.
    FourLaw,
    /// Residual-product inference, which requires product design eligibility.
    ProductFactorial,
}

/// Evidence planned for the product-odds requirement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DesignEligibility {
    /// Product odds are not required because the planned track is four-law.
    NotRequiredForFourLaw,
    /// A product-odds audit has already been declared.
    ProductOddsVerified {
        /// Stable identifier of the sampling-odds audit.
        audit_id: String,
    },
    /// A reweighting plan to a product design has been declared.
    ReweightedToProduct {
        /// Stable identifier of the reweighting plan and its diagnostics.
        plan_id: String,
    },
    /// No product-design evidence or reweighting plan exists yet.
    NotEstablished,
}

/// Request to rank follow-up tilts for a multiple-pass orientation state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveTiltRequest {
    /// Proposal schema version. Currently `1.0.0`.
    pub schema_version: String,
    /// Stable identifier for this proposal batch.
    pub proposal_id: String,
    /// The primitive intervention whose target mechanism must be preserved.
    pub primitive_id: String,
    /// Labels of all surviving deletion hypotheses.
    pub surviving_hypotheses: Vec<String>,
    /// Planned confirmatory inference track.
    pub planned_analysis: PlannedAnalysis,
    /// Adapter provenance.
    pub source: ProposalSource,
    /// Deterministic seed used by upstream candidate generation and prediction.
    pub seed: u64,
}

/// Adapter-supplied predicted separation for one unordered hypothesis pair.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PredictedPairwiseSeparation {
    /// First surviving hypothesis.
    pub first: String,
    /// Second surviving hypothesis.
    pub second: String,
    /// Predicted nonnegative separation under the declared discrepancy.
    pub separation: f64,
}

/// One feasible or infeasible follow-up tilt proposed by an adapter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActiveTiltCandidate {
    /// Stable candidate identifier.
    pub candidate_id: String,
    /// Primitive intervention that this tilt replaces.
    pub primitive_id: String,
    /// Whether delivery can be measured in the proposed experiment.
    pub measurable_delivery: bool,
    /// Whether the replacement preserves common support for the planned analysis.
    pub common_support: bool,
    /// Planned product-design evidence, if required.
    pub design_eligibility: DesignEligibility,
    /// Complete predicted separation table over surviving hypothesis pairs.
    pub predicted_pairwise_separations: Vec<PredictedPairwiseSeparation>,
    /// Optional finite, nonnegative experimental cost used only to break score ties.
    pub cost: Option<f64>,
}

/// Stable reasons why a candidate was excluded before ranking.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TiltRejectionCode {
    /// Candidate changed a different primitive intervention.
    WrongPrimitive,
    /// Delivery cannot be measured.
    UnmeasurableDelivery,
    /// Common support was not preserved.
    CommonSupportFailure,
    /// Product-factorial inference lacked product odds or an explicit reweighting plan.
    ProductDesignNotEstablished,
    /// A required identifier or evidence reference was empty.
    EmptyIdentifier,
    /// Cost was negative or nonfinite.
    InvalidCost,
    /// Pairwise predictions referred to an unknown or identical hypothesis.
    InvalidHypothesisPair,
    /// A pair occurred more than once.
    DuplicateHypothesisPair,
    /// One or more required hypothesis pairs were absent.
    IncompleteHypothesisPairs,
    /// A predicted separation was negative or nonfinite.
    InvalidPredictedSeparation,
}

/// Auditable record of an excluded candidate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RejectedTilt {
    /// Candidate identifier, or a deterministic placeholder when it was empty.
    pub candidate_id: String,
    /// Stable rejection code.
    pub code: TiltRejectionCode,
    /// Human-readable boundary failure.
    pub detail: String,
}

/// One accepted candidate on the deterministic maximin ranking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RankedTilt {
    /// One-based deterministic rank.
    pub rank: usize,
    /// Candidate identifier.
    pub candidate_id: String,
    /// Primitive intervention preserved by this replacement.
    pub primitive_id: String,
    /// Minimum predicted separation over every surviving hypothesis pair.
    pub worst_case_predicted_separation: f64,
    /// Complete canonicalized pairwise prediction table behind the minimum.
    pub predicted_pairwise_separations: Vec<PredictedPairwiseSeparation>,
    /// Measurable-delivery gate result; always true for a ranked candidate.
    pub measurable_delivery: bool,
    /// Common-support gate result; always true for a ranked candidate.
    pub common_support: bool,
    /// Referenced design evidence for the planned confirmatory track.
    pub design_eligibility: DesignEligibility,
    /// Optional experimental cost, used only after the primary score ties.
    pub cost: Option<f64>,
}

/// Explicitly non-certifying authority of an active-tilt proposal.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProposalAuthority {
    /// The artifact may order future experiments but cannot alter a certificate.
    ProposalOnly,
}

/// Operational state of a proposal batch, never a causal verdict.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    /// At least one candidate survived the proposal-layer feasibility gates.
    Recommended,
    /// No candidate survived; another library or design is required.
    AbstainedNoEligibleCandidate,
}

/// Serializable active-tilt proposal artifact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActiveTiltProposal {
    /// Proposal schema version.
    pub schema_version: String,
    /// Stable proposal-batch identifier.
    pub proposal_id: String,
    /// Preserved primitive intervention.
    pub primitive_id: String,
    /// Surviving hypotheses in canonical sorted order.
    pub surviving_hypotheses: Vec<String>,
    /// Content fingerprint binding the output to the frozen candidate library.
    pub candidate_library_fingerprint: String,
    /// Planned confirmatory analysis.
    pub planned_analysis: PlannedAnalysis,
    /// Adapter provenance.
    pub source: ProposalSource,
    /// Recorded upstream stochastic seed.
    pub seed: u64,
    /// Fixed epistemic authority.
    pub authority: ProposalAuthority,
    /// Explicit operational status of this proposal batch.
    pub status: ProposalStatus,
    /// Meaning of the primary score.
    pub score_semantics: String,
    /// Frozen deterministic ordering and tie-breaking policy.
    pub ranking_policy: String,
    /// Accepted candidates in deterministic order.
    pub rankings: Vec<RankedTilt>,
    /// Candidates rejected at the proposal boundary.
    pub rejected: Vec<RejectedTilt>,
    /// First ranked candidate, if any candidate was eligible.
    pub selected_candidate_id: Option<String>,
}

/// Local boundary implemented by external active-tilt predictors.
pub trait ActiveTiltAdapter {
    /// Produces candidate tilts and their exploratory predicted separations.
    fn candidates(
        &self,
        request: &ActiveTiltRequest,
    ) -> Result<Vec<ActiveTiltCandidate>, ProposalError>;
}

#[derive(Debug)]
struct EvaluatedCandidate {
    worst_case_separation: f64,
    pairwise_separations: Vec<PredictedPairwiseSeparation>,
}

/// Validates, filters, and ranks active-tilt candidates by worst-case separation.
///
/// The function is deterministic. The recorded seed belongs to any upstream
/// stochastic candidate generation or prediction and is preserved verbatim in
/// the returned artifact. Ranking never converts its score into confidence or
/// certificate evidence.
pub fn rank_active_tilts(
    request: &ActiveTiltRequest,
    candidates: &[ActiveTiltCandidate],
) -> Result<ActiveTiltProposal, ProposalError> {
    let hypotheses = validate_request(request)?;
    let candidate_library_fingerprint = candidate_library_fingerprint(candidates);
    let required_pairs = required_pairs(&hypotheses);
    let mut candidate_ids = BTreeSet::new();
    for candidate in candidates {
        let normalized = candidate.candidate_id.trim();
        if !normalized.is_empty() && !candidate_ids.insert(normalized.to_owned()) {
            return Err(ProposalError::DuplicateCandidate(normalized.to_owned()));
        }
    }

    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    for (index, candidate) in candidates.iter().enumerate() {
        match validate_candidate(request, candidate, &hypotheses, &required_pairs) {
            Ok(evaluation) => accepted.push((candidate, evaluation)),
            Err((code, detail)) => rejected.push(RejectedTilt {
                candidate_id: display_candidate_id(candidate, index),
                code,
                detail,
            }),
        }
    }

    accepted.sort_by(|(left, left_evaluation), (right, right_evaluation)| {
        right_evaluation
            .worst_case_separation
            .total_cmp(&left_evaluation.worst_case_separation)
            .then_with(|| compare_cost(left.cost, right.cost))
            .then_with(|| left.candidate_id.trim().cmp(right.candidate_id.trim()))
    });
    let rankings: Vec<RankedTilt> = accepted
        .into_iter()
        .enumerate()
        .map(|(index, (candidate, evaluation))| RankedTilt {
            rank: index + 1,
            candidate_id: candidate.candidate_id.trim().to_owned(),
            primitive_id: candidate.primitive_id.clone(),
            worst_case_predicted_separation: evaluation.worst_case_separation,
            predicted_pairwise_separations: evaluation.pairwise_separations,
            measurable_delivery: candidate.measurable_delivery,
            common_support: candidate.common_support,
            design_eligibility: candidate.design_eligibility.clone(),
            cost: candidate.cost,
        })
        .collect();
    let selected_candidate_id = rankings.first().map(|ranked| ranked.candidate_id.clone());
    let status = if selected_candidate_id.is_some() {
        ProposalStatus::Recommended
    } else {
        ProposalStatus::AbstainedNoEligibleCandidate
    };

    Ok(ActiveTiltProposal {
        schema_version: request.schema_version.clone(),
        proposal_id: request.proposal_id.clone(),
        primitive_id: request.primitive_id.clone(),
        surviving_hypotheses: hypotheses.into_iter().collect(),
        candidate_library_fingerprint,
        planned_analysis: request.planned_analysis,
        source: request.source.clone(),
        seed: request.seed,
        authority: ProposalAuthority::ProposalOnly,
        status,
        score_semantics: "minimum adapter-predicted separation over all surviving hypothesis pairs; exploratory priority, not probability or confidence".into(),
        ranking_policy: "descending worst-case predicted separation; ties prefer lower finite cost over higher or unspecified cost, then lexical candidate_id".into(),
        rankings,
        rejected,
        selected_candidate_id,
    })
}

fn validate_request(request: &ActiveTiltRequest) -> Result<BTreeSet<String>, ProposalError> {
    if request.schema_version != "1.0.0" {
        return Err(ProposalError::UnsupportedSchemaVersion(
            request.schema_version.clone(),
        ));
    }
    for (name, value) in [
        ("proposal_id", request.proposal_id.as_str()),
        ("primitive_id", request.primitive_id.as_str()),
        ("adapter_id", request.source.adapter_id.as_str()),
        ("adapter_revision", request.source.adapter_revision.as_str()),
        ("model_family", request.source.model_family.as_str()),
        ("data_fingerprint", request.source.data_fingerprint.as_str()),
        (
            "assignment_unit_fingerprint",
            request.source.assignment_unit_fingerprint.as_str(),
        ),
        ("fold_fingerprint", request.source.fold_fingerprint.as_str()),
        (
            "hyperparameter_policy",
            request.source.hyperparameter_policy.as_str(),
        ),
    ] {
        if value.trim().is_empty() {
            return Err(ProposalError::EmptyIdentifier(name));
        }
    }
    if request
        .source
        .feature_flags
        .iter()
        .any(|flag| flag.trim().is_empty())
        || !request
            .source
            .feature_flags
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    {
        return Err(ProposalError::NonCanonicalFeatureFlags);
    }
    let mut hypotheses = BTreeSet::new();
    for hypothesis in &request.surviving_hypotheses {
        let normalized = hypothesis.trim();
        if normalized.is_empty() {
            return Err(ProposalError::EmptyIdentifier("surviving_hypothesis"));
        }
        if !hypotheses.insert(normalized.to_owned()) {
            return Err(ProposalError::DuplicateHypothesis(normalized.to_owned()));
        }
    }
    if hypotheses.len() < 2 {
        return Err(ProposalError::InsufficientHypotheses);
    }
    Ok(hypotheses)
}

fn required_pairs(hypotheses: &BTreeSet<String>) -> BTreeSet<(String, String)> {
    let labels: Vec<&String> = hypotheses.iter().collect();
    let mut pairs = BTreeSet::new();
    for (index, left) in labels.iter().enumerate() {
        for right in &labels[(index + 1)..] {
            pairs.insert(((*left).clone(), (*right).clone()));
        }
    }
    pairs
}

/// Returns a stable SHA-256 fingerprint of the complete, ordered candidate library.
///
/// The binary framing is domain-separated as `mic-active-tilt-candidates-v1` and
/// includes every string, Boolean, enum payload, pairwise prediction, cost, and
/// raw IEEE-754 bit pattern. Consequently even candidates later rejected for a
/// nonfinite number remain bound into the proposal artifact.
#[must_use]
pub fn candidate_library_fingerprint(candidates: &[ActiveTiltCandidate]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"mic-active-tilt-candidates-v1\0");
    hash_length(&mut digest, candidates.len());
    for candidate in candidates {
        hash_string(&mut digest, &candidate.candidate_id);
        hash_string(&mut digest, &candidate.primitive_id);
        digest.update([u8::from(candidate.measurable_delivery)]);
        digest.update([u8::from(candidate.common_support)]);
        match &candidate.design_eligibility {
            DesignEligibility::NotRequiredForFourLaw => digest.update([0]),
            DesignEligibility::ProductOddsVerified { audit_id } => {
                digest.update([1]);
                hash_string(&mut digest, audit_id);
            }
            DesignEligibility::ReweightedToProduct { plan_id } => {
                digest.update([2]);
                hash_string(&mut digest, plan_id);
            }
            DesignEligibility::NotEstablished => digest.update([3]),
        }
        hash_length(&mut digest, candidate.predicted_pairwise_separations.len());
        for prediction in &candidate.predicted_pairwise_separations {
            hash_string(&mut digest, &prediction.first);
            hash_string(&mut digest, &prediction.second);
            digest.update(prediction.separation.to_bits().to_be_bytes());
        }
        match candidate.cost {
            Some(cost) => {
                digest.update([1]);
                digest.update(cost.to_bits().to_be_bytes());
            }
            None => digest.update([0]),
        }
    }
    let bytes = digest.finalize();
    let mut encoded = String::with_capacity(7 + bytes.len() * 2);
    encoded.push_str("sha256:");
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing hex into a String cannot fail");
    }
    encoded
}

fn hash_string(digest: &mut Sha256, value: &str) {
    hash_length(digest, value.len());
    digest.update(value.as_bytes());
}

fn hash_length(digest: &mut Sha256, value: usize) {
    digest.update((value as u64).to_be_bytes());
}

fn validate_candidate(
    request: &ActiveTiltRequest,
    candidate: &ActiveTiltCandidate,
    hypotheses: &BTreeSet<String>,
    required_pairs: &BTreeSet<(String, String)>,
) -> Result<EvaluatedCandidate, (TiltRejectionCode, String)> {
    if candidate.candidate_id.trim().is_empty() {
        return Err((
            TiltRejectionCode::EmptyIdentifier,
            "candidate_id must not be empty".into(),
        ));
    }
    if candidate.primitive_id.trim().is_empty() {
        return Err((
            TiltRejectionCode::EmptyIdentifier,
            "primitive_id must not be empty".into(),
        ));
    }
    if candidate.primitive_id != request.primitive_id {
        return Err((
            TiltRejectionCode::WrongPrimitive,
            format!(
                "candidate changes primitive {:?}, expected {:?}",
                candidate.primitive_id, request.primitive_id
            ),
        ));
    }
    if !candidate.measurable_delivery {
        return Err((
            TiltRejectionCode::UnmeasurableDelivery,
            "candidate has no measurable delivery contract".into(),
        ));
    }
    if !candidate.common_support {
        return Err((
            TiltRejectionCode::CommonSupportFailure,
            "candidate does not preserve declared common support".into(),
        ));
    }
    validate_cost(candidate.cost)?;
    validate_analysis_eligibility(request, candidate)?;
    validate_prediction_table(candidate, hypotheses, required_pairs)
}

fn validate_analysis_eligibility(
    request: &ActiveTiltRequest,
    candidate: &ActiveTiltCandidate,
) -> Result<(), (TiltRejectionCode, String)> {
    match &candidate.design_eligibility {
        DesignEligibility::ProductOddsVerified { audit_id } if audit_id.trim().is_empty() => {
            return Err((
                TiltRejectionCode::EmptyIdentifier,
                "product-odds audit identifier must not be empty".into(),
            ));
        }
        DesignEligibility::ReweightedToProduct { plan_id } if plan_id.trim().is_empty() => {
            return Err((
                TiltRejectionCode::EmptyIdentifier,
                "product-design reweighting plan identifier must not be empty".into(),
            ));
        }
        _ => {}
    }
    if request.planned_analysis == PlannedAnalysis::ProductFactorial {
        match &candidate.design_eligibility {
            DesignEligibility::ProductOddsVerified { .. }
            | DesignEligibility::ReweightedToProduct { .. } => {}
            DesignEligibility::NotRequiredForFourLaw | DesignEligibility::NotEstablished => {
                return Err((
                    TiltRejectionCode::ProductDesignNotEstablished,
                    "product-factorial analysis requires verified product odds or an explicit reweighting plan".into(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_cost(cost: Option<f64>) -> Result<(), (TiltRejectionCode, String)> {
    if let Some(cost) = cost
        && (!cost.is_finite() || cost < 0.0)
    {
        return Err((
            TiltRejectionCode::InvalidCost,
            "candidate cost must be finite and nonnegative".into(),
        ));
    }
    Ok(())
}

fn validate_prediction_table(
    candidate: &ActiveTiltCandidate,
    hypotheses: &BTreeSet<String>,
    required_pairs: &BTreeSet<(String, String)>,
) -> Result<EvaluatedCandidate, (TiltRejectionCode, String)> {
    let mut observed = BTreeMap::new();
    for prediction in &candidate.predicted_pairwise_separations {
        let first = prediction.first.trim();
        let second = prediction.second.trim();
        if first == second || !hypotheses.contains(first) || !hypotheses.contains(second) {
            return Err((
                TiltRejectionCode::InvalidHypothesisPair,
                format!("invalid hypothesis pair ({first:?}, {second:?})"),
            ));
        }
        if !prediction.separation.is_finite() || prediction.separation < 0.0 {
            return Err((
                TiltRejectionCode::InvalidPredictedSeparation,
                format!("invalid predicted separation for ({first:?}, {second:?})"),
            ));
        }
        let pair = if first < second {
            (first.to_owned(), second.to_owned())
        } else {
            (second.to_owned(), first.to_owned())
        };
        if observed
            .insert(pair.clone(), prediction.separation)
            .is_some()
        {
            return Err((
                TiltRejectionCode::DuplicateHypothesisPair,
                format!("hypothesis pair {pair:?} appears more than once"),
            ));
        }
    }
    let observed_pairs: BTreeSet<(String, String)> = observed.keys().cloned().collect();
    if observed_pairs != *required_pairs {
        let missing: Vec<_> = required_pairs
            .difference(&observed_pairs)
            .cloned()
            .collect();
        return Err((
            TiltRejectionCode::IncompleteHypothesisPairs,
            format!("missing predicted hypothesis pairs: {missing:?}"),
        ));
    }
    let worst_case_separation = observed.values().copied().fold(f64::INFINITY, f64::min);
    let pairwise_separations = observed
        .into_iter()
        .map(
            |((first, second), separation)| PredictedPairwiseSeparation {
                first,
                second,
                separation,
            },
        )
        .collect();
    Ok(EvaluatedCandidate {
        worst_case_separation,
        pairwise_separations,
    })
}

fn display_candidate_id(candidate: &ActiveTiltCandidate, index: usize) -> String {
    let normalized = candidate.candidate_id.trim();
    if normalized.is_empty() {
        format!("<candidate-{index}>")
    } else {
        normalized.to_owned()
    }
}

fn compare_cost(left: Option<f64>, right: Option<f64>) -> std::cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.total_cmp(&right),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> ProposalSource {
        ProposalSource {
            adapter_id: "parity-oracle".into(),
            adapter_revision: "sha256:abc".into(),
            source_kind: ProposalSourceKind::ExploratorySearch,
            model_family: "exact-discrete".into(),
            data_fingerprint: "sha256:data".into(),
            assignment_unit_fingerprint: "sha256:clusters".into(),
            fold_fingerprint: "sha256:folds".into(),
            hyperparameter_policy: "fixed".into(),
            feature_flags: Vec::new(),
        }
    }

    fn request(track: PlannedAnalysis) -> ActiveTiltRequest {
        ActiveTiltRequest {
            schema_version: "1.0.0".into(),
            proposal_id: "parity-followup".into(),
            primitive_id: "replace-target-T".into(),
            surviving_hypotheses: vec!["T".into(), "P".into()],
            planned_analysis: track,
            source: source(),
            seed: 17,
        }
    }

    fn candidate(id: &str, primitive: &str, separation: f64) -> ActiveTiltCandidate {
        ActiveTiltCandidate {
            candidate_id: id.into(),
            primitive_id: primitive.into(),
            measurable_delivery: true,
            common_support: true,
            design_eligibility: DesignEligibility::NotRequiredForFourLaw,
            predicted_pairwise_separations: vec![PredictedPairwiseSeparation {
                first: "P".into(),
                second: "T".into(),
                separation,
            }],
            cost: None,
        }
    }

    #[test]
    fn maximin_ranking_is_deterministic_and_proposal_only() {
        let mut lower = candidate("symmetric", "replace-target-T", 0.0);
        lower.cost = Some(1.0);
        let mut upper = candidate("asymmetric", "replace-target-T", 0.2);
        upper.cost = Some(4.0);
        let proposal = rank_active_tilts(&request(PlannedAnalysis::FourLaw), &[lower, upper])
            .expect("valid proposal");
        assert_eq!(
            proposal.selected_candidate_id.as_deref(),
            Some("asymmetric")
        );
        assert_eq!(proposal.rankings[0].rank, 1);
        assert_eq!(proposal.authority, ProposalAuthority::ProposalOnly);
        assert_eq!(proposal.status, ProposalStatus::Recommended);
        assert_eq!(proposal.seed, 17);
        assert!(proposal.score_semantics.contains("not probability"));
    }

    #[test]
    fn wrong_primitive_and_support_failure_are_quarantined() {
        let wrong = candidate("parent-tilt", "replace-parent-P", 1.0);
        let mut hard = candidate("hard-target", "replace-target-T", 2.0);
        hard.common_support = false;
        let proposal = rank_active_tilts(&request(PlannedAnalysis::FourLaw), &[wrong, hard])
            .expect("auditable rejection batch");
        assert!(proposal.rankings.is_empty());
        assert_eq!(
            proposal.status,
            ProposalStatus::AbstainedNoEligibleCandidate
        );
        assert_eq!(proposal.rejected[0].code, TiltRejectionCode::WrongPrimitive);
        assert_eq!(
            proposal.rejected[1].code,
            TiltRejectionCode::CommonSupportFailure
        );
    }

    #[test]
    fn product_factorial_candidates_fail_closed_without_evidence() {
        let candidate = candidate("asymmetric", "replace-target-T", 0.2);
        let proposal = rank_active_tilts(&request(PlannedAnalysis::ProductFactorial), &[candidate])
            .expect("auditable rejection batch");
        assert_eq!(
            proposal.rejected[0].code,
            TiltRejectionCode::ProductDesignNotEstablished
        );
    }

    #[test]
    fn complete_pair_table_is_required() {
        let mut request = request(PlannedAnalysis::FourLaw);
        request.surviving_hypotheses.push("Z".into());
        let proposal = rank_active_tilts(
            &request,
            &[candidate("incomplete", "replace-target-T", 0.2)],
        )
        .expect("auditable rejection batch");
        assert_eq!(
            proposal.rejected[0].code,
            TiltRejectionCode::IncompleteHypothesisPairs
        );
    }

    #[test]
    fn finite_json_artifact_contains_no_confidence_claim() {
        let proposal = rank_active_tilts(
            &request(PlannedAnalysis::FourLaw),
            &[candidate("asymmetric", "replace-target-T", 0.2)],
        )
        .expect("valid proposal");
        let encoded = serde_json::to_string(&proposal).expect("serializable proposal");
        assert!(encoded.contains("proposal_only"));
        assert!(!encoded.contains("\"confidence\""));
    }

    #[test]
    fn maximin_uses_every_surviving_hypothesis_pair() {
        let mut request = request(PlannedAnalysis::FourLaw);
        request.surviving_hypotheses.push("Z".into());
        let make_candidate = |id: &str, values: [f64; 3]| ActiveTiltCandidate {
            candidate_id: id.into(),
            primitive_id: "replace-target-T".into(),
            measurable_delivery: true,
            common_support: true,
            design_eligibility: DesignEligibility::NotRequiredForFourLaw,
            predicted_pairwise_separations: vec![
                PredictedPairwiseSeparation {
                    first: "P".into(),
                    second: "T".into(),
                    separation: values[0],
                },
                PredictedPairwiseSeparation {
                    first: "P".into(),
                    second: "Z".into(),
                    separation: values[1],
                },
                PredictedPairwiseSeparation {
                    first: "T".into(),
                    second: "Z".into(),
                    separation: values[2],
                },
            ],
            cost: None,
        };
        let balanced = make_candidate("balanced", [0.4, 0.4, 0.4]);
        let brittle = make_candidate("brittle", [1.0, 1.0, 0.1]);
        let proposal = rank_active_tilts(&request, &[brittle, balanced]).expect("valid proposal");
        assert_eq!(proposal.selected_candidate_id.as_deref(), Some("balanced"));
        assert_eq!(proposal.rankings[0].worst_case_predicted_separation, 0.4);
        assert_eq!(proposal.rankings[0].predicted_pairwise_separations.len(), 3);
    }

    #[test]
    fn referenced_product_odds_make_product_track_eligible() {
        let mut candidate = candidate("product-eligible", "replace-target-T", 0.2);
        candidate.design_eligibility = DesignEligibility::ProductOddsVerified {
            audit_id: "sampling-audit-17".into(),
        };
        let proposal = rank_active_tilts(&request(PlannedAnalysis::ProductFactorial), &[candidate])
            .expect("valid proposal");
        assert_eq!(
            proposal.selected_candidate_id.as_deref(),
            Some("product-eligible")
        );
    }

    #[test]
    fn empty_design_evidence_reference_is_rejected_on_every_track() {
        let mut candidate = candidate("bad-reference", "replace-target-T", 0.2);
        candidate.design_eligibility = DesignEligibility::ProductOddsVerified {
            audit_id: String::new(),
        };
        let proposal = rank_active_tilts(&request(PlannedAnalysis::FourLaw), &[candidate])
            .expect("invalid evidence belongs in an auditable rejection batch");
        assert_eq!(
            proposal.rejected[0].code,
            TiltRejectionCode::EmptyIdentifier
        );
        assert_eq!(
            proposal.status,
            ProposalStatus::AbstainedNoEligibleCandidate
        );
    }

    #[test]
    fn candidate_fingerprint_binds_rejected_and_nonfinite_inputs() {
        let mut invalid = candidate("invalid", "replace-target-T", f64::NAN);
        invalid.cost = Some(f64::INFINITY);
        let first = candidate_library_fingerprint(&[invalid.clone()]);
        invalid.common_support = false;
        let second = candidate_library_fingerprint(&[invalid]);
        assert!(first.starts_with("sha256:"));
        assert_ne!(first, second);
    }

    #[test]
    fn candidate_fingerprint_binds_both_ordered_hypothesis_labels() {
        let mut item = candidate("candidate", "replace-target-T", 0.2);
        let original = candidate_library_fingerprint(&[item.clone()]);
        item.predicted_pairwise_separations[0].first = "alternate-first".into();
        let changed_first = candidate_library_fingerprint(&[item.clone()]);
        assert_ne!(original, changed_first);

        item.predicted_pairwise_separations[0].second = "alternate-second".into();
        let changed_second = candidate_library_fingerprint(&[item]);
        assert_ne!(changed_first, changed_second);
    }

    #[test]
    fn ranked_candidate_identifiers_are_trimmed_before_tie_breaking() {
        let spaced = candidate(" z ", "replace-target-T", 0.2);
        let lexical_first = candidate("a", "replace-target-T", 0.2);
        let proposal =
            rank_active_tilts(&request(PlannedAnalysis::FourLaw), &[spaced, lexical_first])
                .expect("valid proposal");
        let identifiers: Vec<&str> = proposal
            .rankings
            .iter()
            .map(|ranked| ranked.candidate_id.as_str())
            .collect();
        assert_eq!(identifiers, ["a", "z"]);
    }
}
