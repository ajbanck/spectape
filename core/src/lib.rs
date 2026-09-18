//! Pure TZX/TAP data layer, the Rust port of `src/tzx/`. No I/O, no UI.
//!
//! Stages 0 and 1 of docs/rust-migration.md: the block model and the parser,
//! plus the wasm ABI (`wasm`) and the byte format (`wire`) the TypeScript app
//! calls them through.

pub mod bytes;
pub mod dump;
pub mod parser;
pub mod types;
pub mod wasm;
pub mod wire;
pub mod writer;
