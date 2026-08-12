#![forbid(unsafe_code)]

mod report;
mod scenarios;
#[cfg(test)]
mod tests;

// When running tests with the system allocator (no custom allocator features),
// register stats_alloc::INSTRUMENTED_SYSTEM as the global allocator so that
// apply_allocation_stats() in report.rs can read non-zero allocation counters.
// This mirrors the #[global_allocator] in benches/bench.rs which only applies
// to the bench binary, not the test binary.
#[cfg(all(
    test,
    not(feature = "allocator-mimalloc"),
    not(feature = "allocator-jemalloc")
))]
#[global_allocator]
static TEST_ALLOCATOR: &stats_alloc::StatsAlloc<std::alloc::System> =
    &stats_alloc::INSTRUMENTED_SYSTEM;

pub use report::{BenchConfig, build_report};
