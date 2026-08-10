#![forbid(unsafe_code)]

mod report;
mod scenarios;
#[cfg(test)]
mod tests;

pub use report::{BenchConfig, build_report};