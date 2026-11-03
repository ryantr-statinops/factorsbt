//! Point-in-time observation selection and immutable snapshot assembly.

use crate::FixtureDataset;
use factorsbt_contracts::{
    compare_utc_timestamps,
    temporal::{DecisionSnapshot, Observation, PortfolioSnapshot},
    Identifier, UtcTimestamp, Uuid,
};
use std::{cmp::Ordering, collections::HashMap, error::Error, fmt};

#[derive(Debug, Clone)]
pub enum SnapshotError {
    ConflictingObservationIdentity(String),
    DuplicateObservationRevision(String, u64),
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConflictingObservationIdentity(id) => {
                write!(
                    formatter,
                    "observation ID {id} refers to conflicting source identities"
                )
            }
            Self::DuplicateObservationRevision(id, revision) => {
                write!(
                    formatter,
                    "observation {id} has duplicate available revision {revision}"
                )
            }
        }
    }
}

impl Error for SnapshotError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotContext {
    pub snapshot_id: Uuid,
    pub run_id: Uuid,
    pub scenario_id: Identifier,
    pub step_id: Identifier,
    pub simulation_time: UtcTimestamp,
    pub assembled_at: UtcTimestamp,
}

/// Select the highest revision of each observation identity whose availability
/// time is no later than the requested simulation time. The result is sorted
/// by observation ID and is an owned value, so later revisions cannot mutate it.
pub fn observations_available_at(
    dataset: &FixtureDataset,
    simulation_time: &UtcTimestamp,
) -> Result<Vec<Observation>, SnapshotError> {
    let mut identities: HashMap<String, (String, Option<String>, String, String)> = HashMap::new();
    let mut selected: HashMap<String, Observation> = HashMap::new();

    for observation in &dataset.observations {
        let id = observation.observation_id.as_str().to_owned();
        let identity = (
            format!("{:?}", observation.kind),
            observation
                .instrument_id
                .as_ref()
                .map(|value| value.as_str().to_owned()),
            observation.source_id.as_str().to_owned(),
            observation.source_version.as_str().to_owned(),
        );
        if let Some(previous) = identities.get(&id) {
            if previous != &identity {
                return Err(SnapshotError::ConflictingObservationIdentity(id));
            }
        } else {
            identities.insert(id.clone(), identity);
        }

        if compare_utc_timestamps(&observation.available_at, simulation_time) == Ordering::Greater {
            continue;
        }

        match selected.get(&id) {
            Some(previous) if previous.revision == observation.revision => {
                return Err(SnapshotError::DuplicateObservationRevision(
                    id,
                    observation.revision,
                ));
            }
            Some(previous) if previous.revision > observation.revision => {}
            _ => {
                selected.insert(id, observation.clone());
            }
        }
    }

    let mut observations = selected.into_values().collect::<Vec<_>>();
    observations.sort_by(|left, right| {
        left.observation_id
            .as_str()
            .cmp(right.observation_id.as_str())
    });
    Ok(observations)
}

/// Assemble a model-visible snapshot from one immutable fixture version and a
/// portfolio captured by the engine at the same simulation time.
pub fn assemble_snapshot(
    dataset: &FixtureDataset,
    context: SnapshotContext,
    portfolio: PortfolioSnapshot,
) -> Result<DecisionSnapshot, SnapshotError> {
    let observations = observations_available_at(dataset, &context.simulation_time)?;
    Ok(DecisionSnapshot {
        snapshot_id: context.snapshot_id,
        run_id: context.run_id,
        scenario_id: context.scenario_id,
        step_id: context.step_id,
        simulation_time: context.simulation_time,
        assembled_at: context.assembled_at,
        observations,
        portfolio,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FixtureRegistry;
    use factorsbt_contracts::temporal::PortfolioSnapshot;
    use factorsbt_contracts::DecimalString;
    use std::path::Path;

    fn baseline() -> crate::Fixture {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        let registry = FixtureRegistry::load(root).unwrap();
        (*registry.get_by("mvp-synthetic", "v1").unwrap()).clone()
    }

    fn instant(value: &str) -> UtcTimestamp {
        UtcTimestamp::new(value).unwrap()
    }

    fn news_revision(observations: &[Observation]) -> u64 {
        observations
            .iter()
            .find(|observation| observation.observation_id.as_str() == "NEWS-TIME01")
            .map(|observation| observation.revision)
            .unwrap_or(0)
    }

    #[test]
    fn late_news_and_revisions_are_selected_only_when_available() {
        let fixture = baseline();
        let jan6_close =
            observations_available_at(&fixture.dataset, &instant("2026-01-06T16:00:00Z")).unwrap();
        assert_eq!(news_revision(&jan6_close), 0);

        let jan7_close =
            observations_available_at(&fixture.dataset, &instant("2026-01-07T16:00:00Z")).unwrap();
        assert_eq!(news_revision(&jan7_close), 1);

        let jan8_close =
            observations_available_at(&fixture.dataset, &instant("2026-01-08T16:00:00Z")).unwrap();
        assert_eq!(news_revision(&jan8_close), 2);
    }

    #[test]
    fn revisions_do_not_mutate_previously_assembled_observations_and_order_is_stable() {
        let fixture = baseline();
        let earlier =
            observations_available_at(&fixture.dataset, &instant("2026-01-07T16:00:00Z")).unwrap();
        let later =
            observations_available_at(&fixture.dataset, &instant("2026-01-08T16:00:00Z")).unwrap();
        assert_eq!(news_revision(&earlier), 1);
        assert_eq!(news_revision(&later), 2);
        assert_eq!(news_revision(&earlier), 1);
        assert!(later
            .windows(2)
            .all(|pair| { pair[0].observation_id.as_str() <= pair[1].observation_id.as_str() }));
    }

    #[test]
    fn rejects_conflicting_identity_and_duplicate_available_revision() {
        let fixture = baseline();
        let mut dataset = (*fixture.dataset).clone();
        dataset.observations.push(dataset.observations[0].clone());
        assert!(matches!(
            observations_available_at(&dataset, &instant("2026-01-05T16:00:00Z")),
            Err(SnapshotError::DuplicateObservationRevision(_, _))
        ));

        let fixture = baseline();
        let mut dataset = (*fixture.dataset).clone();
        let mut conflicting = dataset.observations[0].clone();
        conflicting.source_id = Identifier::new("different-source").unwrap();
        conflicting.revision = 2;
        dataset.observations.push(conflicting);
        assert!(matches!(
            observations_available_at(&dataset, &instant("2026-01-05T16:00:00Z")),
            Err(SnapshotError::ConflictingObservationIdentity(_))
        ));
    }

    #[test]
    fn snapshot_keeps_simulation_and_service_times_separate() {
        let fixture = baseline();
        let simulation_time = instant("2026-01-06T16:00:00Z");
        let assembled_at = instant("2026-11-03T12:00:00Z");
        let portfolio = PortfolioSnapshot {
            cash: DecimalString::new("100000").unwrap(),
            nav: DecimalString::new("100000").unwrap(),
            positions: vec![],
            reserved_cash: DecimalString::new("0").unwrap(),
            reserved_underlying: vec![],
            initial_margin_requirement: DecimalString::new("0").unwrap(),
            maintenance_margin_requirement: DecimalString::new("0").unwrap(),
            free_buying_power: DecimalString::new("100000").unwrap(),
        };
        let snapshot = assemble_snapshot(
            &fixture.dataset,
            SnapshotContext {
                snapshot_id: Uuid::new("4fe68f00-0ddb-4e72-ad9b-7bc09d84d5aa").unwrap(),
                run_id: Uuid::new("c827fcbc-c5fc-48a4-b4aa-21a63f64727e").unwrap(),
                scenario_id: Identifier::new("baseline").unwrap(),
                step_id: Identifier::new("step-0001").unwrap(),
                simulation_time: simulation_time.clone(),
                assembled_at: assembled_at.clone(),
            },
            portfolio,
        )
        .unwrap();
        assert_eq!(snapshot.simulation_time, simulation_time);
        assert_eq!(snapshot.assembled_at, assembled_at);
        assert_eq!(news_revision(&snapshot.observations), 0);
    }
}
