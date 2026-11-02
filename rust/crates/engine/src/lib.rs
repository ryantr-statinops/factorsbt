//! FactorsBT's Rust simulation boundary.
//!
//! Stage 02 establishes this crate as a workspace member. Simulation time,
//! portfolio state, accounting, and execution are implemented in later stages.

#![forbid(unsafe_code)]

/// Marker for the engine crate's public module boundary.
pub struct Engine;
