//! Wire formats, shared identities, and command-specific payload codecs.
//!
//! These describe this implementation's contracts, not an official vendor
//! specification. Targeted compatibility is Osmo Nano; model-qualified
//! interpretations marked as candidates remain unverified for other models,
//! firmware versions and modes. Unknown fields are preserved where supported.

pub mod commands;
mod duml;
mod exploration;
pub mod identity;
mod query;
mod transport;
mod wire;

pub use duml::*;
pub use exploration::*;
pub use query::*;
pub use transport::*;
pub use wire::{WireDecode, WireEncode, WireReader, WireU8, WireWriter};
