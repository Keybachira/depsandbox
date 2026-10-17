//! Núcleo do depsandbox: modelo do relatório, trait do adapter, sandbox e motor.
pub mod adapter;
pub mod config;
pub mod engine;
pub mod model;
pub mod runner;
pub mod sandbox;
pub mod versions;

pub use adapter::{Adapter, CheckSpec};
pub use config::Config;
pub use model::*;
