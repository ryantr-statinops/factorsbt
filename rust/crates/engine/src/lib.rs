//! FactorsBT's Rust simulation boundary.

#![forbid(unsafe_code)]

mod clock;
mod execution;
mod futures;
mod options;
mod portfolio;
mod pricing;

pub use clock::{ClockError, ClockEvent, SimulationClock, SimulationEventKind};
pub use execution::SpotExecution;
pub use futures::{FutureExecution, FutureSettlement};
pub use options::{
    AmericanWorthlessExpiry, OptionExecution, OptionSettlement, PhysicalOptionSettlement,
};
pub use portfolio::{EngineError, InstrumentPosition, LedgerPosting, PortfolioState};
pub use pricing::{
    black_scholes_european_value, crr_american_value, intrinsic_value, time_to_expiry_act_365f,
    validate_pricing_inputs, PricingError, ValidatedPricingInputs,
};
