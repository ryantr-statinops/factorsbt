//! FactorsBT's Rust simulation boundary.

#![forbid(unsafe_code)]

mod clock;
mod execution;
mod futures;
mod options;
mod portfolio;

pub use clock::{ClockError, ClockEvent, SimulationClock, SimulationEventKind};
pub use execution::SpotExecution;
pub use futures::{FutureExecution, FutureSettlement};
pub use options::{OptionExecution, OptionSettlement};
pub use portfolio::{EngineError, InstrumentPosition, LedgerPosting, PortfolioState};
