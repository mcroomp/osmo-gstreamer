import contextlib
import io
import unittest
from unittest.mock import create_autospec, patch

import osmo_camera
from osmo_camera.enumerate_capabilities import (
    _collect_snapshots as collect_capabilities,
)
from osmo_camera.enumerate_capabilities import (
    _EnumerationTimeout as CapabilityEnumerationTimeout,
)
from osmo_camera.enumerate_capabilities import (
    main,
)
from osmo_camera.protocol.raw import Message
from osmo_camera.protocol.typed import CapabilityTopic
from test_protocol import notification_message


class CapabilityEnumerationTests(unittest.TestCase):
    def test_requires_approvals_and_bounded_timeout(self) -> None:
        session = create_autospec(osmo_camera.ControlSession, instance=True)
        for subscribe, acknowledge in ((False, False), (True, False), (False, True)):
            with self.assertRaises(osmo_camera.ConfigurationError):
                collect_capabilities(
                    session,
                    approve_subscriptions=subscribe,
                    approve_acknowledgments=acknowledge,
                )
        for timeout in (0.9, 30.1, float("nan"), float("inf")):
            with self.assertRaises(osmo_camera.ConfigurationError):
                collect_capabilities(
                    session,
                    timeout=timeout,
                    approve_subscriptions=True,
                    approve_acknowledgments=True,
                )
        session.subscribe.assert_not_called()
        session.receive.assert_not_called()

    def test_enumerates_three_topics_and_acks_duplicates_without_fallback(self) -> None:
        session = create_autospec(osmo_camera.ControlSession, instance=True)
        video = notification_message(CapabilityTopic.VIDEO_FORMATS.name, b"\0")
        unavailable = notification_message(
            CapabilityTopic.SLOW_MOTION_RATIOS.name, b"", 0
        )
        aspects = notification_message(
            CapabilityTopic.CAPTURE_ASPECTS.name, b"\x02\x01\xfd"
        )
        unrelated = Message(1, 2, 1, 0x40, 2, 0x80, b"\0" * 4)
        session.receive.side_effect = [
            None,
            unrelated,
            video,
            video,
            unavailable,
            aspects,
        ]
        snapshots = collect_capabilities(
            session, approve_subscriptions=True, approve_acknowledgments=True
        )
        self.assertEqual(
            [snapshot.topic for snapshot in snapshots],
            [
                CapabilityTopic.VIDEO_FORMATS,
                CapabilityTopic.SLOW_MOTION_RATIOS,
                CapabilityTopic.CAPTURE_ASPECTS,
            ],
        )
        self.assertEqual(snapshots[0].video_formats(), [])
        self.assertIsNone(snapshots[1].slow_motion_ratios())
        self.assertEqual(snapshots[2].capture_aspects(), [1, 0xFD])
        self.assertEqual(session.subscribe.call_count, 3)
        self.assertEqual(session.acknowledge_capability_notification.call_count, 4)
        self.assertEqual(
            [call.args[0].subscription_id for call in session.subscribe.call_args_list],
            [1, 2, 3],
        )
        for call in session.acknowledge_capability_notification.call_args_list:
            self.assertTrue(call.kwargs["approved"])

    def test_missing_notifications_fail_with_no_static_fallback(self) -> None:
        session = create_autospec(osmo_camera.ControlSession, instance=True)
        with patch(
            "osmo_camera.enumerate_capabilities.monotonic",
            side_effect=[0, 0, 0, 0, 2],
        ):
            with self.assertRaises(osmo_camera.CameraTimeoutError):
                collect_capabilities(
                    session,
                    timeout=1,
                    approve_subscriptions=True,
                    approve_acknowledgments=True,
                )
        session.receive.assert_not_called()

    def test_subscription_rejection_stops_before_receiving(self) -> None:
        session = create_autospec(osmo_camera.ControlSession, instance=True)
        session.subscribe.return_value.require_success.side_effect = (
            osmo_camera.CameraRejectedError("synthetic subscription rejection")
        )
        with self.assertRaises(osmo_camera.CameraRejectedError):
            collect_capabilities(
                session, approve_subscriptions=True, approve_acknowledgments=True
            )
        self.assertEqual(session.subscribe.call_count, 1)
        session.receive.assert_not_called()

    def test_timeout_retains_validated_partial_snapshots(self) -> None:
        session = create_autospec(osmo_camera.ControlSession, instance=True)
        session.receive.side_effect = [
            notification_message(CapabilityTopic.VIDEO_FORMATS.name, b"\0"),
            notification_message(CapabilityTopic.CAPTURE_ASPECTS.name, b"\0"),
        ]
        with patch(
            "osmo_camera.enumerate_capabilities.monotonic",
            side_effect=[0, 0, 0, 0, 0, 0, 2],
        ):
            with self.assertRaises(CapabilityEnumerationTimeout) as caught:
                collect_capabilities(
                    session,
                    timeout=1,
                    approve_subscriptions=True,
                    approve_acknowledgments=True,
                )
        self.assertEqual(
            [snapshot.topic for snapshot in caught.exception.snapshots],
            [CapabilityTopic.VIDEO_FORMATS, CapabilityTopic.CAPTURE_ASPECTS],
        )
        self.assertEqual(
            caught.exception.missing, (CapabilityTopic.SLOW_MOTION_RATIOS,)
        )
        self.assertEqual(caught.exception.snapshots[0].video_formats(), [])

    def test_unknown_or_malformed_notifications_fail_explicitly(self) -> None:
        for message, error in (
            (
                notification_message(b"synthetic_unknown_topic", b"\0"),
                osmo_camera.UnsupportedError,
            ),
            (
                notification_message(
                    CapabilityTopic.VIDEO_FORMATS.name, b"\x02\x10\x03\0"
                ),
                osmo_camera.ProtocolError,
            ),
        ):
            session = create_autospec(osmo_camera.ControlSession, instance=True)
            session.receive.return_value = message
            with self.assertRaises(error):
                collect_capabilities(
                    session, approve_subscriptions=True, approve_acknowledgments=True
                )

    def test_command_line_approval_checks_precede_connect(self) -> None:
        with patch("osmo_camera.enumerate_capabilities.ControlSession") as session:
            with (
                contextlib.redirect_stderr(io.StringIO()),
                self.assertRaises(SystemExit) as error,
            ):
                main(["--profile", "synthetic.toml"])
            self.assertEqual(error.exception.code, 2)
            session.connect.assert_not_called()

    def test_invalid_metadata_context_precedes_connection(self) -> None:
        for arguments in (
            ["--capture-mode", "0x100"],
            ["--firmware", ""],
        ):
            with patch("osmo_camera.enumerate_capabilities.ControlSession") as session:
                with (
                    contextlib.redirect_stderr(io.StringIO()),
                    self.assertRaises(SystemExit) as error,
                ):
                    main(
                        [
                            "--profile",
                            "synthetic.toml",
                            "--approve-registration",
                            "--approve-subscriptions",
                            "--approve-acks",
                            *arguments,
                        ]
                    )
                self.assertEqual(error.exception.code, 2)
                session.connect.assert_not_called()

    def test_command_line_teardown_and_sanitized_output(self) -> None:
        snapshots = (
            notification_message(CapabilityTopic.VIDEO_FORMATS.name, b"\0"),
            notification_message(CapabilityTopic.SLOW_MOTION_RATIOS.name, b"", 0),
            notification_message(CapabilityTopic.CAPTURE_ASPECTS.name, b"\0"),
        )
        with patch("osmo_camera.enumerate_capabilities.ControlSession") as session:
            session.connect.return_value.__enter__.return_value.receive.side_effect = (
                snapshots
            )
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                result = main(
                    [
                        "--profile",
                        "synthetic.toml",
                        "--approve-registration",
                        "--approve-subscriptions",
                        "--approve-acks",
                    ]
                )
            self.assertEqual(result, 0)
            self.assertIn("0 recording-format rows", output.getvalue())
            self.assertIn("unavailable", output.getvalue())
            self.assertNotIn("synthetic.toml", output.getvalue())
            session.connect.return_value.__exit__.assert_called_once()

    def test_ble_flags_require_both_approvals_before_connect(self) -> None:
        for flag in ("--ble-first", "--approve-ble"):
            with patch("osmo_camera.enumerate_capabilities.ControlSession") as session:
                with (
                    contextlib.redirect_stderr(io.StringIO()),
                    self.assertRaises(SystemExit) as error,
                ):
                    main(
                        [
                            "--profile",
                            "synthetic.toml",
                            "--approve-registration",
                            "--approve-subscriptions",
                            "--approve-acks",
                            flag,
                        ]
                    )
                self.assertEqual(error.exception.code, 2)
                session.connect.assert_not_called()

    def test_command_line_routes_approved_ble_first_startup(self) -> None:
        with patch("osmo_camera.enumerate_capabilities.ControlSession") as session:
            owner = session.connect.return_value.__enter__.return_value
            owner.subscribe.side_effect = osmo_camera.CameraTimeoutError("synthetic")
            with (
                contextlib.redirect_stdout(io.StringIO()),
                contextlib.redirect_stderr(io.StringIO()),
            ):
                result = main(
                    [
                        "--profile",
                        "synthetic.toml",
                        "--approve-registration",
                        "--approve-subscriptions",
                        "--approve-acks",
                        "--ble-first",
                        "--approve-ble",
                    ]
                )
            self.assertEqual(result, 1)
            self.assertTrue(session.connect.call_args.kwargs["ble_first"])
            self.assertTrue(session.connect.call_args.kwargs["approve_ble"])
            session.connect.return_value.__exit__.assert_called_once()

    def test_command_line_failure_closes_owner(self) -> None:
        with patch("osmo_camera.enumerate_capabilities.ControlSession") as session:
            owner = session.connect.return_value.__enter__.return_value
            owner.subscribe.side_effect = osmo_camera.CameraTimeoutError(
                "synthetic timeout"
            )
            with contextlib.redirect_stderr(io.StringIO()):
                result = main(
                    [
                        "--profile",
                        "synthetic.toml",
                        "--approve-registration",
                        "--approve-subscriptions",
                        "--approve-acks",
                    ]
                )
            self.assertEqual(result, 1)
            session.connect.return_value.__exit__.assert_called_once()

    def test_command_line_suppressed_collection_is_not_success(self) -> None:
        with patch("osmo_camera.enumerate_capabilities.ControlSession") as session:
            owner = session.connect.return_value.__enter__.return_value
            owner.subscribe.side_effect = osmo_camera.CameraTimeoutError(
                "synthetic timeout"
            )
            session.connect.return_value.__exit__.return_value = True
            errors = io.StringIO()
            with contextlib.redirect_stderr(errors):
                result = main(
                    [
                        "--profile",
                        "synthetic.toml",
                        "--approve-registration",
                        "--approve-subscriptions",
                        "--approve-acks",
                    ]
                )
            self.assertEqual(result, 1)
            self.assertIn("before capability collection completed", errors.getvalue())
            session.connect.return_value.__exit__.assert_called_once()

    def test_command_line_partial_observations_are_not_success(self) -> None:
        with patch("osmo_camera.enumerate_capabilities.ControlSession") as session:
            owner = session.connect.return_value.__enter__.return_value
            owner.receive.side_effect = [
                notification_message(CapabilityTopic.VIDEO_FORMATS.name, b"\0"),
                notification_message(CapabilityTopic.CAPTURE_ASPECTS.name, b"\0"),
            ]
            output = io.StringIO()
            errors = io.StringIO()
            with (
                patch(
                    "osmo_camera.enumerate_capabilities.monotonic",
                    side_effect=[0, 0, 0, 0, 0, 0, 16],
                ),
                contextlib.redirect_stdout(output),
                contextlib.redirect_stderr(errors),
            ):
                result = main(
                    [
                        "--profile",
                        "synthetic.toml",
                        "--approve-registration",
                        "--approve-subscriptions",
                        "--approve-acks",
                    ]
                )
            self.assertEqual(result, 1)
            self.assertIn("enumeration incomplete", output.getvalue())
            self.assertIn("0 recording-format rows", output.getvalue())
            self.assertIn("camcap_slowmotion_ratio", errors.getvalue())
            session.connect.return_value.__exit__.assert_called_once()


if __name__ == "__main__":
    unittest.main()
