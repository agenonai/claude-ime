/// The crate version, taken directly from the `version` field in `Cargo.toml`
/// at compile time so there is a single source of truth.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
