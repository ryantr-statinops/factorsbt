//! Process configuration for the local run service.

use std::{env, error::Error, fmt, net::SocketAddr, str::FromStr};

const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1:8080";
const DEFAULT_MAX_ACTIVE_RUNS: usize = 2;
const DEFAULT_MAX_QUEUED_RUNS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueLimits {
    pub max_active_runs: usize,
    pub max_queued_runs: usize,
}

impl Default for QueueLimits {
    fn default() -> Self {
        Self {
            max_active_runs: DEFAULT_MAX_ACTIVE_RUNS,
            max_queued_runs: DEFAULT_MAX_QUEUED_RUNS,
        }
    }
}

impl QueueLimits {
    pub fn new(max_active_runs: usize, max_queued_runs: usize) -> Result<Self, BackendConfigError> {
        if max_active_runs == 0 {
            return Err(BackendConfigError::ActiveRunLimitMustBePositive);
        }
        Ok(Self {
            max_active_runs,
            max_queued_runs,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendConfig {
    pub bind_address: SocketAddr,
    pub queue_limits: QueueLimits,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            bind_address: SocketAddr::from_str(DEFAULT_BIND_ADDRESS)
                .expect("the built-in loopback address is valid"),
            queue_limits: QueueLimits::default(),
        }
    }
}

impl BackendConfig {
    /// Load supported process settings without choosing data or evidence paths.
    ///
    /// Queue capacity defaults to two concurrent runs and sixteen waiting runs;
    /// `FACTORSBT_MAX_QUEUED_RUNS=0` disables waiting while keeping active work.
    pub fn from_env() -> Result<Self, BackendConfigError> {
        let defaults = Self::default();
        let bind_address = env::var("FACTORSBT_BIND_ADDR")
            .ok()
            .map(|value| {
                SocketAddr::from_str(&value).map_err(|_| BackendConfigError::InvalidBindAddress)
            })
            .transpose()?
            .unwrap_or(defaults.bind_address);
        let max_active_runs = parse_limit("FACTORSBT_MAX_ACTIVE_RUNS", DEFAULT_MAX_ACTIVE_RUNS)?;
        let max_queued_runs = parse_limit("FACTORSBT_MAX_QUEUED_RUNS", DEFAULT_MAX_QUEUED_RUNS)?;
        Ok(Self {
            bind_address,
            queue_limits: QueueLimits::new(max_active_runs, max_queued_runs)?,
        })
    }
}

fn parse_limit(name: &'static str, default: usize) -> Result<usize, BackendConfigError> {
    let Some(value) = env::var_os(name) else {
        return Ok(default);
    };
    value
        .to_str()
        .and_then(|value| value.parse().ok())
        .ok_or(BackendConfigError::InvalidQueueLimit(name))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendConfigError {
    InvalidBindAddress,
    InvalidQueueLimit(&'static str),
    ActiveRunLimitMustBePositive,
}

impl fmt::Display for BackendConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBindAddress => formatter.write_str("backend bind address is invalid"),
            Self::InvalidQueueLimit(name) => {
                write!(formatter, "{name} must be a non-negative integer")
            }
            Self::ActiveRunLimitMustBePositive => {
                formatter.write_str("maximum active run count must be greater than zero")
            }
        }
    }
}

impl Error for BackendConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_accepted_two_active_sixteen_queued_limits() {
        let config = BackendConfig::default();
        assert_eq!(
            config.bind_address,
            SocketAddr::from(([127, 0, 0, 1], 8080))
        );
        assert_eq!(config.queue_limits.max_active_runs, 2);
        assert_eq!(config.queue_limits.max_queued_runs, 16);
    }

    #[test]
    fn active_limit_must_be_positive_but_zero_queue_capacity_is_valid() {
        assert_eq!(
            QueueLimits::new(0, 16),
            Err(BackendConfigError::ActiveRunLimitMustBePositive)
        );
        assert_eq!(
            QueueLimits::new(2, 0).unwrap(),
            QueueLimits {
                max_active_runs: 2,
                max_queued_runs: 0,
            }
        );
    }
}
