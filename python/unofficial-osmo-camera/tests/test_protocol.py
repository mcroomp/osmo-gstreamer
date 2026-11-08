import ast
import builtins
import struct
import tempfile
import unittest
from pathlib import Path

import osmo_camera
from osmo_camera import _native
from osmo_camera.protocol.raw import Message
from osmo_camera.protocol.typed import (
    CapabilityNotification,
    CapabilitySubscribe,
    CapabilityTable,
    CapabilityTopic,
    VideoFrameRateCode,
    VideoResolutionCode,
)


def notification_body(name: bytes, records: bytes, availability: int = 1) -> bytes:
    data = struct.pack("<BH", availability, len(records)) + records
    return (
        b"\x00\x99\x02\x06\x00\x00"
        + struct.pack("<H", 0x1234)
        + b"\xa5" * 5
        + struct.pack("<HH", 0x4321, len(name))
        + name
        + b"\x5a" * 6
        + struct.pack("<H", len(data))
        + data
    )


def notification_message(name: bytes, records: bytes, availability: int = 1) -> Message:
    body = notification_body(name, records, availability)
    return Message(0x28, 0x02, 0x9876, 0x40, 0x00, 0x99, body[2:])


class ProtocolBindingTests(unittest.TestCase):
    def test_subscription_identity_lengths_and_approval(self) -> None:
        for topic in (
            CapabilityTopic.VIDEO_FORMATS,
            CapabilityTopic.SLOW_MOTION_RATIOS,
            CapabilityTopic.CAPTURE_ASPECTS,
        ):
            with self.subTest(topic=topic.name):
                request = CapabilitySubscribe(topic, 0x12345678)
                expected = (
                    b"\x02\x02\x00\x00\x78\x56\x34\x12\x00\x00\x00"
                    + struct.pack("<HH", len(topic.name) + 6, len(topic.name))
                    + topic.name
                    + b"\x00" * 4
                )
                self.assertEqual(request.payload(), expected)
                self.assertEqual(request.encode(), b"\x00\x99" + expected)
                self.assertEqual(request.command, (0, 0x99))
                self.assertEqual(request.subscription_id, 0x12345678)
                self.assertEqual(request.topic, topic)
                frame = request.frame(0x1234)
                self.assertEqual((frame.sender, frame.receiver), (2, 0x28))
                self.assertEqual(frame.payload, expected)
                self.assertEqual(
                    Message.decode(frame.encode()).encode(), frame.encode()
                )
                for session, operation in (
                    (False, False),
                    (True, False),
                    (False, True),
                ):
                    with self.assertRaises(osmo_camera.ConfigurationError):
                        request.validate(
                            allow_unclassified=session, approve_unclassified=operation
                        )
                request.validate(allow_unclassified=True, approve_unclassified=True)
                for timeout in (0, 0.099, 30.001, float("nan"), float("inf")):
                    with self.assertRaises(osmo_camera.ConfigurationError):
                        request.validate(
                            allow_unclassified=True,
                            approve_unclassified=True,
                            timeout=timeout,
                        )

    def test_subscription_rejection_and_extensions(self) -> None:
        request = CapabilitySubscribe(CapabilityTopic.VIDEO_FORMATS, 1)
        for status in range(256):
            message = Message(0x28, 2, 1, 0x80, 0, 0x99, bytes([status, 0xA5]))
            reply = request.decode_reply(message)
            self.assertEqual(reply.accepted, status == 0)
            self.assertEqual(reply.status, status)
            self.assertEqual(reply.extension, b"\xa5")
            if status == 0:
                reply.require_success()
            else:
                with self.assertRaises(osmo_camera.CameraRejectedError):
                    reply.require_success()
        for sender, receiver, flags, command in (
            (1, 2, 0x80, 0x99),
            (0x28, 1, 0x80, 0x99),
            (0x28, 2, 0x40, 0x99),
            (0x28, 2, 0x80, 1),
        ):
            with self.assertRaises(osmo_camera.ProtocolError):
                request.decode_reply(
                    Message(sender, receiver, 1, flags, 0, command, b"\0")
                )
        with self.assertRaises(osmo_camera.ProtocolError):
            request.decode_reply(Message(0x28, 2, 1, 0x80, 0, 0x99, b""))

    def test_video_rows_preserve_unknown_codes_and_opaque_fields(self) -> None:
        body = notification_body(
            CapabilityTopic.VIDEO_FORMATS.name,
            bytes([2, 0x10, 3, 0xA5, 0xFC, 0xFD, 0x5A]),
        )
        notification = CapabilityNotification.decode(body)
        self.assertEqual(notification.encode(), body)
        self.assertEqual(notification.sequence, 0x1234)
        self.assertEqual(notification.remaining_length, 0x4321)
        self.assertEqual(notification.opaque_prefix, b"\xa5" * 5)
        self.assertEqual(notification.opaque_before_data, b"\x5a" * 6)
        rows = notification.video_formats()
        self.assertIsNotNone(rows)
        assert rows is not None
        self.assertEqual(len(rows), 2)
        self.assertEqual(rows[0].resolution.dimensions, (3840, 2160))
        self.assertEqual(rows[0].frame_rate.name, "Fps30000Over1001")
        self.assertEqual(rows[0].opaque, 0xA5)
        self.assertEqual(rows[1].resolution.code, 0xFC)
        self.assertIsNone(rows[1].resolution.dimensions)
        self.assertEqual(rows[1].frame_rate.code, 0xFD)
        self.assertEqual(rows[1].opaque, 0x5A)
        with self.assertRaises(osmo_camera.ProtocolError):
            notification.capture_aspects()

    def test_slow_motion_and_aspect_rows(self) -> None:
        slow = CapabilityNotification.decode(
            notification_body(
                CapabilityTopic.SLOW_MOTION_RATIOS.name, bytes([2, 3, 4, 0xFD, 8])
            )
        ).slow_motion_ratios()
        self.assertIsNotNone(slow)
        assert slow is not None
        self.assertEqual(
            [(row.frame_rate.code, row.factor) for row in slow], [(3, 4), (0xFD, 8)]
        )
        aspects = CapabilityNotification.decode(
            notification_body(
                CapabilityTopic.CAPTURE_ASPECTS.name, bytes([3, 1, 2, 0xFD])
            )
        ).capture_aspects()
        self.assertEqual(aspects, [1, 2, 0xFD])

    def test_empty_unavailable_and_unknown_are_distinct(self) -> None:
        empty = CapabilityNotification.decode(
            notification_body(CapabilityTopic.VIDEO_FORMATS.name, b"\0")
        )
        self.assertEqual(empty.video_formats(), [])
        self.assertTrue(empty.table().available)
        unavailable = CapabilityNotification.decode(
            notification_body(CapabilityTopic.VIDEO_FORMATS.name, b"\xa5", 0)
        )
        self.assertIsNone(unavailable.video_formats())
        self.assertFalse(unavailable.table().available)
        unknown = CapabilityNotification.decode(
            notification_body(CapabilityTopic.VIDEO_FORMATS.name, b"\xa5", 9)
        )
        self.assertEqual(unknown.table().availability, 9)
        self.assertEqual(unknown.table().data, b"\xa5")
        self.assertEqual(CapabilityTable.decode(unknown.data).encode(), unknown.data)
        with self.assertRaises(osmo_camera.UnsupportedError):
            unknown.video_formats()
        with self.assertRaises(osmo_camera.UnsupportedError):
            _ = unknown.table().available

    def test_notification_boundaries_and_row_counts(self) -> None:
        body = notification_body(
            CapabilityTopic.VIDEO_FORMATS.name, b"\x01\x10\x03\x00"
        )
        for end in range(len(body)):
            with self.assertRaises(osmo_camera.ProtocolError):
                CapabilityNotification.decode(body[:end])
        with self.assertRaises(osmo_camera.ProtocolError):
            CapabilityNotification.decode(body + b"\0")
        for records in (b"\x02\x10\x03\0", b"\x01\x10\x03\0\0", b""):
            notification = CapabilityNotification.decode(
                notification_body(CapabilityTopic.VIDEO_FORMATS.name, records)
            )
            with self.assertRaises(osmo_camera.ProtocolError):
                notification.video_formats()
        for data in (b"", b"\x01\x01", b"\x01\x02\0\0", b"\x01\0\0\0"):
            with self.assertRaises(osmo_camera.ProtocolError):
                CapabilityTable.decode(data)

    def test_checked_notification_and_acknowledgment(self) -> None:
        message = notification_message(CapabilityTopic.VIDEO_FORMATS.name, b"\0")
        notification = CapabilityNotification.from_raw(message)
        self.assertEqual(notification.topic, CapabilityTopic.VIDEO_FORMATS)
        ack = CapabilityNotification.acknowledgment(message)
        self.assertEqual((ack.sender, ack.receiver), (2, 0x28))
        self.assertEqual(ack.sequence, message.sequence)
        self.assertEqual(ack.flags, 0xC0)
        self.assertEqual(ack.payload, b"\0")
        self.assertEqual(ack.command, message.command)
        for sender, receiver, flags, command in (
            (1, 2, 0x40, 0x99),
            (0x28, 1, 0x40, 0x99),
            (0x28, 2, 0x80, 0x99),
            (0x28, 2, 0x40, 1),
        ):
            invalid = Message(sender, receiver, 1, flags, 0, command, message.payload)
            with self.assertRaises(osmo_camera.ProtocolError):
                CapabilityNotification.from_raw(invalid)
        unknown = notification_message(b"synthetic_unknown_topic", b"\xa5")
        self.assertIsNone(CapabilityNotification.from_raw(unknown).topic)
        with self.assertRaises(osmo_camera.UnsupportedError):
            CapabilityNotification.acknowledgment(unknown)

    def test_all_byte_codes_round_trip(self) -> None:
        for code in range(256):
            self.assertEqual(VideoResolutionCode(code).code, code)
            self.assertEqual(VideoFrameRateCode(code).code, code)
        with self.assertRaises(OverflowError):
            VideoResolutionCode(256)
        with self.assertRaises(OverflowError):
            CapabilitySubscribe(CapabilityTopic.VIDEO_FORMATS, -1)
        with self.assertRaises(AttributeError):
            VideoResolutionCode(0x10).code = 1

    def test_payloads_do_not_leak_through_repr_or_codec_errors(self) -> None:
        marker = b"synthetic-private-payload"
        message = Message(2, 0x28, 1, 0x40, 0, 0x99, marker)
        self.assertNotIn(marker.decode(), repr(message))
        with self.assertRaises(osmo_camera.ProtocolError) as error:
            CapabilityNotification.decode(marker)
        self.assertNotIn(marker.decode(), str(error.exception))
        corrupted = bytearray(message.encode())
        corrupted[-1] ^= 1
        with self.assertRaises(osmo_camera.ProtocolError):
            Message.decode(bytes(corrupted))

    def test_connection_requires_approval_before_file_access(self) -> None:
        with self.assertRaises(osmo_camera.ConfigurationError):
            osmo_camera.ControlSession.connect("missing-synthetic-profile.toml")
        cancellation = osmo_camera.Cancellation()
        self.assertFalse(cancellation.is_cancelled)
        cancellation.cancel()
        self.assertTrue(cancellation.is_cancelled)
        with self.assertRaises(osmo_camera.CancelledError):
            osmo_camera.ControlSession.connect(
                "missing-synthetic-profile.toml",
                approved=True,
                cancellation=cancellation,
            )
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "synthetic.toml"
            path.write_text("version = 1\n", encoding="utf-8")
            with self.assertRaises(osmo_camera.ConfigurationError):
                osmo_camera.ControlSession.connect(path, approved=True)

    def test_stubs_cover_native_exports_and_members(self) -> None:
        tree = ast.parse(
            Path(_native.__file__).with_suffix(".pyi").read_text(encoding="utf-8")
        )
        declarations = {
            item.name: item
            for item in tree.body
            if isinstance(item, (ast.ClassDef, ast.FunctionDef))
        }
        exported = {name for name in dir(_native) if not name.startswith("_")}
        self.assertEqual(set(declarations), exported)
        for name, declaration in declarations.items():
            if not isinstance(declaration, ast.ClassDef):
                continue
            native = getattr(_native, name)
            declared = {
                item.name
                for item in declaration.body
                if isinstance(item, ast.FunctionDef) and not item.name.startswith("_")
            }
            declared.update(
                item.target.id
                for item in declaration.body
                if isinstance(item, ast.AnnAssign) and isinstance(item.target, ast.Name)
            )
            actual = {member for member in vars(native) if not member.startswith("_")}
            self.assertEqual(declared, actual, name)
            for base in declaration.bases:
                if isinstance(base, ast.Attribute) and isinstance(base.value, ast.Name):
                    if base.value.id == "builtins":
                        self.assertTrue(hasattr(builtins, base.attr), base.attr)


if __name__ == "__main__":
    unittest.main()
