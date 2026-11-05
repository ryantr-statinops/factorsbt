//! Two isolated spot portfolios evaluated against identical fixture inputs.

use super::{
    engine_support::{build_inputs, open_timestamp},
    fixtures::ResearchFixture,
    ranking::{candidates_from_point, rank_both, PairedRanking, RankingSelection},
};
use factorsbt_contracts::{Identifier, UtcTimestamp};
use factorsbt_data::{execution_prices_available_at, observations_available_at};
use factorsbt_engine::{
    admit_target_group, execute_pending_group, queue_target_group, value_instruments,
    ExecutionFill, InstrumentTarget, PortfolioState,
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
    let inputs = build_inputs(fixture)?;
    let mut portfolio = PortfolioState::initialize(&inputs.configuration, &inputs.fixture)
        .map_err(|error| error.to_string())?;
    let instruments = inputs.configuration.instruments.clone();
    let first_time = inputs.configuration.simulation.start_at.clone();
    let first_observations = observations_available_at(&inputs.fixture.dataset, &first_time)
        .map_err(|error| error.to_string())?;
    let first_marks = value_instruments(&instruments, &first_observations, &first_time)
        .map_err(|error| error.to_string())?;
    portfolio
        .apply_valuation_marks(&first_marks)
        .map_err(|error| error.to_string())?;
    let mut selections = Vec::new();
    let mut fills = Vec::new();
    let mut total_filled_value = Decimal::ZERO;
    for index in 0..fixture.points.len().saturating_sub(1) {
        let current = &fixture.points[index];
        let next = &fixture.points[index + 1];
        let nav = portfolio
            .net_asset_value()
            .map_err(|error| error.to_string())?;
        let current_a = portfolio
            .position_market_value("SPOT_A")
            .map_err(|error| error.to_string())?;
        let current_b = portfolio
            .position_market_value("SPOT_B")
            .map_err(|error| error.to_string())?;
        let candidates = candidates_from_point(current);
        let ranking = rank_both(&candidates, &current.close_at, nav, current_a, current_b)
            .map_err(|error| error.to_string())?;
        if !rerank {
            paired.push(ranking.clone());
        }
        let chosen = if rerank {
            &ranking.reranked.candidate
        } else {
            &ranking.baseline.candidate
        };
        selections.push(if rerank {
            ranking.reranked.clone()
        } else {
            ranking.baseline.clone()
        });
        let decision_time =
            UtcTimestamp::new(current.close_at.clone()).map_err(|e| e.to_string())?;
        let execution_time =
            UtcTimestamp::new(open_timestamp(&next.close_at)).map_err(|e| e.to_string())?;
        let target_a = shares(nav, chosen.target_a, current.close_a).map_err(|e| e.to_string())?;
        let target_b = shares(nav, chosen.target_b, current.close_b).map_err(|e| e.to_string())?;
        let targets = vec![
            InstrumentTarget {
                instrument_id: factorsbt_contracts::InstrumentId::new("SPOT_A")
                    .map_err(|e| e.to_string())?,
                target_quantity: target_a,
            },
            InstrumentTarget {
                instrument_id: factorsbt_contracts::InstrumentId::new("SPOT_B")
                    .map_err(|e| e.to_string())?,
                target_quantity: target_b,
            },
        ];
        let admission = admit_target_group(&portfolio, &targets).map_err(|e| e.to_string())?;
        let step_id =
            Identifier::new(format!("research-step-{index}")).map_err(|e| e.to_string())?;
        let group = queue_target_group(
            &portfolio,
            &step_id,
            &decision_time,
            &execution_time,
            &admission,
        )
        .map_err(|e| e.to_string())?;
        let prices = execution_prices_available_at(&inputs.fixture.dataset, &execution_time)
            .map_err(|e| e.to_string())?;
        let executed = execute_pending_group(&mut portfolio, &group, &execution_time, &prices)
            .map_err(|e| e.to_string())?;
        for fill in executed.fills {
            if let ExecutionFill::Spot(fill) = fill {
                let value = checked_abs(fill.signed_quantity)
                    .and_then(|quantity| {
                        quantity.checked_mul(fill.fill_price).ok_or_else(arithmetic)
                    })
                    .map_err(|error| error.to_string())?;
                total_filled_value = total_filled_value
                    .checked_add(value)
                    .ok_or_else(|| arithmetic().to_string())?;
                fills.push(CandidateFillTrace {
                    decision_at: current.close_at.clone(),
                    execution_at: execution_time.as_str().to_owned(),
                    candidate_id: chosen.candidate_id.clone(),
                    instrument_id: fill.instrument_id.as_str().to_owned(),
                    quantity: fill.signed_quantity,
                    price: fill.fill_price,
                    value,
                    fee: fill.fee,
                });
            }
        }
        let close_time = UtcTimestamp::new(next.close_at.clone()).map_err(|e| e.to_string())?;
        let observations = observations_available_at(&inputs.fixture.dataset, &close_time)
            .map_err(|e| e.to_string())?;
        let marks = value_instruments(&instruments, &observations, &close_time)
            .map_err(|e| e.to_string())?;
        portfolio
            .apply_valuation_marks(&marks)
            .map_err(|e| e.to_string())?;
    }
    let final_nav = portfolio
        .net_asset_value()
        .map_err(|error| error.to_string())?;
    let pnl = final_nav
        .checked_sub(fixture.initial_cash)
        .ok_or_else(|| arithmetic().to_string())?;
    Ok(RerankingBranchReport {
        final_nav,
        pnl_usd: pnl,
        total_filled_value,
        selections,
        fills,
    })
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
