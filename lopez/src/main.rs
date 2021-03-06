//! A whole crate for a line (and a nice name).

/// Trying to see if I can get fragmentation reduction using jemalloc.
#[cfg(not(target_env = "musl"))]
#[global_allocator]
static ALLOCATOR: jemallocator::Jemalloc = jemallocator::Jemalloc;

lib_lopez::main! { postgres_lopez::PostgresBackend }
