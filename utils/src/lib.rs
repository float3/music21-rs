//! Shared utility helpers used by local development tooling.

mod utils {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../shared.rs"));
}
