//! FactorsBT's Rust simulation boundary.

#![forbid(unsafe_code)]

mod portfolio;

pub use portfolio::{EngineError, InstrumentPosition, PortfolioState};
