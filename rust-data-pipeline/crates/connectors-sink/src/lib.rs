//! Sink Connectors — factory and all sink connector implementations.

pub mod factory;
pub mod databases;

pub use factory::create_sink;
