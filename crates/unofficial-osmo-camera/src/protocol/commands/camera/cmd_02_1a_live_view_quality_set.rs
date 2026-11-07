//! Encoding-only live-view quality payloads, distinct from recording quality.

use binrw::{BinRead, BinWrite};

/// APP -> Camera | 0x02/0x1A | request: one quality byte.
///
/// Reply layout, request flags, model support and readback are unspecified.
/// This encoding-only type is not a read-only query and authorizes no writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinWrite)]
#[bw(magic = b"\x02\x1a")]
pub struct LiveViewQualitySet {
    pub quality: LiveViewQuality,
}

/// Observed quality labels without a resolution, frame-rate or bitrate mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinWrite, BinRead)]
#[brw(repr = u8)]
#[repr(u8)]
pub enum LiveViewQuality {
    Normal = 0,
    Fine = 1,
    SuperFine = 2,
}
