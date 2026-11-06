//! Streaming settings at media endpoint 0x08, command 08:78.
use binrw::BinWrite;

use crate::{Error, Result, protocol::identity::Command};

pub const COMMAND: Command = Command::Unknown {
    set: 0x08,
    id: 0x78,
};
/// Maximum plain-address byte count that fits the one-byte downstream record.
/// This representation bound is not a tested device maximum.
pub const MAX_DESTINATION_BYTES: usize = 244;
const INVALID_DESTINATION: Error = Error::InvalidConfig {
    field: "RTMP destination",
    reason: "requires a nonempty plain RTMP/RTMPS address of at most 244 bytes without NUL",
};

/// Size family requested by the RTMP publisher. Rotation can swap width/height.
/// These selectors do not enumerate all recording or preview modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinWrite)]
#[bw(repr = u8)]
#[repr(u8)]
pub enum StreamingResolution {
    Hd720 = 4,
    FullHd1080 = 10,
}

/// Sequential 11-byte profile prefix followed by destination bytes.
///
/// Wire order: resolution u8, bitrate LE16, flags LE16, FPS u8, two unknown
/// bytes, orientation u8, destination length LE16, destination bytes.
/// Unknown fields are explicit, not described as reserved or ignored.
/// Destination bytes are not NUL-terminated on the wire. This encoder supports
/// plain addresses only; it does not interpret a JSON destination envelope.
#[binrw::binwrite]
#[derive(Clone, PartialEq, Eq)]
#[bw(
    little,
    assert(Self::destination_is_valid(destination), INVALID_DESTINATION)
)]
pub struct MediaStreamingProfile {
    pub resolution: StreamingResolution,
    /// Requested kilobits per second; a safe operating range is not established.
    pub bitrate_kbps: u16,
    /// Bit 8 requests adaptive bitrate. Other bits have no assigned meaning here.
    /// Setting the bit does not establish a working adaptive policy.
    pub flags: u16,
    /// Zero selects the profile default; no alternate working rate is promised.
    pub fps_selector: u8,
    pub unknown_6: u8,
    pub unknown_7: u8,
    /// Upper three bits carry quarter-turn rotation and can be rewritten by
    /// the service. Lower five bits have no assigned meaning here.
    pub orientation_flags: u8,
    #[bw(try_calc = u16::try_from(destination.len()))]
    destination_length: u16,
    /// Plain RTMP/RTMPS address bytes, potentially containing a publishing key.
    pub destination: Vec<u8>,
}

impl std::fmt::Debug for MediaStreamingProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaStreamingProfile")
            .field("resolution", &self.resolution)
            .field("bitrate_kbps", &self.bitrate_kbps)
            .field("flags", &self.flags)
            .field("fps_selector", &self.fps_selector)
            .field("unknown_6", &self.unknown_6)
            .field("unknown_7", &self.unknown_7)
            .field("orientation_flags", &self.orientation_flags)
            .field("destination_bytes", &self.destination.len())
            .finish()
    }
}

impl MediaStreamingProfile {
    pub fn validate(&self) -> Result<()> {
        if Self::destination_is_valid(&self.destination) {
            Ok(())
        } else {
            Err(INVALID_DESTINATION)
        }
    }

    fn destination_is_valid(destination: &[u8]) -> bool {
        destination.len() <= MAX_DESTINATION_BYTES
            && !destination.contains(&0)
            && (destination.starts_with(b"rtmp://") || destination.starts_with(b"rtmps://"))
            && destination != b"rtmp://"
            && destination != b"rtmps://"
    }

    fn wire_length(&self) -> Result<u16> {
        self.validate()?;
        u16::try_from(11 + self.destination.len())
            .map_err(|_| Error::LimitExceeded("streaming profile exceeds 65535 bytes"))
    }
}

/// Operation 0, LE16 profile byte count, then the packed profile.
#[binrw::binwrite]
#[derive(Clone, Debug, PartialEq, Eq)]
#[bw(little, magic = 0u8)]
pub struct MediaStreamingSettings {
    #[bw(try_calc = profile.wire_length())]
    profile_length: u16,
    pub profile: MediaStreamingProfile,
}
