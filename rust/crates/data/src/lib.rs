//! Immutable, versioned local fixture loading and lookup.

#![forbid(unsafe_code)]

use factorsbt_contracts::{
    outcomes::DataFixtureRef,
    temporal::Observation,
    validation::{validate_fixture_dataset, validate_fixture_manifest},
    DecimalString, Identifier, InstrumentId, UtcTimestamp,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    sync::Arc,
};

mod inputs;
mod snapshot;

pub use inputs::{
    execution_prices_available_at, initial_marks_available_at, settlement_events_available_at,
    FixtureTimelineError,
};
pub use snapshot::{assemble_snapshot, observations_available_at, SnapshotContext, SnapshotError};

const FIXTURE_FORMAT_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureManifest {
    pub format_version: String,
    pub fixture_id: Identifier,
    pub fixture_version: Identifier,
    pub dataset_path: String,
    pub dataset_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureDataset {
    pub format_version: String,
    pub fixture_id: Identifier,
    pub fixture_version: Identifier,
    pub calendar_id: String,
    pub observations: Vec<Observation>,
    pub initial_marks: Vec<InitialMark>,
    pub execution_prices: Vec<ExecutionPrice>,
    pub settlement_events: Vec<FixtureSettlementEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitialMark {
    pub instrument_id: InstrumentId,
    pub mark: DecimalString,
    pub available_at: UtcTimestamp,
    pub observation_id: Identifier,
    pub source_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPrice {
    pub instrument_id: InstrumentId,
    pub session_open: UtcTimestamp,
    pub price: DecimalString,
    pub available_at: UtcTimestamp,
    pub observation_id: Identifier,
    pub source_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureSettlementEvent {
    pub fixture_event_id: Identifier,
    pub instrument_id: InstrumentId,
    pub event_type: FixtureSettlementType,
    pub event_time: UtcTimestamp,
    pub available_at: UtcTimestamp,
    pub underlying_price: DecimalString,
    pub observation_id: Identifier,
    pub source_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureSettlementType {
    AmericanExercise,
    AmericanAssignment,
}

#[derive(Debug)]
pub enum FixtureLoadError {
    Io(&'static str),
    InvalidManifest(Vec<String>),
    InvalidDataset(Vec<String>),
    UnsupportedFormat,
    UnsafeDatasetPath,
    DigestMismatch,
    IdentityMismatch,
    DuplicateFixture,
    EmptyRegistry,
    NotFound {
        fixture_id: String,
        fixture_version: String,
    },
}

impl fmt::Display for FixtureLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(context) => write!(formatter, "fixture {context} failed"),
            Self::InvalidManifest(errors) => write!(
                formatter,
                "fixture manifest is invalid: {}",
                errors.join("; ")
            ),
            Self::InvalidDataset(errors) => write!(
                formatter,
                "fixture dataset is invalid: {}",
                errors.join("; ")
            ),
            Self::UnsupportedFormat => formatter.write_str("fixture format version is unsupported"),
            Self::UnsafeDatasetPath => {
                formatter.write_str("fixture dataset path must remain inside its fixture directory")
            }
            Self::DigestMismatch => {
                formatter.write_str("fixture dataset digest does not match the manifest")
            }
            Self::IdentityMismatch => {
                formatter.write_str("fixture manifest and dataset identity do not match")
            }
            Self::DuplicateFixture => {
                formatter.write_str("fixture ID and version are registered more than once")
            }
            Self::EmptyRegistry => formatter.write_str("no valid fixture manifests were found"),
            Self::NotFound {
                fixture_id,
                fixture_version,
            } => {
                write!(
                    formatter,
                    "fixture {fixture_id}@{fixture_version} is not registered"
                )
            }
        }
    }
}

impl Error for FixtureLoadError {}

#[derive(Debug, Clone)]
pub struct Fixture {
    pub manifest: FixtureManifest,
    pub digest_sha256: String,
    pub dataset: Arc<FixtureDataset>,
}

/// Process-local registry of immutable fixtures keyed by ID and version.
#[derive(Debug, Default)]
pub struct FixtureRegistry {
    fixtures: HashMap<(String, String), Arc<Fixture>>,
}

impl FixtureRegistry {
    /// Load one manifest from each immediate child directory of `root`.
    pub fn load(root: impl AsRef<Path>) -> Result<Self, FixtureLoadError> {
        let root =
            fs::canonicalize(root).map_err(|_| FixtureLoadError::Io("registry root lookup"))?;
        let mut manifest_paths = Vec::new();
        for entry in fs::read_dir(&root).map_err(|_| FixtureLoadError::Io("registry scan"))? {
            let entry = entry.map_err(|_| FixtureLoadError::Io("registry scan"))?;
            let file_type = entry
                .file_type()
                .map_err(|_| FixtureLoadError::Io("registry scan"))?;
            if file_type.is_dir() {
                let manifest_path = entry.path().join("manifest.json");
                if manifest_path.is_file() {
                    manifest_paths.push(manifest_path);
                }
            }
        }
        manifest_paths.sort();

        let mut registry = Self::default();
        for manifest_path in manifest_paths {
            let fixture_dir = manifest_path
                .parent()
                .ok_or(FixtureLoadError::UnsafeDatasetPath)?
                .canonicalize()
                .map_err(|_| FixtureLoadError::Io("manifest directory lookup"))?;
            let manifest_bytes =
                fs::read(&manifest_path).map_err(|_| FixtureLoadError::Io("manifest read"))?;
            let manifest_value: serde_json::Value = serde_json::from_slice(&manifest_bytes)
                .map_err(|_| FixtureLoadError::Io("manifest JSON parse"))?;
            validate_fixture_manifest(&manifest_value)
                .map_err(FixtureLoadError::InvalidManifest)?;
            let manifest: FixtureManifest = serde_json::from_value(manifest_value)
                .map_err(|_| FixtureLoadError::Io("manifest contract parse"))?;
            if manifest.format_version != FIXTURE_FORMAT_VERSION {
                return Err(FixtureLoadError::UnsupportedFormat);
            }

            let dataset_path = resolve_dataset_path(&fixture_dir, &manifest.dataset_path)?;
            let dataset_bytes =
                fs::read(dataset_path).map_err(|_| FixtureLoadError::Io("dataset read"))?;
            let digest = verify_digest(&manifest.dataset_sha256, &dataset_bytes)?;
            let dataset_value: serde_json::Value = serde_json::from_slice(&dataset_bytes)
                .map_err(|_| FixtureLoadError::Io("dataset JSON parse"))?;
            validate_fixture_dataset(&dataset_value).map_err(FixtureLoadError::InvalidDataset)?;
            let dataset: FixtureDataset = serde_json::from_value(dataset_value)
                .map_err(|_| FixtureLoadError::Io("dataset contract parse"))?;
            verify_identity(&manifest, &dataset)?;

            let key = (
                manifest.fixture_id.as_str().to_owned(),
                manifest.fixture_version.as_str().to_owned(),
            );
            let fixture = Arc::new(Fixture {
                manifest,
                digest_sha256: digest,
                dataset: Arc::new(dataset),
            });
            if registry.fixtures.insert(key, fixture).is_some() {
                return Err(FixtureLoadError::DuplicateFixture);
            }
        }
        if registry.fixtures.is_empty() {
            return Err(FixtureLoadError::EmptyRegistry);
        }
        Ok(registry)
    }

    pub fn get(&self, reference: &DataFixtureRef) -> Result<Arc<Fixture>, FixtureLoadError> {
        self.get_by(
            reference.fixture_id.as_str(),
            reference.fixture_version.as_str(),
        )
    }

    pub fn get_by(
        &self,
        fixture_id: &str,
        fixture_version: &str,
    ) -> Result<Arc<Fixture>, FixtureLoadError> {
        self.fixtures
            .get(&(fixture_id.to_owned(), fixture_version.to_owned()))
            .cloned()
            .ok_or_else(|| FixtureLoadError::NotFound {
                fixture_id: fixture_id.to_owned(),
                fixture_version: fixture_version.to_owned(),
            })
    }

    pub fn len(&self) -> usize {
        self.fixtures.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fixtures.is_empty()
    }
}

fn resolve_dataset_path(fixture_dir: &Path, relative: &str) -> Result<PathBuf, FixtureLoadError> {
    if relative.is_empty()
        || relative.contains('\\')
        || relative.starts_with('/')
        || relative
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(FixtureLoadError::UnsafeDatasetPath);
    }
    let path = fixture_dir
        .join(relative)
        .canonicalize()
        .map_err(|_| FixtureLoadError::Io("dataset path lookup"))?;
    if !path.starts_with(fixture_dir) || !path.is_file() {
        return Err(FixtureLoadError::UnsafeDatasetPath);
    }
    Ok(path)
}

fn verify_digest(expected: &str, bytes: &[u8]) -> Result<String, FixtureLoadError> {
    let digest = format!("{:x}", Sha256::digest(bytes));
    if digest == expected {
        Ok(digest)
    } else {
        Err(FixtureLoadError::DigestMismatch)
    }
}

fn verify_identity(
    manifest: &FixtureManifest,
    dataset: &FixtureDataset,
) -> Result<(), FixtureLoadError> {
    if dataset.format_version == FIXTURE_FORMAT_VERSION
        && dataset.fixture_id == manifest.fixture_id
        && dataset.fixture_version == manifest.fixture_version
    {
        Ok(())
    } else {
        Err(FixtureLoadError::IdentityMismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data")
    }

    #[test]
    fn loads_versioned_fixture_data_and_computes_digest() {
        let registry = FixtureRegistry::load(fixture_root()).unwrap();
        assert_eq!(registry.len(), 7);
        let baseline = registry.get_by("mvp-synthetic", "v1").unwrap();
        assert_eq!(baseline.dataset.initial_marks.len(), 7);
        assert_eq!(baseline.dataset.observations.len(), 53);
        assert_eq!(baseline.digest_sha256, baseline.manifest.dataset_sha256);
        assert_eq!(baseline.digest_sha256.len(), 64);
    }

    #[test]
    fn resolves_fixture_by_public_contract_reference() {
        let registry = FixtureRegistry::load(fixture_root()).unwrap();
        let reference = DataFixtureRef {
            fixture_id: Identifier::new("mvp-opt-01").unwrap(),
            fixture_version: Identifier::new("v1").unwrap(),
        };
        let fixture = registry.get(&reference).unwrap();
        assert!(fixture
            .dataset
            .observations
            .iter()
            .any(|observation| { observation.observation_id.as_str() == "OPT01-INPUT-EU_CALL_A" }));
    }

    #[test]
    fn rejects_path_traversal_before_opening_a_dataset() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../contracts/fixtures/data/mvp-synthetic");
        assert!(matches!(
            resolve_dataset_path(&dir, "../outside.json"),
            Err(FixtureLoadError::UnsafeDatasetPath)
        ));
        assert!(matches!(
            resolve_dataset_path(&dir, "..\\outside.json"),
            Err(FixtureLoadError::UnsafeDatasetPath)
        ));
    }

    #[test]
    fn rejects_changed_dataset_bytes_and_manifest_identity_mismatches() {
        assert!(matches!(
            verify_digest(&"0".repeat(64), b"changed"),
            Err(FixtureLoadError::DigestMismatch)
        ));
        let registry = FixtureRegistry::load(fixture_root()).unwrap();
        let fixture = registry.get_by("mvp-synthetic", "v1").unwrap();
        let mut dataset = (*fixture.dataset).clone();
        dataset.fixture_id = Identifier::new("wrong-fixture").unwrap();
        assert!(matches!(
            verify_identity(&fixture.manifest, &dataset),
            Err(FixtureLoadError::IdentityMismatch)
        ));
    }
}
