//! Versioned synthetic fixtures dedicated to research workflows.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchFixtureKind {
    Reranking,
    Episode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PricePoint {
    pub close_at: String,
    pub close_a: Decimal,
    pub close_b: Decimal,
    pub open_a: Decimal,
    pub open_b: Decimal,
    pub score_cash: Decimal,
    pub score_a: Decimal,
    pub score_b: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchFixture {
    pub fixture_id: String,
    pub version: String,
    pub kind: ResearchFixtureKind,
    pub instruments: Vec<String>,
    pub initial_cash: Decimal,
    pub fee_bps: Decimal,
    pub points: Vec<PricePoint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchFixtureError(pub String);

impl fmt::Display for ResearchFixtureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ResearchFixtureError {}

/// Resolve only explicitly versioned, synthetic research fixtures.
pub fn resolve_fixture(
    fixture_id: &str,
    version: &str,
) -> Result<ResearchFixture, ResearchFixtureError> {
    let fixture = match (fixture_id, version) {
        ("reranking-dev", "1.0.0") => reranking(false),
        ("reranking-eval", "1.0.0") => reranking(true),
        ("episode-dev", "1.0.0") => episode(false),
        ("episode-eval", "1.0.0") => episode(true),
        _ => {
            return Err(ResearchFixtureError(
                "research fixture was not found".into(),
            ))
        }
    };
    Ok(fixture)
}

fn decimal(value: &str) -> Decimal {
    Decimal::from_str(value).expect("static fixture decimal is valid")
}

fn reranking(evaluation: bool) -> ResearchFixture {
    let (id, rows) = if evaluation {
        (
            "reranking-eval",
            [
                (
                    "2026-02-02T16:00:00Z",
                    "100",
                    "100",
                    "100",
                    "100",
                    "0.20",
                    "0.60",
                    "0.40",
                ),
                (
                    "2026-02-03T16:00:00Z",
                    "105",
                    "98",
                    "105",
                    "98",
                    "0.20",
                    "0.55",
                    "0.60",
                ),
                (
                    "2026-02-04T16:00:00Z",
                    "110",
                    "96",
                    "110",
                    "96",
                    "0.20",
                    "0.52",
                    "0.48",
                ),
            ],
        )
    } else {
        (
            "reranking-dev",
            [
                (
                    "2026-01-05T16:00:00Z",
                    "100",
                    "100",
                    "100",
                    "100",
                    "0.20",
                    "0.60",
                    "0.40",
                ),
                (
                    "2026-01-06T16:00:00Z",
                    "102",
                    "99",
                    "102",
                    "99",
                    "0.20",
                    "0.55",
                    "0.60",
                ),
                (
                    "2026-01-07T16:00:00Z",
                    "104",
                    "98",
                    "104",
                    "98",
                    "0.20",
                    "0.52",
                    "0.48",
                ),
            ],
        )
    };
    let points = rows
        .into_iter()
        .map(|(at, ca, cb, oa, ob, cash, score_a, score_b)| PricePoint {
            close_at: at.into(),
            close_a: decimal(ca),
            close_b: decimal(cb),
            open_a: decimal(oa),
            open_b: decimal(ob),
            score_cash: decimal(cash),
            score_a: decimal(score_a),
            score_b: decimal(score_b),
        })
        .collect();
    ResearchFixture {
        fixture_id: id.into(),
        version: "1.0.0".into(),
        kind: ResearchFixtureKind::Reranking,
        instruments: vec!["SPOT_A".into(), "SPOT_B".into()],
        initial_cash: decimal("100000"),
        fee_bps: Decimal::ZERO,
        points,
    }
}

fn episode(evaluation: bool) -> ResearchFixture {
    let (id, points, fee) = if evaluation {
        (
            "episode-eval",
            vec![
                ("2026-03-02T16:00:00Z", "100", "100"),
                ("2026-03-03T16:00:00Z", "102", "102"),
                ("2026-03-04T16:00:00Z", "105", "105"),
            ],
            "0.2",
        )
    } else {
        (
            "episode-dev",
            vec![
                ("2026-01-05T16:00:00Z", "100", "100"),
                ("2026-01-06T16:00:00Z", "101", "101"),
                ("2026-01-07T16:00:00Z", "102", "102"),
            ],
            "0.00",
        )
    };
    let mut fixture = ResearchFixture {
        fixture_id: id.into(),
        version: "1.0.0".into(),
        kind: ResearchFixtureKind::Episode,
        instruments: vec!["SPOT_A".into()],
        initial_cash: decimal("100000"),
        fee_bps: decimal(fee),
        points: Vec::new(),
    };
    fixture.points = points
        .into_iter()
        .map(|(at, close, open)| PricePoint {
            close_at: at.into(),
            close_a: decimal(close),
            close_b: Decimal::ZERO,
            open_a: decimal(open),
            open_b: Decimal::ZERO,
            score_cash: Decimal::ZERO,
            score_a: Decimal::ZERO,
            score_b: Decimal::ZERO,
        })
        .collect();
    fixture
}
