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
