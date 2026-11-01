use crate::{Error, Result, protocol::*};
use std::{collections::BTreeMap, time::Instant};

#[derive(Debug)]
/// One reconstructed Annex-B H.264 access unit.
pub struct AccessUnit {
    pub bytes: Vec<u8>,
    /// Monotonic local receiver counter, not a camera frame identifier.
    pub frame_number: u64,
    /// Local monotonic reconstruction time, not camera capture time.
    pub received_at: Instant,
    pub camera_metadata: u32,
    pub keyframe: bool,
    /// Receiver discontinuity; camera pauses may require adapter-level detection.
    pub discontinuity: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct Fragment {
    sequence: u16,
    fec: u32,
    payload: Vec<u8>,
}
impl Fragment {
    fn parse(bytes: &[u8], session: u16) -> Result<Self> {
        if bytes.len() < VIDEO_PAYLOAD
            || bytes.len() > MAX_DATAGRAM
            || !valid_datagram(bytes, session)
            || bytes[TYPE_OFFSET] != PacketType::Video.to_wire_u8()
        {
            return Err(Error::Protocol("Invalid video datagram"));
        }
        let fec = u32_at(bytes, FEC_OFFSET);
        let mut length = bytes.len() - VIDEO_PAYLOAD;
        if fec & FEC_ENABLED != 0 {
            let count = (fec >> FEC_COUNT_SHIFT) & FEC_FIELD_MASK;
            let index = (fec >> FEC_INDEX_SHIFT) & FEC_FIELD_MASK;
            let shards = count
                + if count == 1 {
                    SINGLE_PARITY
                } else {
                    MULTIPLE_PARITY
                };
            if count == 0 || index >= shards {
                return Err(Error::Protocol("Invalid FEC count/index"));
            }
            if index == count - 1 {
                let valid = (fec >> FEC_LENGTH_SHIFT) as usize;
                if valid > length {
                    return Err(Error::Protocol("FEC valid length exceeds payload"));
                }
                length = valid;
            }
        }
        Ok(Self {
            sequence: u16_at(bytes, SEQUENCE_OFFSET),
            fec,
            payload: bytes[VIDEO_PAYLOAD..VIDEO_PAYLOAD + length].to_vec(),
        })
    }
    fn parity(&self) -> bool {
        self.fec & FEC_ENABLED != 0
            && ((self.fec >> FEC_INDEX_SHIFT) & FEC_FIELD_MASK)
                >= ((self.fec >> FEC_COUNT_SHIFT) & FEC_FIELD_MASK)
    }
}

fn envelope_header(bytes: &[u8]) -> bool {
    bytes.len() >= ENVELOPE_HEADER
        && bytes[..ENVELOPE_PREFIX.len()] == ENVELOPE_PREFIX
        && bytes[..ENVELOPE_CHECKSUM].iter().fold(0, |a, b| a ^ b) == bytes[ENVELOPE_CHECKSUM]
}

#[derive(Default)]
struct Envelopes {
    bytes: Vec<u8>,
}
impl Envelopes {
    fn feed(&mut self, bytes: &[u8]) -> Result<Vec<(u32, Vec<u8>)>> {
        if bytes.len() > MAX_DATAGRAM
            || self.bytes.len() + bytes.len()
                > MAX_MEDIA + usize::from(EXTENSION_LENGTH_MASK) + MAX_DATAGRAM
        {
            return Err(Error::LimitExceeded("Media envelope buffer limit exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        let mut cursor = 0;
        let mut packets = Vec::new();
        while self.bytes.len() - cursor >= ENVELOPE_HEADER {
            let header = &self.bytes[cursor..];
            if !envelope_header(header) {
                return Err(Error::Protocol(
                    "Media envelope desynchronized; refusing to skip compressed pictures",
                ));
            }
            let length = u32_at(header, ENVELOPE_LENGTH) as usize;
            if length > MAX_MEDIA {
                return Err(Error::LimitExceeded("Media envelope exceeds 2 MiB limit"));
            }
            let extended = header[ENVELOPE_EXTENSION] & EXTENSION_MASK == EXTENSION_MARKER;
            let header_length = if extended {
                usize::from(header[ENVELOPE_EXTENSION] & EXTENSION_LENGTH_MASK)
            } else {
                ENVELOPE_HEADER
            };
            if extended && header_length <= ENVELOPE_HEADER {
                return Err(Error::Protocol("Invalid extended media header"));
            }
            if extended && header.len() < METADATA_HEADER {
                break;
            }
            if header.len() < header_length + length {
                break;
            }
            let metadata = if extended {
                u32_at(header, ENVELOPE_HEADER)
            } else {
                0
            };
            match MediaType::try_from(header[ENVELOPE_TYPE])? {
                MediaType::Video => packets.push((
                    metadata,
                    header[header_length..header_length + length].to_vec(),
                )),
                MediaType::Audio | MediaType::Command => {}
            }
            cursor += header_length + length;
        }
        self.bytes.drain(..cursor);
        Ok(packets)
    }
}

pub fn clean_access_unit(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut cleaned = Vec::with_capacity(bytes.len());
    let mut cursor = 0;
    while let Some(relative) = bytes[cursor..]
        .windows(PRIVATE_SEI.len())
        .position(|w| w == PRIVATE_SEI)
    {
        let marker = cursor + relative;
        let end = marker + PRIVATE_SEI.len() + PRIVATE_SEI_DATA + 1;
        if end > bytes.len() || bytes[end - 1] != PRIVATE_SEI_END {
            return Err(Error::Protocol("Malformed Nano private SEI"));
        }
        let start = if marker > cursor && bytes[marker - 1] == 0 {
            marker - 1
        } else {
            marker
        };
        cleaned.extend_from_slice(&bytes[cursor..start]);
        cursor = end;
    }
    cleaned.extend_from_slice(&bytes[cursor..]);
    if cleaned.ends_with(TRAILING_AUD) {
        cleaned.truncate(cleaned.len() - TRAILING_AUD.len());
    }
    Ok(cleaned)
}

pub struct Nal<'a> {
    pub start: usize,
    pub end: usize,
    pub bytes: &'a [u8],
}
pub fn nals(frame: &[u8]) -> Vec<Nal<'_>> {
    let mut boundaries = Vec::new();
    let mut zeros = 0;
    for (i, byte) in frame.iter().enumerate() {
        if *byte == PREFIX_END && zeros >= MIN_PREFIX_ZEROS {
            boundaries.push((i - zeros, i + 1));
        }
        zeros = if *byte == 0 {
            (zeros + 1).min(MAX_PREFIX_ZEROS)
        } else {
            0
        };
    }
    boundaries
        .iter()
        .enumerate()
        .filter_map(|(i, (start, header))| {
            let end = boundaries.get(i + 1).map_or(frame.len(), |b| b.0);
            (*header < end).then(|| Nal {
                start: *start,
                end,
                bytes: &frame[*header..end],
            })
        })
        .collect()
}

#[derive(Default)]
struct Joiner {
    sps: Option<Vec<u8>>,
    pps: Option<Vec<u8>>,
    hevc: [bool; 3],
    joined: bool,
    picture_number: u64,
}
impl Joiner {
    fn prepare(&mut self, frame: Vec<u8>) -> Result<Option<(Vec<u8>, bool, bool)>> {
        let mut idr = None;
        let mut picture = false;
        let mut sps_in_frame = false;
        let mut pps_in_frame = false;
        for nal in nals(&frame) {
            if nal.bytes[0] & NAL_FORBIDDEN != 0 {
                return Err(Error::Protocol("Forbidden bit in H.264 NAL"));
            }
            let kind = nal.bytes[0] & NAL_TYPE_MASK;
            if nal.bytes.len() >= PPS_PROBE && nal.bytes[1] & HEVC_TEMPORAL_MASK != 0 {
                let hevc = (nal.bytes[0] >> HEVC_TYPE_SHIFT) & HEVC_TYPE_MASK;
                for (i, candidate) in [HevcNal::Vps, HevcNal::Sps, HevcNal::Pps]
                    .iter()
                    .enumerate()
                {
                    if hevc == *candidate as u8 {
                        self.hevc[i] = true;
                    }
                }
            }
            if kind == AvcNal::Slice as u8 || kind == AvcNal::Idr as u8 {
                picture = true;
                validate_slice(nal.bytes)?;
            }
            if nal.bytes[0] & NAL_REFERENCE == 0 {
                continue;
            }
            if idr.is_none() {
                if kind == AvcNal::Sps as u8 && nal.bytes.len() >= SPS_PROBE {
                    self.sps = Some(frame[nal.start..nal.end].to_vec());
                    sps_in_frame = true;
                }
                if kind == AvcNal::Pps as u8 && nal.bytes.len() >= PPS_PROBE {
                    self.pps = Some(frame[nal.start..nal.end].to_vec());
                    pps_in_frame = true;
                }
                if kind == AvcNal::Idr as u8 {
                    idr = Some(nal.start);
                }
            }
        }
        if self.hevc.iter().all(|v| *v) {
            return Err(Error::Unsupported(
                "Only unambiguous H.264 is supported; H.265 headers received",
            ));
        }
        if !picture {
            return Ok(None);
        }
        self.picture_number = self
            .picture_number
            .checked_add(1)
            .ok_or(Error::LimitExceeded("Preview picture counter overflow"))?;
        if !self.joined {
            let (Some(sps), Some(pps), Some(offset)) = (&self.sps, &self.pps, idr) else {
                return Ok(None);
            };
            let mut joined = Vec::with_capacity(sps.len() + pps.len() + frame.len() - offset);
            joined.extend_from_slice(sps);
            joined.extend_from_slice(pps);
            joined.extend_from_slice(&frame[offset..]);
            self.joined = true;
            return Ok(Some((joined, true, true)));
        }
        // Header-only envelopes are cached above; prepend them when an IDR arrives separately.
        if let Some(offset) = idr.filter(|_| !sps_in_frame || !pps_in_frame) {
            let mut joined = self
                .sps
                .clone()
                .ok_or(Error::InvalidState("Missing cached SPS"))?;
            joined.extend_from_slice(
                self.pps
                    .as_ref()
                    .ok_or(Error::InvalidState("Missing cached PPS"))?,
            );
            joined.extend_from_slice(&frame[offset..]);
            return Ok(Some((joined, true, false)));
        }
        Ok(Some((frame, idr.is_some(), false)))
    }
}

fn validate_slice(nal: &[u8]) -> Result<()> {
    let mut rbsp = Vec::new();
    let mut zeros = 0;
    for byte in &nal[1..] {
        if zeros >= MIN_PREFIX_ZEROS && *byte == EMULATION_PREVENTION {
            zeros = 0;
            continue;
        }
        rbsp.push(*byte);
        zeros = if *byte == 0 { zeros + 1 } else { 0 };
    }
    let mut bit = 0;
    let mut read_ue = || -> Result<u32> {
        let mut zeros = 0;
        loop {
            if bit >= rbsp.len() * u8::BITS as usize || zeros >= u32::BITS - 1 {
                return Err(Error::Protocol("Invalid H.264 slice header"));
            }
            let value = rbsp[bit / u8::BITS as usize]
                & (1 << (u8::BITS as usize - 1 - bit % u8::BITS as usize));
            bit += 1;
            if value != 0 {
                break;
            }
            zeros += 1;
        }
        let mut value = 1u32;
        for _ in 0..zeros {
            if bit >= rbsp.len() * u8::BITS as usize {
                return Err(Error::Protocol("Truncated H.264 slice header"));
            }
            value = (value << 1)
                | u32::from(
                    (rbsp[bit / u8::BITS as usize]
                        >> (u8::BITS as usize - 1 - bit % u8::BITS as usize))
                        & 1,
                );
            bit += 1;
        }
        Ok(value - 1)
    };
    read_ue()?;
    let slice_type = read_ue()?;
    if slice_type > MAX_SLICE_TYPE || slice_type % SLICE_TYPE_COUNT == B_SLICE {
        return Err(Error::Unsupported(
            "H.264 B slices or invalid slice types are unsupported",
        ));
    }
    Ok(())
}

pub struct PreviewReceiver {
    session: u16,
    next: Option<u16>,
    highest: Option<u16>,
    pending: BTreeMap<u16, Fragment>,
    phases: BTreeMap<u16, RetryPhase>,
    envelopes: Envelopes,
    joiner: Joiner,
    fault: Option<Error>,
}
impl PreviewReceiver {
    pub fn new(session: u16) -> Self {
        Self {
            session,
            next: None,
            highest: None,
            pending: BTreeMap::new(),
            phases: BTreeMap::new(),
            envelopes: Envelopes::default(),
            joiner: Joiner::default(),
            fault: None,
        }
    }
    fn furthest(&self) -> Option<u16> {
        let gap = distance(self.next?, self.highest?);
        (gap < HALF_CYCLE).then_some(gap)
    }
    pub fn cursor(&self) -> Option<u16> {
        self.next.map(|s| s.wrapping_sub(SEQUENCE_STEP))
    }
    pub fn observe_window(&mut self, window: &[u8]) -> Result<()> {
        if window.len() < WINDOW_LENGTH {
            return Err(Error::Protocol("Truncated video sender window"));
        }
        let Some(next) = self.next else {
            return Ok(());
        };
        let left = u16_at(window, LEFT);
        let right = u16_at(window, RIGHT);
        if distance(left, right) > VIDEO_CAPACITY || distance(left, next) > VIDEO_CAPACITY {
            return Ok(());
        }
        if distance(left, right) != 0 {
            self.observe_high(right);
        }
        if let Some(furthest) = self.furthest() {
            for gap in 0..=furthest {
                let sequence = next.wrapping_add(gap * SEQUENCE_STEP) & INDEX_BITS;
                if self.pending.contains_key(&sequence) {
                    continue;
                }
                let mut phase = self
                    .phases
                    .get(&sequence)
                    .copied()
                    .unwrap_or(RetryPhase::Initial);
                for (offset, from, to) in [
                    (FIRST_RETRY, RetryPhase::Initial, RetryPhase::First),
                    (SECOND_RETRY, RetryPhase::First, RetryPhase::Second),
                ] {
                    let marker = u16_at(window, offset);
                    if phase == from
                        && marker & RETRY_MARKER != 0
                        && distance(sequence, marker) < VIDEO_CAPACITY
                    {
                        phase = to;
                    }
                }
                self.phases.insert(sequence, phase);
            }
        }
        Ok(())
    }
    fn observe_high(&mut self, sequence: u16) {
        if let Some(next) = self.next {
            let gap = distance(next, sequence);
            if gap < VIDEO_CAPACITY && self.furthest().is_none_or(|current| gap > current) {
                self.highest = Some(sequence & INDEX_BITS);
            }
        }
    }
    pub fn feed(&mut self, bytes: &[u8], received_at: Instant) -> Result<Vec<AccessUnit>> {
        if let Some(error) = &self.fault {
            return Err(error.clone());
        }
        let result = self.feed_inner(bytes, received_at);
        if let Err(error) = &result {
            self.fault = Some(error.clone());
        }
        result
    }
    fn feed_inner(&mut self, bytes: &[u8], received_at: Instant) -> Result<Vec<AccessUnit>> {
        let fragment = Fragment::parse(bytes, self.session)?;
        if self.next.is_none() {
            if fragment.parity() || !envelope_header(&fragment.payload) {
                return Ok(Vec::new());
            }
            self.next = Some(fragment.sequence & INDEX_BITS);
        }
        let next = self.next.expect("initialized receive sequence");
        self.observe_window(&bytes[UDP_HEADER..UDP_HEADER + WINDOW_LENGTH])?;
        let sequence = fragment.sequence & INDEX_BITS;
        let gap = distance(next, sequence);
        if gap > HALF_CYCLE {
            return Ok(Vec::new());
        }
        if gap >= VIDEO_CAPACITY {
            return Err(Error::Protocol(
                "Unrecovered video gap exceeds window; no RS recovery or skipping",
            ));
        }
        if let Some(previous) = self.pending.get(&sequence) {
            if previous.fec != fragment.fec || previous.payload != fragment.payload {
                return Err(Error::Protocol("Conflicting duplicate video fragment"));
            }
            return Ok(Vec::new());
        }
        self.observe_high(sequence);
        self.phases.remove(&sequence);
        self.pending.insert(sequence, fragment);
        let mut frames = Vec::new();
        let mut next = next;
        while let Some(fragment) = self.pending.remove(&next) {
            if !fragment.parity() {
                for (metadata, payload) in self.envelopes.feed(&fragment.payload)? {
                    let cleaned = clean_access_unit(&payload)?;
                    if let Some((bytes, keyframe, discontinuity)) = self.joiner.prepare(cleaned)? {
                        frames.push(AccessUnit {
                            bytes,
                            frame_number: self.joiner.picture_number,
                            received_at,
                            camera_metadata: metadata,
                            keyframe,
                            discontinuity,
                        });
                    }
                }
            }
            next = next_sequence(next) & INDEX_BITS;
        }
        self.next = Some(next);
        Ok(frames)
    }
    pub fn acknowledgment(&self, encoding: AckEncoding) -> Option<Vec<u8>> {
        let next = self.next?;
        let furthest = self.furthest();
        let missing: Vec<_> = furthest.map_or_else(Vec::new, |last| {
            (0..=last)
                .filter(|gap| {
                    !self
                        .pending
                        .contains_key(&(next.wrapping_add(gap * SEQUENCE_STEP) & INDEX_BITS))
                })
                .collect()
        });
        let count = match encoding {
            AckEncoding::List => missing.len(),
            AckEncoding::Compressed => missing
                .first()
                .zip(missing.last())
                .map_or(0, |(a, b)| usize::from(b - a + 1)),
        };
        let header = match encoding {
            AckEncoding::List => RECEIVE_LIST_HEADER,
            AckEncoding::Compressed => RECEIVE_COMPRESSED_HEADER,
        };
        let size = match encoding {
            AckEncoding::List => count * WORD,
            AckEncoding::Compressed => count.div_ceil(ACK_STATES_PER_BYTE),
        };
        let mut bytes = vec![0; header + size];
        put16(&mut bytes, LEFT, next.wrapping_sub(SEQUENCE_STEP));
        put16(
            &mut bytes,
            RIGHT,
            furthest.map_or(next.wrapping_sub(SEQUENCE_STEP), |gap| {
                next.wrapping_add(gap * SEQUENCE_STEP)
            }),
        );
        put16(&mut bytes, WORD * 2, count as u16);
        if encoding == AckEncoding::Compressed {
            put16(
                &mut bytes,
                RECEIVE_LIST_HEADER,
                missing.first().map_or(0, |gap| gap + 1),
            );
            bytes[header..].fill(u8::MAX);
        }
        for (i, gap) in missing.iter().enumerate() {
            let sequence = next.wrapping_add(gap * SEQUENCE_STEP) & INDEX_BITS;
            let phase = self
                .phases
                .get(&sequence)
                .copied()
                .unwrap_or(RetryPhase::Initial) as u16;
            match encoding {
                AckEncoding::List => put16(
                    &mut bytes,
                    header + i * WORD,
                    sequence | (phase << RETRY_SHIFT),
                ),
                AckEncoding::Compressed => {
                    let position = usize::from(gap - missing[0]);
                    let shift = ACK_STATE_BITS * (position % ACK_STATES_PER_BYTE);
                    bytes[header + position / ACK_STATES_PER_BYTE] &= !(ACK_STATE_MASK << shift);
                    bytes[header + position / ACK_STATES_PER_BYTE] |= (phase as u8) << shift;
                }
            }
        }
        Some(bytes)
    }
    pub fn complete(&self) -> Result<()> {
        if let Some(error) = &self.fault {
            return Err(error.clone());
        }
        if !self.pending.is_empty() || self.furthest().is_some() || !self.envelopes.bytes.is_empty()
        {
            return Err(Error::Protocol(
                "Preview ended with incomplete fragments or media envelope",
            ));
        }
        Ok(())
    }
}
