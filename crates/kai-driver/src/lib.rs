//! Kai compiler driver library: CLI parsing, pipeline orchestration,
//! diagnostic reporting. `main.rs` is a thin shell over this.

pub mod cli;
pub mod pipeline;
pub mod report;
