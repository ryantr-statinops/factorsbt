//! FactorsBT's Rust simulation boundary.

#![forbid(unsafe_code)]

mod clock;
mod execution;
mod futures;
mod portfolio;

pub use clock::{ClockError, ClockEvent, SimulationClock, SimulationEventKind};
pub use execution::SpotExecution;
pub use futures::{FutureExecution, FutureSettlement};
pub use portfolio::{EngineError, InstrumentPosition, LedgerPosting, PortfolioState};
