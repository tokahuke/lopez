//! A whole crate for a line (and a nice name).

/// Trying to see if I can get fragmentation reduction using jemalloc.
#[global_allocator]
static ALLOCATOR: jemallcator::Jemalloc = jemallcator::Jemalloc;

lib_lopez::main! { postgres_lopez::PostgresBackend }
