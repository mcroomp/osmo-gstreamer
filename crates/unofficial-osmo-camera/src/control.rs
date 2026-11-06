use crate::protocol::binrw_error;
use crate::protocol::encode;
use crate::{Error, Result, protocol::*};
use binrw::{BinRead, BinResult, Endian};
use std::collections::VecDeque;
use std::io::{Read, Seek};

#[binrw::binread]
#[br(little)]
struct ControlAckHeader {
    #[br(temp)]
    _windows: [u8; CONTROL_ACK_OFFSET],
    left: u16,
    right: u16,
    count: u16,
}

#[derive(Debug)]
pub struct ControlAck {
    pub left: u16,
    pub right: u16,
    pub missing: Vec<u16>,
}
impl ControlAck {
    pub fn parse(payload: &[u8], encoding: AckEncoding) -> Result<Self> {
        if payload.len() < ACK_LIST {
            return Err(Error::Protocol("Truncated control ACK"));
        }
        Self::read_options(
            &mut std::io::Cursor::new(payload),
            Endian::Little,
            (&encoding,),
        )
        .map_err(|source| codec_error("decode", source))
    }
}

impl BinRead for ControlAck {
    type Args<'a> = (&'a AckEncoding,);

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        _: Endian,
        (encoding,): Self::Args<'_>,
    ) -> BinResult<Self> {
        let pos = reader.stream_position()?;
        let header = ControlAckHeader::read_options(reader, Endian::Little, ())?;
        let left = header.left;
        let right = header.right;
        let count = usize::from(header.count);
        if distance(left, right) > CONTROL_CAPACITY || count > usize::from(CONTROL_CAPACITY) {
            return Err(binrw_error(
                pos,
                Error::Protocol("Control ACK exceeds capacity"),
            ));
        }
        let mut missing = Vec::new();
        match encoding {
            AckEncoding::List => {
                missing = Vec::<u16>::read_options(
                    reader,
                    Endian::Little,
                    binrw::VecArgs { count, inner: () },
                )?;
            }
            AckEncoding::Compressed => {
                let displacement = u16::read_options(reader, Endian::Little, ())?;
                let bitmap = Vec::<u8>::read_options(
                    reader,
                    Endian::Little,
                    binrw::VecArgs {
                        count: count.div_ceil(ACK_STATES_PER_BYTE),
                        inner: (),
                    },
                )?;
                for i in 0..count {
                    let state = (bitmap[i / ACK_STATES_PER_BYTE]
                        >> (ACK_STATE_BITS * (i % ACK_STATES_PER_BYTE)))
                        & ACK_STATE_MASK;
                    if state != ACK_RECEIVED {
                        let index = ((left >> SEQUENCE_SHIFT)
                            .wrapping_add(displacement)
                            .wrapping_add(i as u16))
                            & INDEX_MASK;
                        missing.push((index << SEQUENCE_SHIFT) | (u16::from(state) << RETRY_SHIFT));
                    }
                }
            }
        }
        let embedded_length = u16::read_options(reader, Endian::Little, ())?;
        let copied = std::io::copy(
            &mut reader.take(u64::from(embedded_length)),
            &mut std::io::sink(),
        )?;
        if copied != u64::from(embedded_length) {
            return Err(binrw_error(
                pos,
                Error::Protocol("Truncated embedded ACK data"),
            ));
        }
        let mut previous = 0;
        for (i, record) in missing.iter().enumerate() {
            let position = distance(left, *record);
            if position <= previous || position > distance(left, right) || (i == 0 && position != 1)
            {
                return Err(binrw_error(
                    pos,
                    Error::Protocol("Invalid ordered control ACK gaps"),
                ));
            }
            previous = position;
        }
        Ok(Self {
            left,
            right,
            missing,
        })
    }
}

struct Pending {
    sequence: u16,
    counter: u8,
    message: Message,
}

pub struct ControlSender {
    pub left: u16,
    pub right: u16,
    first_retry: u16,
    second_retry: u16,
    counter: u8,
    pending: VecDeque<Pending>,
}
impl ControlSender {
    pub fn has_reply(&self, message: &Message) -> bool {
        self.pending.iter().any(|pending| {
            let sent = &pending.message;
            sent.sender == message.sender
                && sent.receiver == message.receiver
                && sent.id == message.id
                && sent.command == message.command
                && sent.flags == message.flags
        })
    }
    pub fn new(base: u16) -> Self {
        Self {
            left: base,
            right: base,
            first_retry: 0,
            second_retry: 0,
            counter: 0,
            pending: VecDeque::new(),
        }
    }
    pub fn can_send(&self) -> bool {
        distance(self.left, self.right) < CONTROL_CAPACITY
    }
    fn routing(&self, right: u16, counter: u8, retry: Option<u16>) -> Vec<u8> {
        let window = SenderWindow {
            left: self.left,
            right,
            first_retry: retry
                .filter(|s| RetryPhase::of(*s) == RetryPhase::First)
                .map_or(self.first_retry, |s| (s & INDEX_BITS) | RETRY_MARKER),
            second_retry: retry
                .filter(|s| RetryPhase::of(*s) == RetryPhase::Second)
                .map_or(self.second_retry, |s| (s & INDEX_BITS) | RETRY_MARKER),
        };
        encode(&(window, counter, CONTROL_CHANNEL, [0u8; 2]))
            .expect("fixed routing header encodes into memory")
    }
    pub fn send(&mut self, message: Message, session: u16) -> Result<Vec<u8>> {
        if !self.can_send() {
            return Err(Error::InvalidState(
                "Control window is full; no automatic reconnect",
            ));
        }
        let sequence = next_sequence(self.right);
        self.counter = self.counter.wrapping_add(1);
        let mut payload = self.routing(sequence, self.counter, None);
        payload.extend(encode(&message)?);
        let packet = datagram(PacketType::RoutedCommand, session, sequence, &payload)?;
        self.pending.push_back(Pending {
            sequence,
            counter: self.counter,
            message,
        });
        self.right = sequence;
        Ok(packet)
    }
    pub fn observe(
        &mut self,
        bytes: &[u8],
        encoding: AckEncoding,
        session: u16,
    ) -> Result<Vec<Vec<u8>>> {
        let ack = ControlAck::parse(bytes, encoding)?;
        let span = distance(self.left, self.right);
        let left = distance(self.left, ack.left);
        let right = distance(self.left, ack.right);
        if left > span || right > span {
            return Err(Error::Protocol("Stale or future control ACK"));
        }
        let old_left = self.left;
        let mut retries = Vec::new();
        self.pending
            .retain(|p| distance(old_left, p.sequence) > left);
        if !ack.missing.is_empty() {
            self.pending.retain(|p| {
                distance(old_left, p.sequence) > right
                    || ack
                        .missing
                        .iter()
                        .any(|m| m >> SEQUENCE_SHIFT == p.sequence >> SEQUENCE_SHIFT)
            });
            for pending in &mut self.pending {
                if let Some(record) = ack
                    .missing
                    .iter()
                    .find(|r| *r >> SEQUENCE_SHIFT == pending.sequence >> SEQUENCE_SHIFT)
                {
                    let phase = RetryPhase::of(*record);
                    if phase != RetryPhase::Final
                        && (phase as u16) >= RetryPhase::of(pending.sequence) as u16
                    {
                        pending.sequence = (pending.sequence & !RETRY_MASK)
                            | (record.wrapping_add(1 << RETRY_SHIFT) & RETRY_MASK);
                        retries.push((
                            pending.sequence,
                            pending.counter,
                            encode(&pending.message)?,
                        ));
                    }
                }
            }
        }
        self.left = self.pending.front().map_or(self.right, |p| {
            p.sequence.wrapping_sub(SEQUENCE_STEP) & INDEX_BITS
        });
        if self.first_retry & RETRY_MARKER != 0
            && distance(self.first_retry, self.left) < CONTROL_CAPACITY
        {
            self.first_retry = 0;
        }
        if self.second_retry & RETRY_MARKER != 0
            && distance(self.second_retry, self.left) < CONTROL_CAPACITY
        {
            self.second_retry = 0;
        }
        let mut packets = Vec::new();
        for (sequence, counter, encoded) in retries {
            let mut payload = self.routing(self.right, counter, Some(sequence));
            payload.extend(encoded);
            packets.push(datagram(
                PacketType::RoutedCommand,
                session,
                sequence,
                &payload,
            )?);
            match RetryPhase::of(sequence) {
                RetryPhase::First => self.first_retry = (sequence & INDEX_BITS) | RETRY_MARKER,
                RetryPhase::Second => self.second_retry = (sequence & INDEX_BITS) | RETRY_MARKER,
                _ => {}
            }
        }
        Ok(packets)
    }
    pub fn acknowledgment(
        &self,
        encoding: AckEncoding,
        video: u16,
        download: u16,
        feedback: Option<Vec<u8>>,
    ) -> Vec<u8> {
        let block = match encoding {
            AckEncoding::List => RECEIVE_LIST_HEADER,
            AckEncoding::Compressed => RECEIVE_COMPRESSED_HEADER,
        };
        let mut bytes = feedback.unwrap_or_else(|| {
            let mut bytes = vec![0; block];
            put16(&mut bytes, LEFT, video);
            put16(&mut bytes, RIGHT, video);
            bytes
        });
        let offset = bytes.len();
        bytes.resize(offset + block + WINDOW_LENGTH + WORD, 0);
        put16(&mut bytes, offset + LEFT, download);
        put16(&mut bytes, offset + RIGHT, download);
        put16(&mut bytes, offset + block + LEFT, self.left);
        put16(&mut bytes, offset + block + RIGHT, self.right);
        put16(&mut bytes, offset + block + FIRST_RETRY, self.first_retry);
        put16(&mut bytes, offset + block + SECOND_RETRY, self.second_retry);
        bytes
    }
}
