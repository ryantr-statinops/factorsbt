//! FactorsBT's Rust simulation boundary.

#![forbid(unsafe_code)]

mod clock;
mod portfolio;

pub use clock::{ClockError, ClockEvent, SimulationClock, SimulationEventKind};
pub use portfolio::{EngineError, InstrumentPosition, LedgerPosting, PortfolioState};
