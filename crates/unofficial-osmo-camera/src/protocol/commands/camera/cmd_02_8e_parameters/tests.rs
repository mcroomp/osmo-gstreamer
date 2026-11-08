use super::*;
use crate::protocol::{
    CommandPacket, ExplorationRequest, Flags, Message, ResponseStatus,
    capabilities::{CapabilitySelector, NANO_10_00_31_22, OperationAccess, is_known_read},
    decode, encode,
    identity::{CameraCommand, Command},
};
use std::{array::from_fn, collections::HashSet, time::Duration};

fn is_known_parameter_read(payload: &[u8]) -> bool {
    is_known_read(
        Endpoint::Camera,
        Command::Camera(CameraCommand::Parameters),
        payload,
    )
}

fn parameter_access(id: u16) -> Option<OperationAccess> {
    NANO_10_00_31_22
        .lookup(
            Endpoint::Camera,
            Command::Camera(CameraCommand::Parameters),
            CapabilitySelector::Parameter(id),
        )
        .map(|entry| entry.access)
}

#[test]
fn shared_codec_bounds_do_not_change_operation_metadata_or_raw_read_approval() {
    let query = Get::<AddCustomModeValue>::default();
    let request = query.payload().unwrap();
    assert_eq!(request, [0, 1, 0x2f, 0]);
    assert!(
        NANO_10_00_31_22
            .require_query::<Get<AddCustomModeValue>>()
            .is_err()
    );
    assert!(!is_known_parameter_read(&request));
    assert_eq!(parameter_access(0x2f), Some(OperationAccess::WriteOnly));

    let setting = Set::new(LensControlValue { code: 2 });
    assert_eq!(encode(&setting).unwrap(), [2, 0x8e, 1, 1, 0x1e, 0, 1, 2]);
    assert_eq!(parameter_access(0x1e), Some(OperationAccess::ReadOnly));
}

#[test]
fn set_wrapping_preserves_fields_without_validation_or_normalization() {
    let short = ShortVideoParametersValue::new(0x1234).unwrap();
    for changed in [
        ShortVideoParametersValue {
            flag: 0,
            ..short.clone()
        },
        ShortVideoParametersValue {
            added_duration: 1,
            ..short
        },
    ] {
        assert_eq!(Set::new(changed.clone()).into_value(), changed);
    }

    let trigger = Unknown0019Value::new().unwrap();
    for index in 0..10 {
        let mut changed = trigger.clone();
        if index < 3 {
            changed.reserved_before[index] = 1;
        } else {
            changed.reserved_after[index - 3] = 1;
        }
        assert_eq!(Set::new(changed.clone()).into_value(), changed);
    }
    let mut changed = trigger;
    changed.flag = 0;
    assert_eq!(Set::new(changed.clone()).into_value(), changed);
}

#[test]
fn beauty_constructor_builds_a_complete_indexed_record() {
    let values = from_fn(|index| (index * 17) as u8);
    let canonical = BeautyEffectValue::new(values).unwrap();
    let accepted = Set::new(canonical.clone());
    assert_eq!(accepted.value().entries.map(|entry| entry.value), values);

    for (index, entry) in canonical.entries.iter().enumerate() {
        assert_eq!(entry.index, index as u16);
        assert_eq!(entry.length, 1);
    }
    assert_eq!(canonical.count, 15);
}

#[test]
fn duration_and_aspect_checks_precede_any_narrowing() {
    for low_word in [0, 300, 1200, 3600, 65535] {
        assert!(LoopDurationValue::new(low_word).is_ok());
        for high_word in [1, 0x8000, 0xffff] {
            let value = (high_word << 16) | low_word;
            assert!(LoopDurationValue::new(value).is_err());
        }
    }
    for value in [255, 0x1ff, 0xffff, u32::MAX] {
        assert_eq!(CaptureAspectTypeValue::new(value).is_ok(), value == 255);
    }
    for code in [3, 6, 12, 24] {
        assert!(
            Unknown001fValue::new(code).is_err(),
            "combined flags must not become sparse enum codes"
        );
    }
}

#[test]
fn tc_selector_and_width_must_agree_even_when_the_payload_fits_both_widths() {
    for selector in 1..=4 {
        for data in [TcData::Byte(0), TcData::Word(0)] {
            let expected = matches!(
                (selector, &data),
                (1..=3, TcData::Byte(_)) | (4, TcData::Word(_))
            );
            assert_eq!(
                Unknown0026Value::new(selector, data.clone()).is_ok(),
                expected
            );
        }
    }
}

#[test]
fn storage_stop_is_not_a_start_with_a_different_operation_byte() {
    for source in 0..=3 {
        for destination in 0..=3 {
            assert!(StorageSynchronizationValue::new(source, destination, 1).is_ok());
            assert_eq!(
                StorageSynchronizationValue::new(source, destination, 2).is_ok(),
                source == 0 && destination == 0
            );
        }
    }
    assert!(StorageSynchronizationValue::new(4, 0, 1).is_err());
    assert!(StorageSynchronizationValue::new(0, 4, 1).is_err());
}

#[test]
fn calibration_file_limit_includes_both_name_prefixes_and_terminators() {
    let first = CalibrationName::new(&"a".repeat(123)).unwrap();
    let second = CalibrationName::new(&"b".repeat(125)).unwrap();
    assert!(LensUserCalibrationFileValue::new(1, first.clone(), Some(second)).is_ok());
    let second = CalibrationName::new(&"b".repeat(126)).unwrap();
    assert!(LensUserCalibrationFileValue::new(1, first, Some(second)).is_err());

    let max = CalibrationName::new(&"a".repeat(127)).unwrap();
    assert!(LensUserCalibrationFileValue::new(0, max.clone(), None).is_ok());
    assert!(LensUserCalibrationFileValue::new(1, max.clone(), Some(max)).is_err());
    assert!(CalibrationName::new(&"a".repeat(128)).is_err());
}

#[test]
fn calibration_operation_selects_name_arity_not_just_an_optional_field() {
    let name = CalibrationName::new("synthetic").unwrap();
    for operation in 0..=3 {
        for second_name in [None, Some(name.clone())] {
            let expected = operation <= 2 && (operation == 1) == second_name.is_some();
            assert_eq!(
                LensUserCalibrationFileValue::new(operation, name.clone(), second_name.clone())
                    .is_ok(),
                expected
            );
        }
    }
}

#[test]
fn calibration_constructors_check_names_and_build_only_the_supported_action() {
    for name in ["", "a\0b", "a\nb", "\u{80}"] {
        assert!(CalibrationName::new(name).is_err());
    }
    let canonical =
        LensUserCalibrationValue::new(CalibrationName::new("synthetic").unwrap(), 65535).unwrap();
    assert_eq!(canonical.operation, 3);
    assert_eq!(canonical.metadata, 0);
    assert_eq!(canonical.extra_length, 4);
    assert_eq!(canonical.reserved, 0);
    let mut changed = canonical;
    changed.name.bytes.pop();
    assert!(LensUserCalibrationValue::new(changed.name, changed.focal_code).is_err());
}

#[test]
fn set_is_a_snapshot_not_an_alias_to_mutable_raw_values() {
    let mut raw = BeautyEffectValue::new([0; 15]).unwrap();
    let snapshot = Set::new(raw.clone());
    raw.entries[7].value = 255;
    assert_eq!(snapshot.value().entries[7].value, 0);
    assert_eq!(Set::new(raw.clone()).value().entries[7].value, 255);
    raw.entries[7].index = 8;
    assert_eq!(Set::new(raw).value().entries[7].index, 8);
    assert_eq!(snapshot.value().entries[7].index, 7);
}

#[test]
fn single_get_rejects_a_reply_for_another_id_or_a_batch() {
    let query = Get::<EisValue>::default();
    let accepted = [0, 0, 1, 8, 0, 1, 7];
    let CommandReadReply::Accepted(reply) = decode_reply(&query, &accepted).unwrap() else {
        panic!("expected accepted typed reply");
    };
    assert_eq!(reply.value.mode, CameraStabilizationMode::Unknown07);
    for (case, bytes) in [
        ("wrong ID", vec![0, 0, 1, 6, 0, 1, 2]),
        ("wrong operation", vec![0, 1, 1, 8, 0, 1, 7]),
        ("wrong count", vec![0, 0, 2, 8, 0, 1, 7]),
        ("extra record", vec![0, 0, 1, 8, 0, 1, 7, 8, 0, 1, 7]),
        ("different value width", vec![0, 0, 1, 8, 0, 2, 7, 0]),
    ] {
        assert!(decode_reply(&query, &bytes).is_err(), "{case}");
    }
}

#[test]
fn typed_get_does_not_apply_set_bounds_or_decode_rejections_as_values() {
    let query = Get::<EisValue>::default();
    let CommandReadReply::Accepted(reply) = decode_reply(&query, &[0, 0, 1, 8, 0, 1, 255]).unwrap()
    else {
        panic!("expected raw GET code");
    };
    assert_eq!(reply.value.mode, CameraStabilizationMode::Unknown(255));
    assert!(EisValue::new(reply.value.mode).is_err());
    assert_eq!(
        encode(&Set::new(reply.value)).unwrap(),
        [2, 0x8e, 1, 1, 8, 0, 1, 255]
    );
    let CommandReadReply::Rejected { status, extension } =
        decode_reply(&query, &[0xe3, 0, 0xff]).unwrap()
    else {
        panic!("expected rejection");
    };
    assert_eq!(status, ResponseStatus::Unknown(0xe3));
    assert_eq!(extension.as_slice(), [0, 0xff]);
}

#[test]
fn fixed_legacy_pair_requires_both_ids_in_the_declared_order() {
    let query = AdvancedLiveReversalGet::default();
    let accepted = [0, 0, 2, 0, 0, 1, 2, 1, 0, 2, 0xaa, 0xbb];
    let CommandReadReply::Accepted(reply) = decode_reply(&query, &accepted).unwrap() else {
        panic!("expected fixed pair");
    };
    assert_eq!(reply.advanced_mode.code, 2);
    assert_eq!(reply.legacy_value.bytes, [0xaa, 0xbb]);
    for index in [2, 3, 7] {
        let mut changed = accepted;
        changed[index] ^= 1;
        assert!(decode_reply(&query, &changed).is_err());
    }
    assert!(decode_reply(&query, &[0, 0, 2, 0, 0, 2, 2, 0, 1, 0, 0]).is_err());
}

#[test]
fn catalogue_read_eligibility_matches_raw_read_approval_without_granting_writes() {
    let mut ids = HashSet::new();
    for entry in NANO_10_00_31_22.commands {
        if entry.receiver != Endpoint::Camera
            || entry.command != Command::Camera(CameraCommand::Parameters)
        {
            continue;
        }
        let CapabilitySelector::Parameter(id) = entry.selector else {
            continue;
        };
        assert!(ids.insert(id), "duplicate catalogue ID");
        let request = ExplorationRequest {
            receiver: Endpoint::Camera,
            command: Command::Camera(CameraCommand::Parameters),
            flags: Flags::REQUEST,
            payload: [vec![0, 1], id.to_le_bytes().to_vec()].concat(),
            timeout: Duration::from_secs(1),
        };
        assert_eq!(request.is_known_read(), entry.access.can_read());
        assert_eq!(
            request.validate(false, false).is_ok(),
            entry.access.can_read()
        );
    }
    assert_eq!(ids.len(), 37);
    assert!(NANO_10_00_31_22.require_query::<Get<EisValue>>().is_ok());
    assert!(
        NANO_10_00_31_22
            .require_query::<Get<RotationStateValue>>()
            .is_ok()
    );
    assert!(
        NANO_10_00_31_22
            .require_query::<Get<QuickModeSwitchValue>>()
            .is_err()
    );
    assert!(is_known_parameter_read(
        &Get::<QuickModeSwitchValue>::default().payload().unwrap()
    ));
    assert!(!is_known_parameter_read(&[0, 1, 0xff, 0xff]));
    assert!(!is_known_parameter_read(
        &encode(&Set::new(
            EisValue::new(CameraStabilizationMode::Unknown07).unwrap()
        ))
        .unwrap()
    ));
}

#[test]
fn scalar_constructor_bounds_cover_every_byte_without_restricting_manual_values() {
    fn check<T: Parameter>(new: fn(u8) -> Result<T>, permitted: &[u8]) {
        for code in u8::MIN..=u8::MAX {
            let constructed = new(code);
            assert_eq!(
                constructed.is_ok(),
                permitted.contains(&code),
                "parameter {:04x}, code {code}",
                T::ID,
            );
            if let Ok(value) = constructed {
                assert_eq!(encode(&Set::new(value)).unwrap().last(), Some(&code));
            }
        }
    }

    check(AdvanceModeValue::new, &[0, 1, 2]);
    check(StreamQualityValue::new, &[0, 1, 2]);
    check(FieldOfViewValue::new, &[0, 1, 2, 3, 4, 5]);
    check(VoiceControlValue::new, &[0, 1]);
    check(VoiceLanguageValue::new, &[0, 1]);
    check(Unknown001fValue::new, &[1, 2, 4, 8, 16]);
    check(SelfieFollowingValue::new, &[0, 1]);
    check(Unknown0028Value::new, &[0, 1]);
    check(StabilizationPreferenceValue::new, &[0, 1]);
    check(ImageAdjustmentValue::new, &[0, 1, 2, 3]);
    check(ShutterLimitValue::new, &[0, 1, 2, 3, 4, 5]);
    check(VideoQualityEnhancementValue::new, &[0, 1]);
    check(RecordingPlanValue::new, &[0, 1, 2]);

    let all_codes: Vec<_> = (u8::MIN..=u8::MAX).collect();
    check(IsoLimitValue::new, &all_codes);
    check(RecordingAudioChannelValue::new, &all_codes);
    check(SelfieMirrorValue::new, &all_codes);
    check(TimelapseTrackSetValue::new, &all_codes);
}

#[test]
fn stabilization_parameters_share_a_lossless_enum_and_identical_constructor_bounds() {
    fn check<T: Parameter>(
        new: fn(CameraStabilizationMode) -> Result<T>,
        manual: fn(CameraStabilizationMode) -> T,
        mode_of: fn(&T) -> CameraStabilizationMode,
    ) {
        let named_modes = [
            CameraStabilizationMode::Off,
            CameraStabilizationMode::RockSteady,
            CameraStabilizationMode::HorizonSteady,
            CameraStabilizationMode::RockSteadyPlus,
            CameraStabilizationMode::HorizonBalancing,
            CameraStabilizationMode::Unknown05,
            CameraStabilizationMode::Unknown06,
            CameraStabilizationMode::Unknown07,
        ];
        let [low, high] = T::ID.to_le_bytes();
        let query = Get::<T>::default();
        assert_eq!(encode(&query).unwrap(), [2, 0x8e, 0, 1, low, high]);
        for code in u8::MIN..=u8::MAX {
            let expected = named_modes
                .get(usize::from(code))
                .copied()
                .unwrap_or(CameraStabilizationMode::Unknown(code));
            assert_eq!(
                decode::<CameraStabilizationMode>(&[code], ()).unwrap(),
                expected
            );
            assert_eq!(encode(&expected).unwrap(), [code]);
            for mode in [expected, CameraStabilizationMode::Unknown(code)] {
                let constructed = new(mode);
                assert_eq!(
                    constructed.is_ok(),
                    code <= 7,
                    "ID {:04x}, code {code}",
                    T::ID
                );
                if let Ok(value) = constructed {
                    assert_eq!(mode_of(&value), mode);
                    assert_eq!(
                        encode(&Set::new(value)).unwrap(),
                        [2, 0x8e, 1, 1, low, high, 1, code],
                    );
                }
                let setting = Set::new(manual(mode));
                assert_eq!(mode_of(setting.value()), mode);
                assert_eq!(
                    encode(&setting).unwrap(),
                    [2, 0x8e, 1, 1, low, high, 1, code],
                );
            }
            let body = [0, 0, 1, low, high, 1, code];
            let CommandReadReply::Accepted(reply) = decode_reply(&query, &body).unwrap() else {
                panic!("expected typed stabilization reply");
            };
            assert_eq!(mode_of(&reply.value), expected);
            assert_eq!(
                encode(&Set::new(reply.value)).unwrap(),
                [2, 0x8e, 1, 1, low, high, 1, code],
            );
        }
        for malformed in [
            vec![0, 0, 1, low, high, 0],
            vec![0, 0, 1, low, high, 2, 0, 0],
            vec![0, 0, 1, low, high, 1],
            vec![0, 0, 1, low, high, 1, 0, 0],
        ] {
            assert!(decode_reply(&query, &malformed).is_err());
        }
    }
    check(EisValue::new, |mode| EisValue { mode }, |value| value.mode);
    check(
        LiveViewEisValue::new,
        |mode| LiveViewEisValue { mode },
        |value| value.mode,
    );
    assert!(decode_reply(&Get::<EisValue>::default(), &[0, 0, 1, 0x3e, 0, 1, 1]).is_err());
    assert!(decode_reply(&Get::<LiveViewEisValue>::default(), &[0, 0, 1, 8, 0, 1, 1]).is_err());
}

#[test]
fn every_profile_write_has_a_constructor_with_the_exact_single_tlv_shape() {
    fn checked_id<T: Parameter>(value: Result<T>, body: &[u8]) -> u16 {
        let mut expected = vec![2, 0x8e, 1, 1];
        expected.extend_from_slice(&T::ID.to_le_bytes());
        expected.push(u8::try_from(body.len()).unwrap());
        expected.extend_from_slice(body);
        assert_eq!(encode(&Set::new(value.unwrap())).unwrap(), expected);
        T::ID
    }

    let name = CalibrationName::new("x").unwrap();
    let mut beauty = vec![15, 0];
    for index in 0..15u16 {
        beauty.extend_from_slice(&index.to_le_bytes());
        beauty.extend_from_slice(&[1, 0xa5]);
    }
    let ids = [
        checked_id(AdvanceModeValue::new(2), &[2]),
        checked_id(StreamQualityValue::new(2), &[2]),
        checked_id(EisValue::new(CameraStabilizationMode::Unknown07), &[7]),
        checked_id(FieldOfViewValue::new(5), &[5]),
        checked_id(VoiceControlValue::new(1), &[1]),
        checked_id(VoiceLanguageValue::new(1), &[1]),
        checked_id(IsoLimitValue::new(0xa5), &[0xa5]),
        checked_id(LoopDurationValue::new(1200), &[0xb0, 4, 0, 0]),
        checked_id(Unknown0019Value::new(), &[0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]),
        checked_id(Unknown001fValue::new(16), &[16]),
        checked_id(RecordingAudioChannelValue::new(0xa5), &[0xa5]),
        checked_id(SelfieFollowingValue::new(1), &[1]),
        checked_id(
            Unknown0026Value::new(4, TcData::Word(0x12345678)),
            &[1, 4, 0x78, 0x56, 0x34, 0x12],
        ),
        checked_id(Unknown0028Value::new(1), &[1]),
        checked_id(
            ShortVideoParametersValue::new(0x12345678),
            &[1, 0x78, 0x56, 0x34, 0x12, 0, 0, 0, 0],
        ),
        checked_id(
            LensUserCalibrationValue::new(name.clone(), 0x1234),
            &[3, 0, 2, b'x', 0, 4, 0, 0x34, 0x12, 0, 0],
        ),
        checked_id(
            LensUserCalibrationFileValue::new(1, name.clone(), Some(name)),
            &[1, 6, 0, 2, b'x', 0, 2, b'x', 0],
        ),
        checked_id(AddCustomModeValue::new(0xa5, 3), &[0xa5, 3]),
        checked_id(StabilizationPreferenceValue::new(1), &[1]),
        checked_id(StorageSynchronizationValue::new(1, 2, 1), &[1, 2, 1]),
        checked_id(ImageAdjustmentValue::new(3), &[3]),
        checked_id(ShutterLimitValue::new(5), &[5]),
        checked_id(VideoQualityEnhancementValue::new(1), &[1]),
        checked_id(TimelapseTrackPreviewValue::new(0xa5, 1), &[0xa5, 1]),
        checked_id(TimelapseTrackSetValue::new(0xa5), &[0xa5]),
        checked_id(SelfieMirrorValue::new(0xa5), &[0xa5]),
        checked_id(BeautyEffectValue::new([0xa5; 15]), &beauty),
        checked_id(AutofocusSensitivityValue::new(0xa5), &[1, 0xa5]),
        checked_id(AutofocusStrategyValue::new(0xa5), &[1, 0xa5]),
        checked_id(Unknown003cValue::new(300), &[0x2c, 1]),
        checked_id(RecordingPlanValue::new(2), &[2]),
        checked_id(
            LiveViewEisValue::new(CameraStabilizationMode::Unknown07),
            &[7],
        ),
        checked_id(CaptureAspectTypeValue::new(255), &[255, 0, 0, 0]),
    ];
    let checked: HashSet<_> = ids.into_iter().collect();
    let writes: HashSet<_> = NANO_10_00_31_22
        .commands
        .iter()
        .filter(|entry| {
            entry.receiver == Endpoint::Camera
                && entry.command == Command::Camera(CameraCommand::Parameters)
        })
        .filter_map(|entry| match entry.selector {
            CapabilitySelector::Parameter(id) if entry.access.can_write() => Some(id),
            _ => None,
        })
        .collect();
    assert_eq!(checked.len(), 33);
    assert_eq!(checked, writes);
    for selector in 1..=3 {
        checked_id(
            Unknown0026Value::new(selector, TcData::Byte(0xa5)),
            &[1, selector, 0xa5],
        );
    }
    checked_id(StorageSynchronizationValue::new(0, 0, 2), &[0, 0, 2]);
}

fn decode_reply<Q: Query>(query: &Q, bytes: &[u8]) -> Result<Q::Response> {
    let mut message = Message::request(Endpoint::App, 1, query.command(), bytes);
    message.flags = Flags::RESPONSE;
    query.decode_reply(&message)
}
