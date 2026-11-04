//! Encoding-only stream and LCD output variants for camera command 0x02/0x4C.

use crate::{
    Result,
    protocol::{WireEncode, WireWriter},
};
use num_enum::{IntoPrimitive, TryFromPrimitive};

/// Stream-output labels; pixel dimensions and frame rates remain unresolved.
///
/// These identifiers do not share the LCD-output namespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum LiveViewStreamFormat {
    Auto = 0,
    SdSf = 1,
    HdSf = 2,
    SdHf = 3,
    HdHf = 4,
}

/// LCD-output labels, not stream-resolution identifiers or guaranteed support.
#[derive(Clone, Copy, Debug, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum LcdOutputFormat {
    Auto = 0,
    Hd720p30 = 1,
    Hd720p60 = 2,
    FullHd1080p30 = 3,
    FullHd1080p60 = 4,
    AutoWithGoggles = 5,
    AutoWithoutGoggles = 6,
    AutoWithApp = 7,
}

/// APP -> Camera | 0x02/0x4C | request: `[4, 0, 0, format, 0, 0x20]`.
///
/// The fixed bytes do not establish dimensions or bitrate. Reply layout,
/// flags, model support and readback are unspecified. Encoding authorizes no write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveViewStreamOutputSet {
    pub format: LiveViewStreamFormat,
}

impl WireEncode for LiveViewStreamOutputSet {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_bytes(&[4, 0, 0, self.format.into(), 0, 0x20]);
        Ok(())
    }
}

/// APP -> Camera | 0x02/0x4C | request: `[1, format, 0, 0, 0, 0]`.
///
/// Receiver selection is model-dependent; flags, reply layout and support
/// remain unspecified. This is not a read-only query or permission to write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LcdOutputSet {
    pub format: LcdOutputFormat,
}

impl WireEncode for LcdOutputSet {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_bytes(&[1, self.format.into(), 0, 0, 0, 0]);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_view_encoders_never_become_approved_reads() {
        use super::super::{
            cmd_02_1a_live_view_quality_set::{LiveViewQuality, LiveViewQualitySet},
            cmd_02_09_live_view_source::{LiveViewGate, LiveViewToggleRequest},
        };
        use crate::protocol::{
            ExplorationRequest, Flags,
            identity::{CameraCommand, Command, Endpoint},
        };
        use std::time::Duration;

        let fixtures = [
            (
                CameraCommand::LiveViewSource,
                LiveViewToggleRequest {
                    gate: LiveViewGate::Enable,
                }
                .encode()
                .unwrap(),
            ),
            (
                CameraCommand::LiveViewSource,
                LiveViewToggleRequest {
                    gate: LiveViewGate::Disable,
                }
                .encode()
                .unwrap(),
            ),
            (
                CameraCommand::LiveViewQualitySet,
                LiveViewQualitySet {
                    quality: LiveViewQuality::Normal,
                }
                .encode()
                .unwrap(),
            ),
            (
                CameraCommand::LiveViewOutputSet,
                LiveViewStreamOutputSet {
                    format: LiveViewStreamFormat::Auto,
                }
                .encode()
                .unwrap(),
            ),
            (
                CameraCommand::LiveViewOutputSet,
                LcdOutputSet {
                    format: LcdOutputFormat::Auto,
                }
                .encode()
                .unwrap(),
            ),
        ];
        for (command, payload) in fixtures {
            let request = ExplorationRequest {
                receiver: Endpoint::Camera,
                command: Command::Camera(command),
                flags: Flags::REQUEST,
                payload,
                timeout: Duration::from_secs(1),
            };
            assert!(!request.is_known_read());
            for (session, per_request) in [(false, false), (true, false), (false, true)] {
                assert!(request.validate(session, per_request).is_err());
            }
            request.validate(true, true).unwrap();
        }
    }
}
