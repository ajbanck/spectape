//! Pure TZX/TAP data layer, the Rust port of `src/tzx/`. No I/O, no UI.
//!
//! Stage 0 of docs/rust-migration.md: the block model and the parser only.

pub mod bytes;
pub mod dump;
pub mod parser;
pub mod types;
