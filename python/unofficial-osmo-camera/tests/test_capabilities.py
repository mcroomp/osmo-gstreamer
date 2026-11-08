import unittest
from collections import Counter
from unittest.mock import MagicMock, patch

import osmo_camera
from osmo_camera.capabilities import (
    Availability,
    Capabilities,
    CapabilityCatalog,
    CaptureMode,
    FirmwareScope,
    resolve_capabilities,
)
from osmo_camera.enumerate_capabilities import _EnumerationTimeout, collect_capabilities
from osmo_camera.protocol import typed
from test_protocol import notification_message


def notification(topic: typed.CapabilityTopic, rows: bytes, availability: int = 1):
    return typed.CapabilityNotification.from_raw(
        notification_message(topic.name, rows, availability)
    )


class CapabilityViewTests(unittest.TestCase):
    def setUp(self) -> None:
        self.catalog = CapabilityCatalog.nano()

    def test_published_tables_and_modes_come_from_rust(self) -> None:
        formats = self.catalog.recording_formats
        self.assertEqual(
            Counter(row.mode.name for row in formats),
            {
                "Video": 35,
                "SlowMotion": 4,
                "SuperNight": 9,
                "Timelapse": 6,
                "Hyperlapse": 6,
            },
        )
        self.assertEqual(
            {row.resolution.code for row in formats},
            {0x10, 0x67, 0x2D, 0x5F, 0x0A, 0x0C},
        )
        self.assertEqual(
            {row.code for row in self.catalog.modes}, {1, 0, 0x28, 2, 0x0A}
        )
        self.assertEqual(
            {row.ratio for row in self.catalog.recording_aspects}, {(16, 9), (4, 3)}
        )
        self.assertTrue(
            all(not hasattr(row, "code") for row in self.catalog.recording_aspects)
        )
        self.assertEqual(
            {row.dimensions for row in self.catalog.preview_observations},
            {(1280, 720), (960, 720)},
        )
        self.assertIn("https://www.dji.com/nano/specs", self.catalog.sources)
        self.assertTrue(
            any(
                "2b1b8a1f94f59578f5092d562fa937fafd0d3a74" in source
                for source in self.catalog.sources
            )
        )
        for code in range(256):
            self.assertEqual(CaptureMode(code).code, code)
        self.assertEqual(CaptureMode(1), CaptureMode.VIDEO)

    def test_no_implicit_model_and_no_static_success(self) -> None:
        empty = resolve_capabilities()
        self.assertIsNone(empty.model)
        self.assertFalse(empty.complete)
        self.assertEqual(len(empty.missing), 3)
        self.assertEqual(empty.recording_formats.published, ())
        view = resolve_capabilities(catalog=self.catalog)
        self.assertEqual(view.model, "Osmo Nano")
        self.assertIsNone(view.mode)
        self.assertEqual(len(view.recording_formats.published), 60)
        self.assertIs(view.recording_formats.state, Availability.MISSING)
        self.assertIsNone(view.recording_formats.current)
        self.assertEqual(
            {row.factor for row in view.slow_motion_ratios.published}, {4, 8}
        )
        self.assertIsNone(view.slow_motion_ratios.current)
        self.assertTrue(view.preview_observations)

    def test_partial_live_precedence_preserves_raw_and_opaque_values(self) -> None:
        video = notification(typed.CapabilityTopic.VIDEO_FORMATS, b"\x01\x10\x01\xa5")
        aspect = notification(
            typed.CapabilityTopic.CAPTURE_ASPECTS, b"\x03\x01\x02\x03"
        )
        view = resolve_capabilities(
            (video, aspect), catalog=self.catalog, mode=CaptureMode.VIDEO
        )
        self.assertEqual(len(view.recording_formats.published), 35)
        live = view.recording_formats.current
        assert live is not None
        self.assertEqual(len(live), 1)
        self.assertEqual(live[0].opaque, 0xA5)
        self.assertIs(view.recording_formats.notification, video)
        self.assertEqual(view.capture_aspects.current, (1, 2, 3))
        self.assertEqual(len(view.capture_aspects.published), 2)
        self.assertIs(view.slow_motion_ratios.state, Availability.MISSING)
        self.assertIsNone(view.slow_motion_ratios.current)
        self.assertEqual(len(view.slow_motion_ratios.published), 2)
        self.assertEqual(view.missing, (typed.CapabilityTopic.SLOW_MOTION_RATIOS,))
        self.assertFalse(view.complete)
        assert video.raw is not None
        self.assertEqual(video.to_raw().encode(), video.raw.encode())

    def test_empty_and_unavailable_never_refill_from_catalog(self) -> None:
        view = resolve_capabilities(
            (
                notification(typed.CapabilityTopic.VIDEO_FORMATS, b"\x00"),
                notification(typed.CapabilityTopic.SLOW_MOTION_RATIOS, b"", 0),
                notification(typed.CapabilityTopic.CAPTURE_ASPECTS, b"\x00"),
            ),
            catalog=self.catalog,
            mode=CaptureMode.VIDEO,
        )
        self.assertTrue(view.complete)
        self.assertEqual(view.missing, ())
        self.assertIs(view.recording_formats.state, Availability.AVAILABLE)
        self.assertEqual(view.recording_formats.current, ())
        self.assertEqual(len(view.recording_formats.published), 35)
        self.assertIs(view.slow_motion_ratios.state, Availability.UNAVAILABLE)
        self.assertEqual(view.slow_motion_ratios.current, ())
        self.assertEqual(len(view.slow_motion_ratios.published), 2)

    def test_unknown_codes_and_availability_are_explicit(self) -> None:
        view = resolve_capabilities(
            (notification(typed.CapabilityTopic.VIDEO_FORMATS, b"\x01\xfe\xfd\xa5"),),
            catalog=self.catalog,
        )
        rows = view.recording_formats.current
        assert rows is not None
        self.assertEqual(rows[0].resolution.code, 0xFE)
        self.assertEqual(rows[0].frame_rate.code, 0xFD)
        unknown = notification(typed.CapabilityTopic.VIDEO_FORMATS, b"\x00", 9)
        view = resolve_capabilities((unknown,), catalog=self.catalog)
        self.assertIs(view.recording_formats.state, Availability.UNKNOWN)
        self.assertIsNone(view.recording_formats.current)
        self.assertIsInstance(
            view.recording_formats.error, osmo_camera.UnsupportedError
        )
        self.assertEqual(unknown.table().availability, 9)
        self.assertIn(typed.CapabilityTopic.VIDEO_FORMATS, view.unresolved)
        self.assertFalse(view.complete)

    def test_malformed_rows_and_ambiguous_duplicates_raise(self) -> None:
        malformed = notification(typed.CapabilityTopic.VIDEO_FORMATS, b"\x01")
        with self.assertRaises(osmo_camera.ProtocolError):
            resolve_capabilities((malformed,), catalog=self.catalog)
        video = notification(typed.CapabilityTopic.VIDEO_FORMATS, b"\x00")
        with self.assertRaises(osmo_camera.ConfigurationError):
            resolve_capabilities((video, video), catalog=self.catalog)
        other = typed.CapabilityNotification.from_raw(
            notification_message(b"synthetic_unknown_topic", b"\xff")
        )
        view = resolve_capabilities((other,), catalog=self.catalog)
        self.assertEqual(view.other_notifications, (other,))

    def test_unknown_mode_does_not_inherit_another_modes_candidates(self) -> None:
        view = resolve_capabilities(catalog=self.catalog, mode=CaptureMode(0xFE))
        assert view.mode is not None
        self.assertEqual(view.mode.code, 0xFE)
        self.assertEqual(view.recording_formats.published, ())
        self.assertEqual(len(self.catalog.recording_formats), 60)

    def test_command_scope_and_evidence_are_not_permission(self) -> None:
        view = resolve_capabilities(catalog=self.catalog)
        targeted = [
            row for row in view.command_coverage if row.contract.firmware is not None
        ]
        self.assertTrue(targeted)
        self.assertTrue(
            all(row.firmware_scope is FirmwareScope.UNKNOWN for row in targeted)
        )
        for firmware, expected in (
            ("10.00.31.22", FirmwareScope.MATCHES),
            ("future", FirmwareScope.DIFFERENT),
        ):
            view = resolve_capabilities(catalog=self.catalog, firmware=firmware)
            self.assertTrue(
                all(
                    row.firmware_scope is expected
                    for row in view.command_coverage
                    if row.contract.firmware is not None
                )
            )
            self.assertTrue(
                all(
                    row.firmware_scope is FirmwareScope.UNSPECIFIED
                    for row in view.command_coverage
                    if row.contract.firmware is None
                )
            )
        country = next(
            row
            for row in self.catalog.command_contracts
            if row.receiver == 7 and row.command == (7, 0x19)
        )
        self.assertEqual(country.request_payload, b"")
        self.assertIsNone(country.parameter)
        self.assertTrue(country.can_read)
        self.assertFalse(country.can_write)
        self.assertEqual(country.evidence, "CompatibilityContract")
        self.assertIn("NANO_10_00_31_22", country.source)
        with self.assertRaises(osmo_camera.ConfigurationError):
            typed.CapabilitySubscribe(
                typed.CapabilityTopic.VIDEO_FORMATS, 1
            ).to_raw().validate()

    def test_invalid_context_is_rejected_before_collection(self) -> None:
        session = MagicMock()
        with patch("osmo_camera.enumerate_capabilities._collect_snapshots") as collect:
            for firmware in ("", " "):
                with self.assertRaises(osmo_camera.ConfigurationError):
                    collect_capabilities(session, firmware=firmware)
            with self.assertRaises(osmo_camera.ConfigurationError):
                collect_capabilities(session, mode=MagicMock())
            with self.assertRaises(osmo_camera.ConfigurationError):
                collect_capabilities(session, catalog=MagicMock())
            collect.assert_not_called()

    def test_collection_keeps_unknown_availability_explicit(self) -> None:
        session = MagicMock()
        session.receive.side_effect = [
            notification_message(typed.CapabilityTopic.VIDEO_FORMATS.name, b"\x00", 9),
            notification_message(
                typed.CapabilityTopic.SLOW_MOTION_RATIOS.name, b"\x00"
            ),
            notification_message(typed.CapabilityTopic.CAPTURE_ASPECTS.name, b"\x00"),
        ]
        view = collect_capabilities(
            session,
            catalog=self.catalog,
            approve_subscriptions=True,
            approve_acknowledgments=True,
        )
        self.assertIs(view.recording_formats.state, Availability.UNKNOWN)
        self.assertIsNone(view.recording_formats.current)
        self.assertFalse(view.complete)
        self.assertEqual(view.missing, ())
        self.assertEqual(view.unresolved, (typed.CapabilityTopic.VIDEO_FORMATS,))
        self.assertEqual(session.acknowledge_capability_notification.call_count, 3)

    def test_collection_returns_one_view_and_propagates_real_failures(self) -> None:
        video = notification(typed.CapabilityTopic.VIDEO_FORMATS, b"\x00")
        session = MagicMock()
        with patch(
            "osmo_camera.enumerate_capabilities._collect_snapshots",
            side_effect=_EnumerationTimeout(
                (video,),
                (
                    typed.CapabilityTopic.SLOW_MOTION_RATIOS,
                    typed.CapabilityTopic.CAPTURE_ASPECTS,
                ),
            ),
        ) as collect:
            view = collect_capabilities(
                session,
                catalog=self.catalog,
                approve_subscriptions=True,
                approve_acknowledgments=True,
            )
        self.assertIsInstance(view, Capabilities)
        self.assertFalse(view.complete)
        self.assertEqual(view.recording_formats.current, ())
        self.assertIsNone(view.slow_motion_ratios.current)
        self.assertTrue(collect.call_args.kwargs["approve_subscriptions"])
        for error in (
            osmo_camera.CameraTimeoutError,
            osmo_camera.CameraRejectedError,
            osmo_camera.CancelledError,
        ):
            with patch(
                "osmo_camera.enumerate_capabilities._collect_snapshots",
                side_effect=error("synthetic"),
            ):
                with self.assertRaises(error):
                    collect_capabilities(session, catalog=self.catalog)


if __name__ == "__main__":
    unittest.main()
