use crate::{
    control::*,
    preview::*,
    protocol::{
        identity::{Command, Endpoint, WifiCommand},
        *,
    },
    *,
};
use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

#[test]
fn duml_messages_round_trip_and_reject_corruption() {
    let message = Message::request(
        Endpoint::Wifi,
        FIRST_ID,
        Command::Wifi(WifiCommand::Country),
        &[],
    );
    let encoded = message.encode().unwrap();
    assert_eq!(Message::scan(&encoded), vec![message.clone()]);
    assert_eq!(Message::decode(&encoded, &()).unwrap(), message);

    let mut corrupt = encoded;
    corrupt[DUML_COMMAND] ^= 1;
    assert!(Message::scan(&corrupt).is_empty());
}

#[test]
fn control_ack_parses_synthetic_list_and_rejects_malformed_payloads() {
    let left = INDEX_BITS;
    let first = next_sequence(left);
    let right = next_sequence(first);
    let payload = control_status(left, right, &[first]);
    let ack = ControlAck::parse(&payload, AckEncoding::List).unwrap();
    assert_eq!((ack.left, ack.right), (left, right));
    assert_eq!(ack.missing, [first]);
    assert!(ControlAck::parse(&payload[..ACK_LIST], AckEncoding::List).is_err());
    assert!(ControlAck::parse(&payload[..payload.len() - 1], AckEncoding::List).is_err());

    let mut over_capacity = control_status(left, right, &[]);
    put16(&mut over_capacity, ACK_COUNT, CONTROL_CAPACITY + 1);
    assert!(ControlAck::parse(&over_capacity, AckEncoding::List).is_err());
}

fn control_status(left: u16, right: u16, missing: &[u16]) -> Vec<u8> {
    let mut bytes = vec![0; ACK_LIST + missing.len() * WORD + WORD];
    put16(&mut bytes, CONTROL_ACK_OFFSET, left);
    put16(&mut bytes, CONTROL_ACK_OFFSET + RIGHT, right);
    put16(&mut bytes, ACK_COUNT, missing.len() as u16);
    for (i, record) in missing.iter().enumerate() {
        put16(&mut bytes, ACK_LIST + i * WORD, *record);
    }
    bytes
}

#[test]
fn control_window_rollover_retries_and_capacity() {
    let base = INDEX_BITS;
    let mut sender = ControlSender::new(base);
    let message = Message::request(
        Endpoint::Wifi,
        FIRST_ID,
        Command::Wifi(WifiCommand::Country),
        &[],
    );
    let first = sender.send(message.clone(), INITIAL_SESSION).unwrap();
    let sequence = next_sequence(base);
    assert_eq!(u16_at(&first, SEQUENCE_OFFSET), sequence);
    let mut requested = sequence;
    for expected in [RetryPhase::First, RetryPhase::Second, RetryPhase::Final] {
        let retries = sender
            .observe(
                &control_status(base, sequence, &[requested]),
                AckEncoding::List,
                INITIAL_SESSION,
            )
            .unwrap();
        assert_eq!(retries.len(), 1);
        requested = u16_at(&retries[0], SEQUENCE_OFFSET);
        assert_eq!(RetryPhase::of(requested), expected);
        assert_eq!(Message::scan(&retries[0]), vec![message.clone()]);
    }
    assert!(
        sender
            .observe(
                &control_status(base, sequence, &[requested]),
                AckEncoding::List,
                INITIAL_SESSION
            )
            .unwrap()
            .is_empty()
    );
    sender
        .observe(
            &control_status(sequence, sequence, &[]),
            AckEncoding::List,
            INITIAL_SESSION,
        )
        .unwrap();
    assert_eq!(sender.left, sender.right);
    for _ in 0..CONTROL_CAPACITY {
        sender.send(message.clone(), INITIAL_SESSION).unwrap();
    }
    assert!(!sender.can_send());
    assert!(sender.send(message, INITIAL_SESSION).is_err());
}

// Byte-exact synthetic Annex-B fixture: SPS, PPS, IDR (I slice), then P/B slice variants.
const IDR_FIXTURE: &[u8] = &[
    0, 0, 0, 1, 0x67, 0x64, 0, 0x1f, 0, 0, 0, 1, 0x68, 0xee, 0, 0, 0, 1, 0x65, 0xb8,
];
const P_FIXTURE: &[u8] = &[0, 0, 0, 1, 0x41, 0xe0];
const B_FIXTURE: &[u8] = &[0, 0, 0, 1, 0x41, 0xa0];

pub(crate) fn media_envelope(payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; ENVELOPE_HEADER];
    bytes[..ENVELOPE_PREFIX.len()].copy_from_slice(&ENVELOPE_PREFIX);
    bytes[ENVELOPE_LENGTH..ENVELOPE_LENGTH + 4]
        .copy_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes[ENVELOPE_TYPE] = MediaType::Video as u8;
    bytes[ENVELOPE_CHECKSUM] = bytes[..ENVELOPE_CHECKSUM].iter().fold(0, |a, b| a ^ b);
    bytes.extend_from_slice(payload);
    bytes
}
pub(crate) fn video_datagram(session: u16, sequence: u16, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; VIDEO_PAYLOAD - UDP_HEADER];
    put16(&mut bytes, LEFT, sequence.wrapping_sub(SEQUENCE_STEP));
    put16(&mut bytes, RIGHT, sequence);
    bytes.extend_from_slice(payload);
    datagram(PacketType::Video, session, sequence, &bytes).unwrap()
}

#[test]
fn preview_preroll_reordering_gap_ack_and_terminal_loss() {
    let mut preview = PreviewReceiver::new(INITIAL_SESSION);
    assert!(
        preview
            .feed(&video_datagram(INITIAL_SESSION, 0, &[1, 2]), Instant::now())
            .unwrap()
            .is_empty()
    );
    assert!(
        preview
            .feed(
                &video_datagram(INITIAL_SESSION, SEQUENCE_STEP, &media_envelope(P_FIXTURE)),
                Instant::now()
            )
            .unwrap()
            .is_empty()
    );
    let first = SEQUENCE_STEP * 2;
    let frames = preview
        .feed(
            &video_datagram(INITIAL_SESSION, first, &media_envelope(IDR_FIXTURE)),
            Instant::now(),
        )
        .unwrap();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].bytes, IDR_FIXTURE);
    assert!(frames[0].keyframe && frames[0].discontinuity);
    let missing = next_sequence(first);
    let out_of_order = video_datagram(
        INITIAL_SESSION,
        next_sequence(missing),
        &media_envelope(P_FIXTURE),
    );
    assert!(
        preview
            .feed(&out_of_order, Instant::now())
            .unwrap()
            .is_empty()
    );
    assert!(
        preview
            .feed(&out_of_order, Instant::now())
            .unwrap()
            .is_empty()
    );
    let ack = preview.acknowledgment(AckEncoding::List).unwrap();
    assert_eq!(u16_at(&ack, WORD * 2), 1);
    assert_eq!(u16_at(&ack, RECEIVE_LIST_HEADER), missing);
    let mut window = vec![0; WINDOW_LENGTH];
    put16(&mut window, LEFT, first);
    put16(&mut window, RIGHT, next_sequence(missing));
    put16(&mut window, FIRST_RETRY, missing | RETRY_MARKER);
    preview.observe_window(&window).unwrap();
    assert_eq!(
        u16_at(
            &preview.acknowledgment(AckEncoding::List).unwrap(),
            RECEIVE_LIST_HEADER
        ),
        missing | ((RetryPhase::First as u16) << RETRY_SHIFT)
    );
    assert!(preview.complete().is_err());
    let released = preview
        .feed(
            &video_datagram(INITIAL_SESSION, missing, &media_envelope(P_FIXTURE)),
            Instant::now(),
        )
        .unwrap();
    assert_eq!(released.len(), 2);
    assert!(released.iter().all(|u| !u.keyframe && !u.discontinuity));
    preview.complete().unwrap();
    assert!(
        preview
            .feed(
                &video_datagram(
                    INITIAL_SESSION,
                    first.wrapping_add(VIDEO_CAPACITY * SEQUENCE_STEP + SEQUENCE_STEP * 4),
                    &media_envelope(P_FIXTURE)
                ),
                Instant::now()
            )
            .is_err()
    );
    assert!(preview.feed(&out_of_order, Instant::now()).is_err());
}

#[test]
fn fragment_boundaries_private_sei_and_b_slice_rejection() {
    let envelope = media_envelope(IDR_FIXTURE);
    for split in ENVELOPE_HEADER..envelope.len() {
        let mut preview = PreviewReceiver::new(INITIAL_SESSION);
        assert!(
            preview
                .feed(
                    &video_datagram(INITIAL_SESSION, 0, &envelope[..split]),
                    Instant::now()
                )
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            preview
                .feed(
                    &video_datagram(INITIAL_SESSION, SEQUENCE_STEP, &envelope[split..]),
                    Instant::now()
                )
                .unwrap()[0]
                .bytes,
            IDR_FIXTURE
        );
    }
    let mut sei = vec![0];
    sei.extend(PRIVATE_SEI);
    sei.extend(vec![0; PRIVATE_SEI_DATA]);
    sei.push(PRIVATE_SEI_END);
    sei.extend(IDR_FIXTURE);
    sei.extend(TRAILING_AUD);
    assert_eq!(clean_access_unit(&sei).unwrap(), IDR_FIXTURE);
    assert!(clean_access_unit(PRIVATE_SEI).is_err());
    let mut preview = PreviewReceiver::new(INITIAL_SESSION);
    preview
        .feed(
            &video_datagram(INITIAL_SESSION, 0, &envelope),
            Instant::now(),
        )
        .unwrap();
    assert!(
        preview
            .feed(
                &video_datagram(INITIAL_SESSION, SEQUENCE_STEP, &media_envelope(B_FIXTURE)),
                Instant::now()
            )
            .is_err()
    );
}

fn unit(bytes: usize) -> AccessUnit {
    AccessUnit {
        bytes: vec![0; bytes],
        frame_number: 1,
        received_at: Instant::now(),
        camera_metadata: 0,
        keyframe: true,
        discontinuity: true,
    }
}

#[test]
fn queue_flush_cancellation_resume_finish_and_overflow() {
    let reader = PreviewReader::default();
    let blocked = reader.clone();
    let (tx, rx) = mpsc::channel();
    let waiter = thread::spawn(move || tx.send(blocked.receive()).unwrap());
    reader.set_flushing(true);
    assert!(matches!(
        rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        Err(Error::Cancelled)
    ));
    waiter.join().unwrap();
    reader.set_flushing(false);
    reader.push(unit(10)).unwrap();
    assert_eq!(reader.receive().unwrap().unwrap().bytes.len(), 10);
    reader.finish(Ok(()));
    assert!(reader.receive().unwrap().is_none());
    assert!(reader.push(unit(1)).is_err());
    for oversize in [false, true] {
        let reader = PreviewReader::default();
        if oversize {
            assert!(reader.push(unit(QUEUE_BYTES + 1)).is_err());
        } else {
            for _ in 0..QUEUE_UNITS {
                reader.push(unit(1)).unwrap();
            }
            assert!(reader.push(unit(1)).is_err());
        }
        assert!(matches!(reader.receive(), Err(Error::QueueOverflow { .. })));
    }
}

#[test]
fn missing_write_approval_fails_before_network_or_identity_read() {
    let config = CameraConfig {
        host: "127.0.0.1".parse().unwrap(),
        pairing_identity: "0123456789abcdef0123456789abcdef".into(),
        allow_writes: false,
    };
    assert!(matches!(
        CameraSession::start_preview(config),
        Err(Error::InvalidConfig {
            field: "allow-writes",
            ..
        })
    ));
}

#[test]
fn cancelled_startup_releases_ownership_without_network_activity() {
    let directory = std::env::temp_dir().join(format!(
        "osmo-cancel-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = directory.join("camera-owner.lock");
    let lease = std::sync::Arc::new(CameraOwnerLease::acquire_at(&path).unwrap());
    let cancellation = PreviewCancellation::default();
    cancellation.cancel();
    let config = CameraConfig {
        host: "127.0.0.1".parse().unwrap(),
        pairing_identity: "0123456789abcdef0123456789abcdef".into(),
        allow_writes: true,
    };
    assert!(matches!(
        CameraSession::start_with_lease(config, &cancellation, Some(lease)),
        Err(Error::Cancelled)
    ));
    drop(CameraOwnerLease::acquire_at(&path).unwrap());
    assert!(cancellation.is_cancelled());
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn fec_parity_is_not_media_and_desynchronization_is_terminal() {
    let media = media_envelope(IDR_FIXTURE);
    let mut packet = video_datagram(INITIAL_SESSION, 0, &media);
    let fec = FEC_ENABLED
        | (SINGLE_PARITY << FEC_COUNT_SHIFT)
        | ((media.len() as u32) << FEC_LENGTH_SHIFT);
    packet[FEC_OFFSET..FEC_OFFSET + 4].copy_from_slice(&fec.to_le_bytes());
    let mut preview = PreviewReceiver::new(INITIAL_SESSION);
    assert_eq!(preview.feed(&packet, Instant::now()).unwrap().len(), 1);
    let mut parity = video_datagram(INITIAL_SESSION, SEQUENCE_STEP, &[0; WORD]);
    let fec = FEC_ENABLED | (SINGLE_PARITY << FEC_COUNT_SHIFT) | (SINGLE_PARITY << FEC_INDEX_SHIFT);
    parity[FEC_OFFSET..FEC_OFFSET + 4].copy_from_slice(&fec.to_le_bytes());
    assert!(preview.feed(&parity, Instant::now()).unwrap().is_empty());
    preview.complete().unwrap();
    let mut corrupt = media_envelope(P_FIXTURE);
    corrupt[ENVELOPE_CHECKSUM] ^= 1;
    assert!(
        preview
            .feed(
                &video_datagram(INITIAL_SESSION, SEQUENCE_STEP * 2, &corrupt),
                Instant::now()
            )
            .is_err()
    );
    assert!(preview.feed(&parity, Instant::now()).is_err());
}

#[test]
fn header_only_envelopes_do_not_advance_the_picture_clock() {
    let idr = nals(IDR_FIXTURE)
        .into_iter()
        .find(|nal| nal.bytes[0] & NAL_TYPE_MASK == AvcNal::Idr as u8)
        .unwrap()
        .start;
    let mut preview = PreviewReceiver::new(INITIAL_SESSION);
    for (index, payload, expected_picture) in [
        (0, &IDR_FIXTURE[..idr], None),
        (1, &IDR_FIXTURE[idr..], Some(1)),
        (2, &IDR_FIXTURE[..idr], None),
        (3, P_FIXTURE, Some(2)),
    ] {
        let units = preview
            .feed(
                &video_datagram(
                    INITIAL_SESSION,
                    index * SEQUENCE_STEP,
                    &media_envelope(payload),
                ),
                Instant::now(),
            )
            .unwrap();
        assert_eq!(
            units.first().map(|unit| unit.frame_number),
            expected_picture
        );
    }
}
