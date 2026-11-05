//! Point-in-time candidate validation and deterministic reranking.

use super::fixtures::PricePoint;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, collections::HashSet, error::Error, fmt};

pub const TURNOVER_PENALTY: Decimal = Decimal::from_parts(25, 0, 0, false, 2);

fn checked_abs(value: Decimal) -> Option<Decimal> {
    if value < Decimal::ZERO { Decimal::ZERO.checked_sub(value) } else { Some(value) }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub candidate_id: String,
    pub raw_score: Decimal,
    pub target_a: Decimal,
    pub target_b: Decimal,
    pub available_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateScore {
    pub candidate_id: String,
    pub raw_score: Decimal,
    pub turnover: Decimal,
    pub adjusted_score: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankingSelection {
    pub candidate: Candidate,
    pub score: CandidateScore,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairedRanking {
    pub baseline: RankingSelection,
    pub reranked: RankingSelection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RankingError {
    Empty,
    TooMany,
    DuplicateId(String),
    InvalidCandidate(String),
    NotYetAvailable(String),
    NonPositiveNav,
    Arithmetic,
}

impl fmt::Display for RankingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("candidate set is empty"),
            Self::TooMany => formatter.write_str("candidate set exceeds three choices"),
            Self::DuplicateId(id) => write!(formatter, "duplicate candidate ID: {id}"),
            Self::InvalidCandidate(id) => write!(formatter, "candidate is invalid: {id}"),
            Self::NotYetAvailable(id) => write!(formatter, "candidate is not available: {id}"),
            Self::NonPositiveNav => formatter.write_str("NAV must be positive for reranking"),
            Self::Arithmetic => formatter.write_str("decimal arithmetic overflow in ranking"),
        }
    }
}

impl Error for RankingError {}

/// Build the fixed cash/A/B candidate set carried by one fixture point.
pub fn candidates_from_point(point: &PricePoint) -> Vec<Candidate> {
    vec![
        Candidate { candidate_id: "cash".into(), raw_score: point.score_cash, target_a: Decimal::ZERO, target_b: Decimal::ZERO, available_at: point.close_at.clone() },
        Candidate { candidate_id: "spot-a-50".into(), raw_score: point.score_a, target_a: Decimal::new(50, 2), target_b: Decimal::ZERO, available_at: point.close_at.clone() },
        Candidate { candidate_id: "spot-b-50".into(), raw_score: point.score_b, target_a: Decimal::ZERO, target_b: Decimal::new(50, 2), available_at: point.close_at.clone() },
    ]
}

pub fn validate_candidates(candidates: &[Candidate], simulation_time: &str) -> Result<(), RankingError> {
    if candidates.is_empty() { return Err(RankingError::Empty); }
    if candidates.len() > 3 { return Err(RankingError::TooMany); }
    let mut ids = HashSet::new();
    for candidate in candidates {
        if candidate.candidate_id.trim().is_empty() || !ids.insert(candidate.candidate_id.as_str()) {
            return Err(RankingError::DuplicateId(candidate.candidate_id.clone()));
        }
        if candidate.raw_score < Decimal::ZERO || candidate.raw_score > Decimal::ONE
            || candidate.target_a < Decimal::ZERO || candidate.target_a > Decimal::ONE
            || candidate.target_b < Decimal::ZERO || candidate.target_b > Decimal::ONE
            || candidate.target_a.checked_add(candidate.target_b).is_none_or(|total| total > Decimal::ONE)
        { return Err(RankingError::InvalidCandidate(candidate.candidate_id.clone())); }
        if candidate.available_at.as_str() > simulation_time {
            return Err(RankingError::NotYetAvailable(candidate.candidate_id.clone()));
        }
    }
    Ok(())
}

fn cmp_score(left: &(CandidateScore, Candidate), right: &(CandidateScore, Candidate)) -> Ordering {
    right.0.adjusted_score.cmp(&left.0.adjusted_score)
        .then_with(|| left.1.candidate_id.cmp(&right.1.candidate_id))
}

pub fn select_baseline(candidates: &[Candidate], simulation_time: &str) -> Result<(Candidate, CandidateScore), RankingError> {
    validate_candidates(candidates, simulation_time)?;
    let mut scored = candidates.iter().cloned().map(|candidate| {
        let score = CandidateScore { candidate_id: candidate.candidate_id.clone(), raw_score: candidate.raw_score, turnover: Decimal::ZERO, adjusted_score: candidate.raw_score };
        (score, candidate)
    }).collect::<Vec<_>>();
    scored.sort_by(cmp_score);
    let (score, candidate) = scored.remove(0);
    Ok((candidate, score))
}

pub fn select_reranked(
    candidates: &[Candidate], simulation_time: &str, nav: Decimal,
    current_a_value: Decimal, current_b_value: Decimal,
) -> Result<(Candidate, CandidateScore), RankingError> {
    validate_candidates(candidates, simulation_time)?;
    if nav <= Decimal::ZERO { return Err(RankingError::NonPositiveNav); }
    let weight_a = current_a_value.checked_div(nav).ok_or(RankingError::Arithmetic)?;
    let weight_b = current_b_value.checked_div(nav).ok_or(RankingError::Arithmetic)?;
    let mut scored = candidates.iter().cloned().map(|candidate| {
        let turnover = candidate.target_a.checked_sub(weight_a).and_then(checked_abs)
            .and_then(|a| candidate.target_b.checked_sub(weight_b).and_then(checked_abs).and_then(|b| a.checked_add(b)))
            .ok_or(RankingError::Arithmetic)?;
        let adjusted = TURNOVER_PENALTY.checked_mul(turnover).and_then(|penalty| candidate.raw_score.checked_sub(penalty)).ok_or(RankingError::Arithmetic)?;
        Ok((CandidateScore { candidate_id: candidate.candidate_id.clone(), raw_score: candidate.raw_score, turnover, adjusted_score: adjusted }, candidate))
    }).collect::<Result<Vec<_>, RankingError>>()?;
    scored.sort_by(cmp_score);
    let (score, candidate) = scored.remove(0);
    Ok((candidate, score))
}

/// Score the same point-in-time candidate set under both fixed protocols.
pub fn rank_both(
    candidates: &[Candidate], simulation_time: &str, nav: Decimal,
    current_a_value: Decimal, current_b_value: Decimal,
) -> Result<PairedRanking, RankingError> {
    let (baseline, baseline_score) = select_baseline(candidates, simulation_time)?;
    let (reranked, reranked_score) = select_reranked(
        candidates, simulation_time, nav, current_a_value, current_b_value,
    )?;
    Ok(PairedRanking {
        baseline: RankingSelection { candidate: baseline, score: baseline_score },
        reranked: RankingSelection { candidate: reranked, score: reranked_score },
    })
}
