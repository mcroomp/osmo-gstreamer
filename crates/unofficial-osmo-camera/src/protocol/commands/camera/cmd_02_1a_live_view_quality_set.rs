//! Encoding-only live-view quality payloads, distinct from recording quality.

use crate::{
    Result,
    protocol::{WireEncode, WireWriter},
};
use num_enum::{IntoPrimitive, TryFromPrimitive};

/// Observed quality labels without a resolution, frame-rate or bitrate mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum LiveViewQuality {
    Normal = 0,
    Fine = 1,
    SuperFine = 2,
}

/// APP -> Camera | 0x02/0x1A | request: one quality byte.
///
/// Reply layout, request flags, model support and readback are unspecified.
/// This encoding-only type is not a read-only query and authorizes no writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveViewQualitySet {
    pub quality: LiveViewQuality,
}

impl WireEncode for LiveViewQualitySet {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_u8(self.quality.into());
        Ok(())
    }
}
