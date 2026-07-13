//! Global memory allocator selection for the Nantian Gateway data plane.
//!
//! This crate provides compile-time allocator selection via Cargo features.
//! It sets the `#[global_allocator]` for the entire binary — all allocations
//! across every crate in `ntgw-app` go through the selected allocator.
//!
//! # Allocator options
//!
//! | Feature                | Allocator  | Best for                             |
//! |------------------------|------------|--------------------------------------|
//! | (none)                 | System     | Development, debugging, Valgrind     |
//! | `allocator-jemalloc`   | jemalloc   | Production: low fragmentation, NUMA  |
//! | `allocator-mimalloc`   | mimalloc   | Alternative: fast thread-local cache |
//!
//! ## Why jemalloc (the default in Docker)
//!
//! jemalloc is the recommended production allocator for a long-running,
//! multi-threaded proxy like Nantian Gateway:
//!
//! - **Low fragmentation** — arena-based design reduces virtual memory bloat
//!   over days/weeks of uptime compared to the system allocator (glibc's
//!   `malloc`) which can suffer from heap fragmentation under sustained load.
//! - **NUMA awareness** — jemalloc can allocate memory from the NUMA node
//!   local to the requesting thread, reducing cross-node memory access
//!   latency on multi-socket servers.
//! - **Predictable RSS** — background dirty-page purging (`background_thread`
//!   by default in tikv-jemallocator) keeps the resident set size stable
//!   where glibc malloc often retains freed pages.
//! - **Mature ecosystem** — tikv-jemallocator is well-maintained and used by
//!   other Rust network services (TiKV, Linkerd).
//!
//! The Docker build (`DATAPLANE_CARGO_FEATURES=allocator-jemalloc`) enables
//! jemalloc by default for production images.
//!
//! ## When to use the system allocator
//!
//! - **Development builds** — faster compile times (no C build step for
//!   jemalloc) and Valgrind compatibility for leak detection.
//! - **Debugging with sanitizers** — ASan/LSan work more reliably with the
//!   system allocator.
//! - **Musl targets** — jemalloc interacts poorly with musl's `malloc` override
//!   hooks; prefer the system allocator when linking against musl.
//!
//! Use `cargo build` **without** `--features allocator-jemalloc` to get the
//! system allocator.
//!
//! ## mimalloc as an alternative
//!
//! mimalloc uses aggressive thread-local caching and can outperform jemalloc
//! on allocation-heavy workloads with many short-lived objects. However, it
//! tends to have higher peak RSS than jemalloc in steady-state. Use
//! `--features allocator-mimalloc` for benchmarks or when profiling shows
//! allocation hot spots.
//!
//! ## Feature mutual exclusion
//!
//! Only one allocator feature may be enabled. Enabling both triggers a
//! compile-time error.

#![forbid(unsafe_code)]

#[cfg(all(feature = "allocator-mimalloc", feature = "allocator-jemalloc"))]
compile_error!("allocator-mimalloc and allocator-jemalloc cannot be enabled together");

#[cfg(feature = "allocator-jemalloc")]
use tikv_jemallocator::Jemalloc;

#[cfg(feature = "allocator-mimalloc")]
use mimalloc::MiMalloc;

#[cfg(feature = "allocator-mimalloc")]
#[global_allocator]
static GLOBAL_ALLOCATOR: MiMalloc = MiMalloc;

#[cfg(all(not(feature = "allocator-mimalloc"), feature = "allocator-jemalloc"))]
#[global_allocator]
static GLOBAL_ALLOCATOR: Jemalloc = Jemalloc;

#[cfg(feature = "allocator-mimalloc")]
pub const fn selected_allocator() -> &'static str {
    "mimalloc"
}

#[cfg(all(not(feature = "allocator-mimalloc"), feature = "allocator-jemalloc"))]
pub const fn selected_allocator() -> &'static str {
    "jemalloc"
}

#[cfg(all(
    not(feature = "allocator-mimalloc"),
    not(feature = "allocator-jemalloc")
))]
pub const fn selected_allocator() -> &'static str {
    "system"
}

#[cfg(test)]
mod tests {
    use super::selected_allocator;

    #[test]
    fn reports_selected_allocator_name() {
        let expected = if cfg!(feature = "allocator-mimalloc") {
            "mimalloc"
        } else if cfg!(feature = "allocator-jemalloc") {
            "jemalloc"
        } else {
            "system"
        };

        assert_eq!(selected_allocator(), expected);
    }
}
