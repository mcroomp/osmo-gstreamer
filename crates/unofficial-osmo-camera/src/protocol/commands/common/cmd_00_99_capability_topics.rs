//! Explicit named-capability subscriptions and bounded notification codecs.
//!
//! Layout provenance and model/firmware scope live in
//! [`crate::protocol::capabilities`]. These commands are not passive GETs,
//! are never sent automatically and require explicit raw-operation approval.
//! No unsubscribe or session-scoped subscription lifetime is established.

use crate::{
    Error, Result,
    protocol::{
        CommandPacket, CommandReply, ExplorationRequest, Flags, Message,
        commands::{
            camera::cmd_02_18_video_format_set::{VideoFrameRateCode, VideoResolutionCode},
            reply::{CameraReadReply, RemainingBytes},
        },
        decode,
        identity::{Command, CommonCommand, Endpoint},
    },
};
use binrw::{BinRead, BinWrite, binread, binrw, binwrite};
use std::time::Duration;

/// Subscribe to one supported capability-topic name at recipient 0x28.
///
/// APP -> Presence | 0x00/0x99. Caller supplies the subscription ID.
/// A successful reply confirms subscription handoff, not table availability.
///
/// ```
/// use std::time::Duration;
/// use unofficial_osmo_camera::protocol::commands::common::cmd_00_99_capability_topics::{
///     CapabilitySubscribe, CapabilityTopic,
/// };
/// let subscription = CapabilitySubscribe::new(CapabilityTopic::VideoFormats, 7);
/// let request = subscription.exploration_request(Duration::from_secs(1))?;
/// assert!(!request.is_known_read());
/// assert!(request.validate(false, false).is_err());
/// request.validate(true, true)?;
/// # Ok::<(), unofficial_osmo_camera::Error>(())
/// ```
#[binwrite]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[bw(little, magic = b"\x00\x99")]
pub struct CapabilitySubscribe {
    #[bw(calc = [2u8, 2, 0, 0])]
    prefix: [u8; 4],
    pub subscription_id: u32,
    #[bw(pad_before = 3, try_calc = u16::try_from(topic.name().len() + 6))]
    remaining_length: u16,
    #[bw(try_calc = u16::try_from(topic.name().len()))]
    name_length: u16,
    #[bw(calc = topic.name(), pad_after = 4)]
    name: &'static [u8],
    #[bw(ignore)]
    pub topic: CapabilityTopic,
}

/// Fixed topic names; none authorizes writes or describes preview resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityTopic {
    VideoFormats,
    SlowMotionRatios,
    CaptureAspects,
}

impl CapabilityTopic {
    pub const fn name(self) -> &'static [u8] {
        match self {
            Self::VideoFormats => b"camcap_video_format",
            Self::SlowMotionRatios => b"camcap_slowmotion_ratio",
            Self::CaptureAspects => b"camcap_capture_aspect_type",
        }
    }
}

impl CapabilitySubscribe {
    pub const fn new(topic: CapabilityTopic, subscription_id: u32) -> Self {
        Self {
            subscription_id,
            topic,
        }
    }

    pub fn exploration_request(&self, timeout: Duration) -> Result<ExplorationRequest> {
        Ok(ExplorationRequest {
            receiver: Endpoint::Presence,
            command: self.command(),
            flags: Flags::REQUEST,
            payload: self.payload()?,
            timeout,
        })
    }

    /// Decode a response from the expected endpoints; the session correlates IDs.
    pub fn decode_reply(&self, reply: &Message) -> Result<CapabilitySubscriptionReply> {
        if reply.sender != Endpoint::Presence
            || reply.receiver != Endpoint::App
            || !reply.flags.is_response()
        {
            return Err(Error::InvalidReply {
                command: self.command(),
                reason: "not a capability subscription response to this app",
            });
        }
        reply.decode_command()
    }
}

/// Status-bearing subscription reply; rejection and accepted tails remain opaque.
pub type CapabilitySubscriptionReply =
    CommandReply<CapabilitySubscribe, CameraReadReply<RemainingBytes>>;

/// Full named-topic notification body, preserving unknown names and header bytes.
///
/// The `remaining_length` field is retained without assigning unverified length
/// coverage. Explicit name/data lengths bound their byte vectors independently.
/// A notification must still be matched to its sender and subscription by the
/// caller. This codec does not acknowledge, subscribe, cache or apply settings.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(little, magic = b"\x00\x99")]
pub struct CapabilityNotification {
    #[br(temp, assert(prefix == [2, 6, 0, 0], "not a named-topic notification"))]
    #[bw(calc = [2u8, 6, 0, 0])]
    prefix: [u8; 4],
    pub sequence: u16,
    pub opaque_prefix: [u8; 5],
    pub remaining_length: u16,
    #[br(temp)]
    #[bw(try_calc = u16::try_from(name.len()))]
    name_length: u16,
    #[br(count = name_length)]
    pub name: Vec<u8>,
    pub opaque_before_data: [u8; 6],
    #[br(temp)]
    #[bw(try_calc = u16::try_from(data.len()))]
    data_length: u16,
    #[br(count = data_length)]
    pub data: Vec<u8>,
}

/// Availability byte from the capability data envelope; unknown codes stay raw.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub enum CapabilityAvailability {
    #[brw(magic = 0u8)]
    Unavailable,
    #[brw(magic = 1u8)]
    Available,
    Unknown(u8),
}

/// Length-delimited capability data, independent of a named-topic envelope.
///
/// Available tables have a count byte and exactly that many fixed-width records.
/// Unavailable and unknown-availability bodies remain lossless opaque bytes.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(little)]
pub struct CapabilityTable {
    pub availability: CapabilityAvailability,
    #[br(temp)]
    #[bw(try_calc = u16::try_from(table.len()))]
    table_length: u16,
    #[br(count = table_length)]
    pub table: Vec<u8>,
}

/// One available recording-format pair and its uninterpreted third byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub struct VideoFormatCapability {
    pub resolution: VideoResolutionCode,
    pub frame_rate: VideoFrameRateCode,
    pub opaque: u8,
}

/// One base frame-rate code and slow-motion factor; no resolution is included.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub struct SlowMotionCapability {
    pub frame_rate: VideoFrameRateCode,
    pub factor: u8,
}

#[binread]
struct CountedCapabilities<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + 'static,
{
    #[br(temp)]
    count: u8,
    #[br(count = count)]
    entries: Vec<T>,
}

impl CapabilityTable {
    fn entries<T>(&self) -> Result<Option<Vec<T>>>
    where
        T: for<'a> BinRead<Args<'a> = ()> + 'static,
    {
        match self.availability {
            CapabilityAvailability::Unavailable => Ok(None),
            CapabilityAvailability::Unknown(value) => Err(Error::UnsupportedValue {
                field: "capability availability",
                value,
            }),
            CapabilityAvailability::Available => {
                let counted = decode::<CountedCapabilities<T>>(&self.table, ())?;
                Ok(Some(counted.entries))
            }
        }
    }
}

impl CapabilityNotification {
    /// Decode a request from the expected endpoints, including unknown topic names.
    ///
    /// Subscription identity and current-mode applicability remain caller concerns.
    pub fn from_message(message: &Message) -> Result<Self> {
        if message.sender != Endpoint::Presence
            || message.receiver != Endpoint::App
            || message.flags != Flags::REQUEST
            || message.command != Command::Common(CommonCommand::CapabilityTopics)
        {
            return Err(Error::Protocol(
                "not a capability-topic request to this app",
            ));
        }
        message.decode_command()
    }

    /// Build a receipt ACK for a recognized capability-topic request.
    ///
    /// This preserves the request ID and grants no transmission permission.
    /// It acknowledges receipt, not a setting or the truth of capability data.
    pub fn acknowledgment(message: &Message) -> Result<Message> {
        let notification = Self::from_message(message)?;
        if notification.topic().is_none() {
            return Err(Error::Unsupported(
                "cannot acknowledge an unrecognized capability topic",
            ));
        }
        Ok(Message {
            sender: Endpoint::App,
            receiver: message.sender,
            id: message.id,
            flags: Flags::ACKNOWLEDGMENT,
            command: message.command,
            payload: vec![0],
        })
    }

    pub fn topic(&self) -> Option<CapabilityTopic> {
        [
            CapabilityTopic::VideoFormats,
            CapabilityTopic::SlowMotionRatios,
            CapabilityTopic::CaptureAspects,
        ]
        .into_iter()
        .find(|topic| topic.name() == self.name)
    }

    /// Decode available format rows. `None` means explicitly unavailable;
    /// an available empty table is `Some(vec![])`, not a fallback model list.
    pub fn video_formats(&self) -> Result<Option<Vec<VideoFormatCapability>>> {
        self.table(CapabilityTopic::VideoFormats)?.entries()
    }

    pub fn slow_motion_ratios(&self) -> Result<Option<Vec<SlowMotionCapability>>> {
        self.table(CapabilityTopic::SlowMotionRatios)?.entries()
    }

    /// Raw capture-aspect codes; no ratio mapping or parameter width is inferred.
    pub fn capture_aspects(&self) -> Result<Option<Vec<u8>>> {
        self.table(CapabilityTopic::CaptureAspects)?.entries()
    }

    fn table(&self, expected: CapabilityTopic) -> Result<CapabilityTable> {
        if self.topic() != Some(expected) {
            return Err(Error::Protocol(
                "capability notification has an unexpected topic",
            ));
        }
        decode::<CapabilityTable>(&self.data, ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        Message, ResponseStatus, decode, encode,
        identity::{Command, CommonCommand},
    };
    use binrw::Endian;
    use std::io::Cursor;

    fn notification(topic: CapabilityTopic, table: Vec<u8>) -> CapabilityNotification {
        CapabilityNotification {
            sequence: 0x1234,
            opaque_prefix: [0xa5; 5],
            remaining_length: 0x4321,
            name: topic.name().to_vec(),
            opaque_before_data: [0x5a; 6],
            data: encode(&CapabilityTable {
                availability: CapabilityAvailability::Available,
                table,
            })
            .unwrap(),
        }
    }

    #[test]
    fn subscriptions_are_byte_exact_and_never_become_passive_reads() {
        for topic in [
            CapabilityTopic::VideoFormats,
            CapabilityTopic::SlowMotionRatios,
            CapabilityTopic::CaptureAspects,
        ] {
            let request = CapabilitySubscribe::new(topic, 0x12345678);
            let name = topic.name();
            let mut expected = vec![2, 2, 0, 0, 0x78, 0x56, 0x34, 0x12, 0, 0, 0];
            expected.extend_from_slice(&(u16::try_from(name.len()).unwrap() + 6).to_le_bytes());
            expected.extend_from_slice(&u16::try_from(name.len()).unwrap().to_le_bytes());
            expected.extend_from_slice(name);
            expected.extend_from_slice(&[0; 4]);
            assert_eq!(request.payload().unwrap(), expected);
            let raw = request.exploration_request(Duration::from_secs(1)).unwrap();
            assert_eq!(raw.receiver, Endpoint::Presence);
            assert_eq!(
                raw.command,
                Command::Common(CommonCommand::CapabilityTopics)
            );
            assert!(!raw.is_known_read());
            for (session, operation) in [(false, false), (true, false), (false, true)] {
                assert!(raw.validate(session, operation).is_err());
            }

            raw.validate(true, true).unwrap();
            assert!(
                request
                    .exploration_request(Duration::ZERO)
                    .unwrap()
                    .validate(true, true)
                    .is_err()
            );
        }
    }

    #[test]
    fn subscription_reply_checks_headers_and_retains_rejected_status() {
        let subscription = CapabilitySubscribe::new(CapabilityTopic::VideoFormats, 7);
        let reply = Message {
            sender: Endpoint::Presence,
            receiver: Endpoint::App,
            id: 1,
            flags: Flags::RESPONSE,
            command: subscription.command(),
            payload: vec![0xe3, 0xa5],
        };
        assert!(matches!(
            subscription.decode_reply(&reply).unwrap().value,
            CameraReadReply::Rejected {
                status: ResponseStatus::Unknown(0xe3),
                ..
            }
        ));
        for invalid in [
            Message {
                sender: Endpoint::Camera,
                ..reply.clone()
            },
            Message {
                receiver: Endpoint::Camera,
                ..reply.clone()
            },
            Message {
                flags: Flags::REQUEST,
                ..reply.clone()
            },
            Message {
                command: Command::from_wire(0, 1),
                ..reply
            },
        ] {
            assert!(subscription.decode_reply(&invalid).is_err());
        }
    }

    #[test]
    fn subscription_rejections_and_opaque_reply_bytes_survive() {
        for status in [0, 1, 0xe3] {
            let bytes = [0, 0x99, status, 0xa5];
            let reply = decode::<CapabilitySubscriptionReply>(&bytes, ()).unwrap();
            match reply.value {
                CameraReadReply::Accepted(data) => {
                    assert_eq!(status, 0);
                    assert_eq!(data.as_slice(), [0xa5]);
                }
                CameraReadReply::Rejected {
                    status: value,
                    extension,
                } => {
                    assert_eq!(value, ResponseStatus::Unknown(status));
                    assert_eq!(extension.as_slice(), [0xa5]);
                }
            }
        }
        assert!(decode::<CapabilitySubscriptionReply>(&[0, 0x98, 0], ()).is_err());
        let raw = CapabilitySubscribe::new(CapabilityTopic::VideoFormats, 7)
            .exploration_request(Duration::from_secs(1))
            .unwrap()
            .message(0x1234);
        assert_eq!(raw.receiver, Endpoint::Presence);
        assert_eq!(
            encode(&decode::<Message>(&encode(&raw).unwrap(), ()).unwrap()).unwrap(),
            encode(&raw).unwrap()
        );
    }

    #[test]
    fn notifications_and_available_rows_round_trip_unknown_values_and_offsets() {
        let value = notification(
            CapabilityTopic::VideoFormats,
            vec![2, 0x10, 3, 0, 0xfc, 0xfd, 0xa5],
        );
        let bytes = encode(&value).unwrap();
        let decoded = decode::<CapabilityNotification>(&bytes, ()).unwrap();
        assert_eq!(decoded, value);
        let rows = decoded.video_formats().unwrap().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].resolution, VideoResolutionCode::Unknown(0xfc));
        assert_eq!(rows[1].frame_rate, VideoFrameRateCode::Unknown(0xfd));
        assert_eq!(rows[1].opaque, 0xa5);
        assert!(decoded.slow_motion_ratios().is_err());
        for length in 0..bytes.len() {
            assert!(decode::<CapabilityNotification>(&bytes[..length], ()).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(decode::<CapabilityNotification>(&trailing, ()).is_err());
        for endian in [Endian::Little, Endian::Big] {
            let mut writer = Cursor::new(vec![0xa5; bytes.len() + 4]);
            writer.set_position(2);
            value.write_options(&mut writer, endian, ()).unwrap();
            assert_eq!(&writer.get_ref()[2..2 + bytes.len()], bytes);
            writer.set_position(2);
            assert_eq!(
                CapabilityNotification::read_options(&mut writer, endian, ()).unwrap(),
                value
            );
        }
        let mut unknown = value;
        unknown.name = b"synthetic_unknown_topic".to_vec();
        assert_eq!(
            decode::<CapabilityNotification>(&encode(&unknown).unwrap(), ()).unwrap(),
            unknown
        );
        assert!(unknown.topic().is_none());
        assert!(unknown.video_formats().is_err());
    }

    #[test]
    fn notification_acknowledgments_preserve_ids_and_reject_unrelated_messages() {
        let value = notification(CapabilityTopic::VideoFormats, vec![0]);
        let mut message =
            Message::from_command_body(Endpoint::App, 0x4321, Flags::REQUEST, &value).unwrap();
        message.sender = Endpoint::Presence;
        let reply = CapabilityNotification::acknowledgment(&message).unwrap();
        assert_eq!(reply.sender, Endpoint::App);
        assert_eq!(reply.receiver, Endpoint::Presence);
        assert_eq!(reply.id, message.id);
        assert_eq!(reply.command, message.command);
        assert_eq!(reply.flags, Flags::ACKNOWLEDGMENT);
        assert_eq!(reply.payload, [0]);
        for changed in [
            Message {
                sender: Endpoint::Camera,
                ..message.clone()
            },
            Message {
                receiver: Endpoint::Camera,
                ..message.clone()
            },
            Message {
                flags: Flags::RESPONSE,
                ..message.clone()
            },
            Message {
                command: Command::Common(CommonCommand::DeviceInfo),
                ..message.clone()
            },
            Message {
                payload: vec![0],
                ..message
            },
        ] {
            assert!(CapabilityNotification::acknowledgment(&changed).is_err());
        }
    }

    #[test]
    fn counts_lengths_availability_and_topic_record_widths_are_checked() {
        for table in [
            vec![],
            vec![1],
            vec![1, 0x10, 3],
            vec![0, 0xa5],
            vec![2, 0x10, 3, 0],
        ] {
            assert!(
                notification(CapabilityTopic::VideoFormats, table)
                    .video_formats()
                    .is_err()
            );
        }
        let empty = notification(CapabilityTopic::VideoFormats, vec![0]);
        assert_eq!(empty.video_formats().unwrap(), Some(vec![]));
        let mut unavailable = empty.clone();
        unavailable.data = vec![0, 2, 0, 0xa5, 0x5a];
        assert_eq!(unavailable.video_formats().unwrap(), None);
        unavailable.data[0] = 0xfe;
        assert!(unavailable.video_formats().is_err());
        let data = decode::<CapabilityTable>(&unavailable.data, ()).unwrap();
        assert_eq!(encode(&data).unwrap(), unavailable.data);
        for data in [vec![], vec![1], vec![1, 1, 0], vec![1, 0, 0, 0xa5]] {
            let mut malformed = empty.clone();
            malformed.data = data;
            assert!(malformed.video_formats().is_err());
        }
        let slow = notification(CapabilityTopic::SlowMotionRatios, vec![2, 7, 4, 0xfd, 0xa5]);
        assert_eq!(
            slow.slow_motion_ratios().unwrap().unwrap()[1],
            SlowMotionCapability {
                frame_rate: VideoFrameRateCode::Unknown(0xfd),
                factor: 0xa5
            }
        );
        let aspect = notification(CapabilityTopic::CaptureAspects, vec![3, 1, 2, 0xfe]);
        assert_eq!(aspect.capture_aspects().unwrap(), Some(vec![1, 2, 0xfe]));
        let mut oversized = empty;
        oversized.name = vec![0; usize::from(u16::MAX) + 1];
        assert!(encode(&oversized).is_err());
        oversized.name = CapabilityTopic::VideoFormats.name().to_vec();
        oversized.data = vec![0; usize::from(u16::MAX) + 1];
        assert!(encode(&oversized).is_err());
    }
}
