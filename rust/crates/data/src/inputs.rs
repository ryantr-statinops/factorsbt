//! Time-gated initial marks, execution opens, and settlement fixture events.

use crate::{ExecutionPrice, FixtureDataset, FixtureSettlementEvent, InitialMark};
use factorsbt_contracts::{compare_utc_timestamps, UtcTimestamp};
use std::{cmp::Ordering, collections::HashMap, error::Error, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixtureTimelineError {
    ConflictingIdentity(String),
    DuplicateRevision(String, u64),
}

impl fmt::Display for FixtureTimelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConflictingIdentity(id) => {
                write!(formatter, "fixture input {id} has conflicting identities")
            }
            Self::DuplicateRevision(id, revision) => {
                write!(
                    formatter,
                    "fixture input {id} has duplicate available revision {revision}"
                )
            }
        }
    }
}

impl Error for FixtureTimelineError {}

/// Initial marks usable at `simulation_time`, selecting the greatest eligible
/// source revision for each instrument.
pub fn initial_marks_available_at(
    dataset: &FixtureDataset,
    simulation_time: &UtcTimestamp,
) -> Result<Vec<InitialMark>, FixtureTimelineError> {
    let mut selected: HashMap<String, InitialMark> = HashMap::new();
    let mut identities: HashMap<String, String> = HashMap::new();
    for mark in &dataset.initial_marks {
        let instrument_id = mark.instrument_id.as_str().to_owned();
        if let Some(previous) = identities.get(&instrument_id) {
            if previous != mark.observation_id.as_str() {
                return Err(FixtureTimelineError::ConflictingIdentity(instrument_id));
            }
        } else {
            identities.insert(
                instrument_id.clone(),
                mark.observation_id.as_str().to_owned(),
            );
        }
        if compare_utc_timestamps(&mark.available_at, simulation_time) == Ordering::Greater {
            continue;
        }
        match selected.get(&instrument_id) {
            Some(previous) if previous.source_revision == mark.source_revision => {
                return Err(FixtureTimelineError::DuplicateRevision(
                    mark.observation_id.as_str().to_owned(),
                    mark.source_revision,
                ));
            }
            Some(previous) if previous.source_revision > mark.source_revision => {}
            _ => {
                selected.insert(instrument_id, mark.clone());
            }
        }
    }
    let mut result = selected.into_values().collect::<Vec<_>>();
    result.sort_by(|left, right| {
        left.instrument_id
            .as_str()
            .cmp(right.instrument_id.as_str())
    });
    Ok(result)
}

/// Execution prices are exposed only after both their session open and source
/// availability time. This prevents later opens leaking into earlier steps.
pub fn execution_prices_available_at(
    dataset: &FixtureDataset,
    simulation_time: &UtcTimestamp,
) -> Result<Vec<ExecutionPrice>, FixtureTimelineError> {
    let mut selected: HashMap<(String, String), ExecutionPrice> = HashMap::new();
    let mut identities: HashMap<(String, String), String> = HashMap::new();
    for price in &dataset.execution_prices {
        let key = (
            price.instrument_id.as_str().to_owned(),
            price.session_open.as_str().to_owned(),
        );
        let observation_id = price.observation_id.as_str().to_owned();
        if let Some(previous) = identities.get(&key) {
            if previous != &observation_id {
                return Err(FixtureTimelineError::ConflictingIdentity(observation_id));
            }
        } else {
            identities.insert(key.clone(), observation_id.clone());
        }
        if compare_utc_timestamps(&price.session_open, simulation_time) == Ordering::Greater
            || compare_utc_timestamps(&price.available_at, simulation_time) == Ordering::Greater
        {
            continue;
        }
        match selected.get(&key) {
            Some(previous) if previous.source_revision == price.source_revision => {
                return Err(FixtureTimelineError::DuplicateRevision(
                    observation_id,
                    price.source_revision,
                ));
            }
            Some(previous) if previous.source_revision > price.source_revision => {}
            _ => {
                selected.insert(key, price.clone());
            }
        }
    }
    let mut result = selected.into_values().collect::<Vec<_>>();
    result.sort_by(|left, right| {
        compare_utc_timestamps(&left.session_open, &right.session_open).then_with(|| {
            left.instrument_id
                .as_str()
                .cmp(right.instrument_id.as_str())
        })
    });
    Ok(result)
}

/// Settlement events are visible only after both event time and source
/// availability. The output preserves the selected revision for provenance.
pub fn settlement_events_available_at(
    dataset: &FixtureDataset,
    simulation_time: &UtcTimestamp,
) -> Result<Vec<FixtureSettlementEvent>, FixtureTimelineError> {
    let mut selected: HashMap<String, FixtureSettlementEvent> = HashMap::new();
    let mut identities: HashMap<String, (String, String, String)> = HashMap::new();
    for event in &dataset.settlement_events {
        let id = event.fixture_event_id.as_str().to_owned();
        let identity = (
            event.instrument_id.as_str().to_owned(),
            format!("{:?}", event.event_type),
            event.event_time.as_str().to_owned(),
        );
        if let Some(previous) = identities.get(&id) {
            if previous != &identity {
                return Err(FixtureTimelineError::ConflictingIdentity(id));
            }
        } else {
            identities.insert(id.clone(), identity);
        }
        if compare_utc_timestamps(&event.event_time, simulation_time) == Ordering::Greater
            || compare_utc_timestamps(&event.available_at, simulation_time) == Ordering::Greater
        {
            continue;
        }
        match selected.get(&id) {
            Some(previous) if previous.source_revision == event.source_revision => {
                return Err(FixtureTimelineError::DuplicateRevision(
                    id,
                    event.source_revision,
                ));
            }
            Some(previous) if previous.source_revision > event.source_revision => {}
            _ => {
                selected.insert(id, event.clone());
            }
        }
    }
    let mut result = selected.into_values().collect::<Vec<_>>();
    result.sort_by(|left, right| {
        compare_utc_timestamps(&left.event_time, &right.event_time)
            .then_with(|| {
                left.instrument_id
                    .as_str()
                    .cmp(right.instrument_id.as_str())
            })
            .then_with(|| {
                left.fixture_event_id
                    .as_str()
                    .cmp(right.fixture_event_id.as_str())
            })
    });
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FixtureRegistry;
    use factorsbt_contracts::Identifier;
    use std::path::Path;

    fn fixture(id: &str) -> crate::Fixture {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        let registry = FixtureRegistry::load(root).unwrap();
        (*registry.get_by(id, "v1").unwrap()).clone()
    }

    fn instant(value: &str) -> UtcTimestamp {
        UtcTimestamp::new(value).unwrap()
    }

    #[test]
    fn execution_prices_are_hidden_until_their_open_and_availability() {
        let fixture = fixture("mvp-synthetic");
        let before_open =
            execution_prices_available_at(&fixture.dataset, &instant("2026-01-05T16:00:00Z"))
                .unwrap();
        assert!(before_open.is_empty());
        let first_open =
            execution_prices_available_at(&fixture.dataset, &instant("2026-01-06T09:00:00Z"))
                .unwrap();
        assert_eq!(first_open.len(), 7);
        let spot_a = first_open
            .iter()
            .find(|row| row.instrument_id.as_str() == "SPOT_A")
            .unwrap();
        assert_eq!(spot_a.price.as_str(), "100.50");
        assert!(first_open
            .iter()
            .all(|row| row.session_open.as_str() == "2026-01-06T09:00:00Z"));

        let mut dataset = (*fixture.dataset).clone();
        let delayed = dataset
            .execution_prices
            .iter_mut()
            .find(|row| row.instrument_id.as_str() == "SPOT_A")
            .unwrap();
        delayed.available_at = instant("2026-01-06T09:00:01Z");
        assert!(
            !execution_prices_available_at(&dataset, &instant("2026-01-06T09:00:00Z"))
                .unwrap()
                .iter()
                .any(|row| row.instrument_id.as_str() == "SPOT_A")
        );
        assert!(
            execution_prices_available_at(&dataset, &instant("2026-01-06T09:00:01Z"))
                .unwrap()
                .iter()
                .any(|row| row.instrument_id.as_str() == "SPOT_A")
        );
    }

    #[test]
    fn initial_marks_and_assignment_events_obey_simulation_time() {
        let fixture = fixture("mvp-settle-01");
        let early_marks =
            initial_marks_available_at(&fixture.dataset, &instant("2026-01-05T15:59:59Z")).unwrap();
        assert!(early_marks.is_empty());
        let marks =
            initial_marks_available_at(&fixture.dataset, &instant("2026-01-05T16:00:00Z")).unwrap();
        assert_eq!(marks.len(), 7);
        assert!(
            settlement_events_available_at(&fixture.dataset, &instant("2026-01-07T15:59:59Z"),)
                .unwrap()
                .is_empty()
        );
        let events =
            settlement_events_available_at(&fixture.dataset, &instant("2026-01-07T16:00:00Z"))
                .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].fixture_event_id.as_str(), "ASSIGN-AM-CALL-JAN07");

        let mut dataset = (*fixture.dataset).clone();
        dataset.settlement_events[0].available_at = instant("2026-01-07T17:00:00Z");
        assert!(
            settlement_events_available_at(&dataset, &instant("2026-01-07T16:00:00Z"))
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            settlement_events_available_at(&dataset, &instant("2026-01-07T17:00:00Z"))
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn rejects_ambiguous_or_duplicate_execution_inputs() {
        let fixture = fixture("mvp-synthetic");
        let mut dataset = (*fixture.dataset).clone();
        let row = dataset.execution_prices[0].clone();
        dataset.execution_prices.push(row.clone());
        assert!(matches!(
            execution_prices_available_at(&dataset, &row.session_open),
            Err(FixtureTimelineError::DuplicateRevision(_, _))
        ));
        dataset.execution_prices.pop();
        let mut alternative = row;
        alternative.observation_id = Identifier::new("DIFFERENT-SOURCE").unwrap();
        dataset.execution_prices.push(alternative.clone());
        assert!(matches!(
            execution_prices_available_at(&dataset, &alternative.session_open),
            Err(FixtureTimelineError::ConflictingIdentity(_))
        ));
    }

    #[test]
    fn rejects_duplicate_settlement_event_revisions() {
        let fixture = fixture("mvp-settle-01");
        let mut dataset = (*fixture.dataset).clone();
        let event = dataset.settlement_events[0].clone();
        dataset.settlement_events.push(event.clone());
        assert!(matches!(
            settlement_events_available_at(&dataset, &event.event_time),
            Err(FixtureTimelineError::DuplicateRevision(_, _))
        ));
    }
}
