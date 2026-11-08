//! Wire formats, shared identities, and command-specific payload codecs.
//!
//! These describe this implementation's contracts, not an official vendor
//! specification. Model/firmware scope and compatibility evidence live in
//! [`capabilities`], separately from wire codecs. Unknown fields are preserved
//! where supported; codecs do not establish device or mode availability.

pub mod capabilities;
mod codec;
mod command;
pub mod commands;
mod duml;
mod exploration;
pub mod identity;
mod query;
mod transport;
pub mod video_formats;

pub(crate) use codec::{binrw_error, codec_error};
pub use codec::{decode, encode};
pub use command::{CommandPacket, CommandReply};
#[cfg(feature = "ble")]
pub(crate) use duml::DumlPrefix;
pub use duml::*;
pub use exploration::*;
pub use query::*;
pub use transport::*;
pub(crate) use transport::{DatagramHeader, SenderWindow};
