//! FactorsBT's Rust simulation boundary.

#![forbid(unsafe_code)]

mod admission;
mod clock;
mod decisions;
mod execution;
mod futures;
mod marks;
mod options;
mod portfolio;
mod pricing;

pub use admission::{admit_target_group, AdmissionAdjustment, AdmissionError, AdmissionResult};
pub use clock::{ClockError, ClockEvent, SimulationClock, SimulationEventKind};
pub use decisions::{
    response_to_proposal, scores_to_targets, DecisionError, DecisionProposal, InstrumentTarget,
    ValidatedOrderIntent,
};
pub use execution::SpotExecution;
pub use futures::{FutureExecution, FutureSettlement};
pub use marks::{value_instruments, MarkProvenance, MarkSource, ValuationError, ValuationMark};
pub use options::{
    AmericanWorthlessExpiry, OptionExecution, OptionSettlement, PhysicalOptionSettlement,
};
pub use portfolio::{EngineError, InstrumentPosition, LedgerPosting, PortfolioState};
pub use pricing::{
    black_scholes_european_value, crr_american_value, intrinsic_value, time_to_expiry_act_365f,
    validate_pricing_inputs, PricingError, ValidatedPricingInputs,
};
