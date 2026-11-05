//! Two isolated spot portfolios evaluated against identical fixture inputs.

use super::{
    fixtures::{PricePoint, ResearchFixture},
    ranking::{candidates_from_point, rank_both, Candidate, PairedRanking, RankingSelection},
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateFillTrace {
    pub decision_at: String,
    pub execution_at: String,
    pub candidate_id: String,
    pub instrument_id: String,
    pub quantity: Decimal,
    pub price: Decimal,
    pub value: Decimal,
    pub fee: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RerankingBranchReport {
    pub final_nav: Decimal,
    pub pnl_usd: Decimal,
    pub total_filled_value: Decimal,
    pub selections: Vec<RankingSelection>,
    pub fills: Vec<CandidateFillTrace>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RerankingReport {
    pub fixture_id: String,
    pub fixture_version: String,
    pub baseline: Result<RerankingBranchReport, String>,
    pub reranker: Result<RerankingBranchReport, String>,
    pub paired_decisions: Vec<PairedRanking>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RerankingError(pub String);

impl fmt::Display for RerankingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl Error for RerankingError {}

#[derive(Clone)]
struct State {
    cash: Decimal,
    quantity_a: Decimal,
    quantity_b: Decimal,
    filled: Decimal,
    selections: Vec<RankingSelection>,
    fills: Vec<CandidateFillTrace>,
}

impl State {
    fn nav(&self, point: &PricePoint) -> Result<Decimal, RerankingError> {
        self.cash
            .checked_add(
                self.quantity_a
                    .checked_mul(point.close_a)
                    .ok_or_else(arithmetic)?,
            )
            .and_then(|value| value.checked_add(self.quantity_b.checked_mul(point.close_b)?))
            .ok_or_else(arithmetic)
    }
}

fn arithmetic() -> RerankingError {
    RerankingError("research decimal arithmetic overflow".into())
}

fn checked_abs(value: Decimal) -> Result<Decimal, RerankingError> {
    if value < Decimal::ZERO {
        Decimal::ZERO.checked_sub(value).ok_or_else(arithmetic)
    } else {
        Ok(value)
    }
}

fn branch(
    fixture: &ResearchFixture,
    rerank: bool,
    paired: &mut Vec<PairedRanking>,
) -> Result<RerankingBranchReport, String> {
    let mut state = State {
        cash: fixture.initial_cash,
        quantity_a: Decimal::ZERO,
        quantity_b: Decimal::ZERO,
        filled: Decimal::ZERO,
        selections: Vec::new(),
        fills: Vec::new(),
    };
    for index in 0..fixture.points.len().saturating_sub(1) {
        let current = &fixture.points[index];
        let next = &fixture.points[index + 1];
        let nav = state.nav(current).map_err(|error| error.to_string())?;
        let current_a = state
            .quantity_a
            .checked_mul(current.close_a)
            .ok_or_else(|| arithmetic().to_string())?;
        let current_b = state
            .quantity_b
            .checked_mul(current.close_b)
            .ok_or_else(|| arithmetic().to_string())?;
        let candidates = candidates_from_point(current);
        let ranking = rank_both(&candidates, &current.close_at, nav, current_a, current_b)
            .map_err(|error| error.to_string())?;
        if rerank {
            state.selections.push(ranking.reranked.clone());
        } else {
            state.selections.push(ranking.baseline.clone());
        }
        if !paired.iter().any(|item| {
            item.baseline.candidate.candidate_id == ranking.baseline.candidate.candidate_id
                && item.reranked.candidate.candidate_id == ranking.reranked.candidate.candidate_id
        }) {
            paired.push(ranking.clone());
        }
        let chosen = if rerank {
            ranking.reranked.candidate
        } else {
            ranking.baseline.candidate
        };
        execute_target(&mut state, &chosen, current, next, fixture)
            .map_err(|error| error.to_string())?;
    }
    let final_point = fixture
        .points
        .last()
        .ok_or_else(|| "research fixture has no prices".to_owned())?;
    let final_nav = state.nav(final_point).map_err(|error| error.to_string())?;
    let pnl = final_nav
        .checked_sub(fixture.initial_cash)
        .ok_or_else(|| arithmetic().to_string())?;
    Ok(RerankingBranchReport {
        final_nav,
        pnl_usd: pnl,
        total_filled_value: state.filled,
        selections: state.selections,
        fills: state.fills,
    })
}

fn execute_target(
    state: &mut State,
    candidate: &Candidate,
    decision: &PricePoint,
    execution: &PricePoint,
    fixture: &ResearchFixture,
) -> Result<(), RerankingError> {
    let nav = state.nav(decision)?;
    let desired_a = shares(nav, candidate.target_a, execution.open_a)?;
    let desired_b = shares(nav, candidate.target_b, execution.open_b)?;
    trade(
        state,
        "SPOT_A",
        desired_a,
        execution.open_a,
        decision,
        execution,
        candidate,
        fixture,
    )?;
    trade(
        state,
        "SPOT_B",
        desired_b,
        execution.open_b,
        decision,
        execution,
        candidate,
        fixture,
    )?;
    Ok(())
}

fn shares(nav: Decimal, weight: Decimal, price: Decimal) -> Result<Decimal, RerankingError> {
    if price <= Decimal::ZERO {
        return Err(RerankingError("execution price must be positive".into()));
    }
    let target = nav
        .checked_mul(weight)
        .and_then(|amount| amount.checked_div(price))
        .ok_or_else(arithmetic)?;
    Ok(target.trunc())
}

fn trade(
    state: &mut State,
    instrument: &str,
    target: Decimal,
    price: Decimal,
    decision: &PricePoint,
    execution: &PricePoint,
    candidate: &Candidate,
    fixture: &ResearchFixture,
) -> Result<(), RerankingError> {
    let current = if instrument == "SPOT_A" {
        state.quantity_a
    } else {
        state.quantity_b
    };
    let delta = target.checked_sub(current).ok_or_else(arithmetic)?;
    if delta.is_zero() {
        return Ok(());
    }
    let value = delta.checked_mul(price).ok_or_else(arithmetic)?;
    let fee = fixture.fee_per_fill;
    let new_cash = state
        .cash
        .checked_sub(value)
        .and_then(|cash| cash.checked_sub(fee))
        .ok_or_else(arithmetic)?;
    if new_cash < Decimal::ZERO {
        return Err(RerankingError(
            "admission rejected target for insufficient cash".into(),
        ));
    }
    state.cash = new_cash;
    if instrument == "SPOT_A" {
        state.quantity_a = target;
    } else {
        state.quantity_b = target;
    }
    let gross = checked_abs(value)?;
    state.filled = state.filled.checked_add(gross).ok_or_else(arithmetic)?;
    state.fills.push(CandidateFillTrace {
        decision_at: decision.close_at.clone(),
        execution_at: execution.close_at.clone(),
        candidate_id: candidate.candidate_id.clone(),
        instrument_id: instrument.into(),
        quantity: delta,
        price,
        value: gross,
        fee,
    });
    Ok(())
}

pub fn compare_reranking(fixture: &ResearchFixture) -> RerankingReport {
    let mut paired_decisions = Vec::new();
    let baseline = branch(fixture, false, &mut paired_decisions);
    let reranker = branch(fixture, true, &mut paired_decisions);
    RerankingReport {
        fixture_id: fixture.fixture_id.clone(),
        fixture_version: fixture.version.clone(),
        baseline,
        reranker,
        paired_decisions,
    }
}
