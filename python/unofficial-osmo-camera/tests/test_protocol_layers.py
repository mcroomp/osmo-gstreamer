import tempfile
import unittest
from pathlib import Path

import osmo_camera
from osmo_camera import protocol
from osmo_camera.protocol import raw, typed
from test_protocol import notification_body, notification_message


class ProtocolLayerTests(unittest.TestCase):
    def test_namespaces_have_one_explicit_home_without_legacy_aliases(self) -> None:
        self.assertEqual(protocol.__all__, ["raw", "typed"])
        self.assertFalse(hasattr(protocol, "Message"))
        self.assertFalse(hasattr(protocol, "CapabilitySubscribe"))
        self.assertFalse(hasattr(typed.CapabilityNotification, "from_message"))
        self.assertFalse(hasattr(raw, "CapabilitySubscribe"))
        self.assertFalse(hasattr(typed, "Message"))
        self.assertTrue(callable(osmo_camera.ControlSession.request_raw))

    def test_ble_first_requires_separate_approval_before_profile_access(self) -> None:
        for ble_first, approve_ble in ((True, False), (False, True)):
            with self.assertRaises(osmo_camera.ConfigurationError):
                osmo_camera.ControlSession.connect(
                    "missing-synthetic-profile.toml",
                    approved=True,
                    ble_first=ble_first,
                    approve_ble=approve_ble,
                )
        cancellation = osmo_camera.Cancellation()
        cancellation.cancel()
        with self.assertRaises(osmo_camera.CancelledError):
            osmo_camera.ControlSession.connect(
                "missing-synthetic-profile.toml",
                approved=True,
                ble_first=True,
                approve_ble=True,
                cancellation=cancellation,
            )
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "synthetic.toml"
            path.write_text("version = 1\n", encoding="utf-8")
            with self.assertRaises(osmo_camera.ConfigurationError):
                osmo_camera.ControlSession.connect(
                    path, approved=True, ble_first=True, approve_ble=True
                )

    def test_typed_subscription_becomes_canonical_raw_request(self) -> None:
        subscription = typed.CapabilitySubscribe(typed.CapabilityTopic.VIDEO_FORMATS, 7)
        request = subscription.to_raw(timeout=0.1)
        self.assertIsInstance(request, raw.Request)
        self.assertEqual(request.receiver, 0x28)
        self.assertEqual(request.command, subscription.command)
        self.assertEqual(request.flags, 0x40)
        self.assertEqual(request.timeout, 0.1)
        self.assertEqual(request.payload, subscription.payload())
        self.assertFalse(request.is_known_read)
        frame = request.frame(0x1234)
        self.assertEqual(frame.encode(), subscription.frame(0x1234).encode())
        context = raw.MessageContext(2, request.receiver, 0x1234, request.flags)
        self.assertEqual(context.frame(subscription.encode()).encode(), frame.encode())
        for session, operation in ((False, False), (True, False), (False, True)):
            with self.assertRaises(osmo_camera.ConfigurationError):
                request.validate(
                    allow_unclassified=session, approve_unclassified=operation
                )
        request.validate(allow_unclassified=True, approve_unclassified=True)

    def test_raw_read_permission_matches_the_entire_native_tuple(self) -> None:
        request = raw.Request(7, 7, 0x19, b"")
        self.assertTrue(request.is_known_read)
        request.validate()
        for altered in (
            raw.Request(7, 7, 0x19, b"\x01"),
            raw.Request(1, 7, 0x19, b""),
            raw.Request(7, 7, 0x19, b"", flags=0x80),
            raw.Request(7, 7, 0x19, b"", flags=0x41),
        ):
            self.assertFalse(altered.is_known_read)
            with self.assertRaises(osmo_camera.ConfigurationError):
                altered.validate()
            altered.validate(allow_unclassified=True, approve_unclassified=True)

    def test_raw_bounds_and_media_restrictions_are_not_bypassed(self) -> None:
        for timeout in (0, 0.099, 30.001, float("nan"), float("inf")):
            with self.assertRaises(osmo_camera.ConfigurationError):
                raw.Request(7, 7, 0x19, b"", timeout=timeout)
        raw.Request(7, 7, 0x19, b"", timeout=30).validate()
        for command_set, command_id in ((2, 0x8E), (8, 0x78), (8, 0x79)):
            with self.assertRaises(osmo_camera.UnsupportedError):
                raw.Request(8, command_set, command_id, b"").validate(
                    allow_unclassified=True, approve_unclassified=True
                )
        with self.assertRaises(osmo_camera.ConfigurationError):
            raw.Request(7, 7, 0x19, b"\x00" * 1023).validate(
                allow_unclassified=True, approve_unclassified=True
            )

    def test_unknown_raw_identities_and_flags_remain_lossless(self) -> None:
        request = raw.Request(0xFC, 0xFA, 0xFE, b"\xfd\x00", flags=0x33)
        frame = request.frame(0xFFFF)
        decoded = raw.Message.decode(frame.encode())
        self.assertEqual(decoded.encode(), frame.encode())
        self.assertEqual(decoded.command, (0xFA, 0xFE))
        self.assertEqual(decoded.receiver, 0xFC)
        self.assertEqual(decoded.flags, 0x33)
        context = raw.MessageContext(0xFD, 0xFC, 0xFFFF, 0x33)
        framed = context.frame(b"\xfa\xfe\xfd\x00")
        self.assertEqual(raw.Message.decode(framed.encode()).encode(), framed.encode())
        self.assertEqual(
            (context.sender, context.receiver, context.sequence, context.flags),
            (0xFD, 0xFC, 0xFFFF, 0x33),
        )
        with self.assertRaises(osmo_camera.ProtocolError):
            context.frame(b"\xfa")

    def test_typed_notifications_preserve_raw_frames_and_unknown_values(self) -> None:
        for name, rows in (
            (typed.CapabilityTopic.VIDEO_FORMATS.name, b"\x01\xfe\xfd\xa5"),
            (b"synthetic_unknown_topic", b"\x00\xff"),
        ):
            with self.subTest(name=name):
                frame = notification_message(name, rows)
                notification = typed.CapabilityNotification.from_raw(frame)
                self.assertIsNotNone(notification.raw)
                self.assertEqual(notification.to_raw().encode(), frame.encode())
                retained = notification.raw
                assert retained is not None
                self.assertEqual(retained.encode(), frame.encode())
                self.assertEqual(retained.sequence, 0x9876)
                self.assertEqual(notification.sequence, 0x1234)
                if notification.topic is not None:
                    formats = notification.video_formats()
                    assert formats is not None
                    self.assertEqual(formats[0].resolution.code, 0xFE)
                    self.assertEqual(formats[0].frame_rate.code, 0xFD)
                    self.assertEqual(formats[0].opaque, 0xA5)
                else:
                    self.assertEqual(notification.name, name)

    def test_body_only_notification_requires_explicit_framing_context(self) -> None:
        name = typed.CapabilityTopic.CAPTURE_ASPECTS.name
        body = notification_body(name, b"\x02\x01\xfe")
        notification = typed.CapabilityNotification.decode(body)
        self.assertIsNone(notification.raw)
        with self.assertRaises(osmo_camera.ConfigurationError):
            notification.to_raw()
        context = raw.MessageContext(0x28, 2, 0x9876, 0x40)
        expected = notification_message(name, b"\x02\x01\xfe")
        self.assertEqual(notification.to_raw(context).encode(), expected.encode())
        original = typed.CapabilityNotification.from_raw(expected)
        reframed = original.to_raw(raw.MessageContext(0x28, 2, 4, 0x40))
        self.assertEqual(reframed.sequence, 4)
        self.assertEqual(reframed.payload, expected.payload)
        self.assertEqual(original.to_raw().encode(), expected.encode())

    def test_typed_reply_preserves_all_statuses_and_original_transport_fields(
        self,
    ) -> None:
        subscription = typed.CapabilitySubscribe(typed.CapabilityTopic.VIDEO_FORMATS, 1)
        for status in range(256):
            frame = raw.Message(0x28, 2, 0xFEDC, 0x80, 0, 0x99, bytes([status, 0xA5]))
            reply = subscription.decode_reply(raw.Message.decode(frame.encode()))
            self.assertEqual(reply.status, status)
            self.assertEqual(reply.extension, b"\xa5")
            self.assertEqual(reply.raw.encode(), frame.encode())
            self.assertEqual(reply.to_raw().encode(), frame.encode())

    def test_raw_notification_bridge_enforces_typed_envelope_validation(self) -> None:
        frame = notification_message(
            typed.CapabilityTopic.CAPTURE_ASPECTS.name, b"\x00"
        )
        for sender, receiver, flags in ((1, 2, 0x40), (0x28, 1, 0x40), (0x28, 2, 0x80)):
            invalid = raw.Message(sender, receiver, 1, flags, 0, 0x99, frame.payload)
            with self.assertRaises(osmo_camera.CameraError):
                typed.CapabilityNotification.from_raw(invalid)


if __name__ == "__main__":
    unittest.main()
