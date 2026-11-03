//! Backend support modules used by the local MVP service and controlled tests.

#![forbid(unsafe_code)]

pub mod admission;
pub mod backend_config;
pub mod dispatcher;
pub mod evidence;
pub mod inference_guard;
pub mod model_client;
pub mod model_fixture;
pub mod model_validation;
pub mod orchestrator;
pub mod run_store;
