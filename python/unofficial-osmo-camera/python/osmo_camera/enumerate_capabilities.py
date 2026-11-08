"""Explicit, bounded enumeration of current-mode capability topics through Rust."""

import argparse
import math
import sys
from collections.abc import Sequence
from pathlib import Path
from time import monotonic

from . import (
    CameraError,
    CameraTimeoutError,
    ConfigurationError,
    ControlSession,
    UnsupportedError,
)
from .capabilities import (
    Availability,
    Capabilities,
    CapabilityCatalog,
    CaptureMode,
    _validate_context,
    resolve_capabilities,
)
from .protocol.typed import (
    CapabilityNotification,
    CapabilitySubscribe,
    CapabilityTopic,
)


class _EnumerationTimeout(CameraTimeoutError):
    """An incomplete enumeration retaining only validated topic snapshots."""

    def __init__(
        self,
        snapshots: tuple[CapabilityNotification, ...],
        missing: tuple[CapabilityTopic, ...],
    ) -> None:
        names = ", ".join(topic.name.decode("ascii") for topic in missing)
        super().__init__(f"No complete capability enumeration; missing: {names}")
        self.snapshots = snapshots
        self.missing = missing


def _collect_snapshots(
    session: ControlSession,
    *,
    timeout: float = 15.0,
    approve_subscriptions: bool = False,
    approve_acknowledgments: bool = False,
) -> tuple[CapabilityNotification, ...]:
    """Subscribe and receive one snapshot per topic, with explicit receipt ACKs.

    No mode switch or static-metadata fallback occurs. Notifications are matched
    by topic, not by an assumed relationship between sequence and subscription ID.
    The snapshots are not an atomic view across camera mode changes.
    """
    if not (approve_subscriptions and approve_acknowledgments):
        raise ConfigurationError(
            "Enumeration requires subscription and receipt-ACK approvals"
        )
    if not math.isfinite(timeout) or not 1.0 <= timeout <= 30.0:
        raise ConfigurationError("Enumeration timeout must be finite and 1..30 seconds")

    topics = (
        CapabilityTopic.VIDEO_FORMATS,
        CapabilityTopic.SLOW_MOTION_RATIOS,
        CapabilityTopic.CAPTURE_ASPECTS,
    )
    requests = tuple(
        CapabilitySubscribe(topic, index) for index, topic in enumerate(topics, start=1)
    )
    snapshots: dict[bytes, CapabilityNotification] = {}
    deadline = monotonic() + timeout

    def remaining() -> float:
        seconds = deadline - monotonic()
        if seconds < 0.1:
            raise _EnumerationTimeout(
                tuple(
                    snapshots[topic.name] for topic in topics if topic.name in snapshots
                ),
                tuple(topic for topic in topics if topic.name not in snapshots),
            )
        return seconds

    for request in requests:
        session.subscribe(
            request,
            approve_unclassified=approve_subscriptions,
            timeout=min(3.0, remaining()),
        ).require_success()

    while len(snapshots) < len(topics):
        message = session.receive(timeout=min(1.0, remaining()))
        if message is None or message.command != requests[0].command:
            continue
        notification = CapabilityNotification.from_raw(message)
        topic = notification.topic
        if topic is None:
            raise UnsupportedError("Received an unrecognized capability topic")
        session.acknowledge_capability_notification(
            message, approved=approve_acknowledgments
        )
        resolve_capabilities((notification,))
        snapshots.setdefault(notification.name, notification)

    return tuple(snapshots[topic.name] for topic in topics)


def collect_capabilities(
    session: ControlSession,
    *,
    catalog: CapabilityCatalog | None = None,
    firmware: str | None = None,
    mode: CaptureMode | None = None,
    timeout: float = 15.0,
    approve_subscriptions: bool = False,
    approve_acknowledgments: bool = False,
) -> Capabilities:
    """Collect a unified view; only a missing-topic timeout becomes partial data.

    Missing live topics remain explicitly unconfirmed. Rejections, malformed
    records, transport failures and cancellation still propagate.
    Context is validated before subscriptions. Both operation approvals are
    required; the 1..30-second window excludes connection setup. The session
    remains caller-owned, and no mode switch or automatic reconnect occurs.
    """
    _validate_context(catalog, mode, firmware)
    try:
        notifications = _collect_snapshots(
            session,
            timeout=timeout,
            approve_subscriptions=approve_subscriptions,
            approve_acknowledgments=approve_acknowledgments,
        )
    except _EnumerationTimeout as error:
        notifications = error.snapshots
    return resolve_capabilities(
        notifications, catalog=catalog, firmware=firmware, mode=mode
    )


def _print_snapshot(notification: CapabilityNotification) -> None:
    print(notification.name.decode("ascii"))
    if notification.topic == CapabilityTopic.VIDEO_FORMATS:
        rows = notification.video_formats()
        if rows is None:
            print("  unavailable in the reported state")
            return
        print(f"  {len(rows)} recording-format rows")
        for row in rows:
            print(
                f"  resolution={row.resolution.code:#04x} "
                f"dimensions={row.resolution.dimensions} "
                f"frame_rate={row.frame_rate.code:#04x} "
                f"label={row.frame_rate.name} opaque={row.opaque:#04x}"
            )
    elif notification.topic == CapabilityTopic.SLOW_MOTION_RATIOS:
        ratios = notification.slow_motion_ratios()
        if ratios is None:
            print("  unavailable in the reported state")
            return
        print(f"  {len(ratios)} slow-motion rows")
        for ratio in ratios:
            print(
                f"  base_frame_rate={ratio.frame_rate.code:#04x} factor={ratio.factor}"
            )
    elif notification.topic == CapabilityTopic.CAPTURE_ASPECTS:
        aspects = notification.capture_aspects()
        if aspects is None:
            print("  unavailable in the reported state")
            return
        print(f"  {len(aspects)} raw aspect codes: {aspects}")
    else:
        raise UnsupportedError("Cannot report an unrecognized capability topic")


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--profile", type=Path, required=True, help="Existing authorized camera profile"
    )
    parser.add_argument(
        "--timeout", type=float, default=15.0, help="Enumeration window, 1..30 seconds"
    )
    parser.add_argument(
        "--approve-registration",
        action="store_true",
        help="Allow native LAN pairing/app registration",
    )
    parser.add_argument(
        "--approve-subscriptions",
        action="store_true",
        help="Allow subscriptions, which may outlive this session",
    )
    parser.add_argument(
        "--approve-acks",
        action="store_true",
        help="Allow capability-notification receipt ACKs",
    )
    parser.add_argument(
        "--ble-first",
        action="store_true",
        help="Verify BLE pairing/wake before saved Wi-Fi association and LAN control",
    )
    parser.add_argument(
        "--approve-ble",
        action="store_true",
        help="Approve BLE discovery/pairing/wake and saved camera Wi-Fi association",
    )
    parser.add_argument(
        "--catalog", choices=("nano",), help="Explicit published model catalog"
    )
    parser.add_argument(
        "--capture-mode",
        type=lambda value: int(value, 0),
        help="Caller-declared mode byte; decimal or hex",
    )
    parser.add_argument(
        "--firmware", help="Caller-declared firmware scope, not device detection"
    )
    args = parser.parse_args(argv)
    if args.capture_mode is not None and not 0 <= args.capture_mode <= 255:
        parser.error("--capture-mode must be a byte code")
    if args.firmware is not None and not args.firmware.strip():
        parser.error("--firmware must be non-empty")
    if args.ble_first != args.approve_ble:
        parser.error("--ble-first and --approve-ble must be supplied together")
    if not (
        args.approve_registration and args.approve_subscriptions and args.approve_acks
    ):
        parser.error(
            "All three approval flags are required; no camera connection was attempted"
        )
    if not math.isfinite(args.timeout) or not 1.0 <= args.timeout <= 30.0:
        parser.error("--timeout must be finite and 1..30 seconds")

    if args.ble_first:
        print(
            "BLE-first startup: pairing/wake and health verification, saved Wi-Fi "
            "association, then LAN control. Approve on the camera if prompted."
        )
    catalog = CapabilityCatalog.nano() if args.catalog == "nano" else None
    mode = CaptureMode(args.capture_mode) if args.capture_mode is not None else None
    view: Capabilities | None = None
    try:
        with ControlSession.connect(
            args.profile,
            approved=args.approve_registration,
            allow_unclassified=args.approve_subscriptions,
            ble_first=args.ble_first,
            approve_ble=args.approve_ble,
        ) as session:
            view = collect_capabilities(
                session,
                catalog=catalog,
                mode=mode,
                firmware=args.firmware,
                timeout=args.timeout,
                approve_subscriptions=args.approve_subscriptions,
                approve_acknowledgments=args.approve_acks,
            )
        if view is None:
            raise CameraError(
                "Control session exited before capability collection completed"
            )
        if not view.complete:
            names = ", ".join(topic.name.decode("ascii") for topic in view.unresolved)
            print(
                f"Capability enumeration incomplete; unresolved: {names}",
                file=sys.stderr,
            )
            print("Partial capability observations (enumeration incomplete):")
        for topic in view.topics:
            if topic.state is Availability.UNKNOWN:
                print(
                    f"{topic.topic.name.decode('ascii')}: {topic.error}",
                    file=sys.stderr,
                )
            elif topic.notification is not None:
                _print_snapshot(topic.notification)
            if catalog is not None:
                print(
                    f"{topic.topic.name.decode('ascii')}: {len(topic.published)} "
                    "published candidates (not current availability)"
                )
        return 0 if view.complete else 1
    except CameraError as error:
        print(f"{type(error).__name__}: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
