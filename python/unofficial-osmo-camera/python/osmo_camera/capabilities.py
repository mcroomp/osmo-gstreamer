"""Immutable capability evidence, with explicit catalog and camera context.

Published candidates never imply current availability or authorize operations.
"""

from collections.abc import Callable, Iterable
from dataclasses import dataclass
from enum import Enum
from typing import Generic, TypeVar

from ._native import (
    CapabilityCatalog,
    CaptureMode,
    CommandContract,
    ConfigurationError,
    PublishedAspect,
    PublishedPreviewObservation,
    PublishedRecordingFormat,
    UnsupportedError,
)
from .protocol.typed import (
    CapabilityNotification,
    CapabilityTopic,
    SlowMotionCapability,
    VideoFormatCapability,
    VideoFrameRateCode,
)

Live = TypeVar("Live")
Published = TypeVar("Published")


class Availability(str, Enum):
    """Interpretation of camera evidence, independent of published candidates."""

    MISSING = "missing"
    AVAILABLE = "available"
    UNAVAILABLE = "unavailable"
    UNKNOWN = "unknown"


class FirmwareScope(str, Enum):
    """Exact caller-context comparison, not device verification or permission."""

    MATCHES = "matches"
    DIFFERENT = "different"
    UNKNOWN = "unknown"
    UNSPECIFIED = "unspecified"


@dataclass(frozen=True)
class TopicCapabilities(Generic[Live, Published]):
    """One topic's camera evidence and separately scoped published candidates."""

    topic: CapabilityTopic
    state: Availability
    current: tuple[Live, ...] | None
    published: tuple[Published, ...]
    notification: CapabilityNotification | None
    error: UnsupportedError | None = None


@dataclass(frozen=True)
class SlowMotionCandidate:
    """Published ratio candidate retaining its contributing recording formats."""

    frame_rate: VideoFrameRateCode
    factor: int
    formats: tuple[PublishedRecordingFormat, ...]


@dataclass(frozen=True)
class CommandCoverage:
    """Native command contract with caller-declared firmware scope."""

    contract: CommandContract
    firmware_scope: FirmwareScope


@dataclass(frozen=True)
class Capabilities:
    """Immutable, non-atomic evidence view for explicitly selected context."""

    catalog: CapabilityCatalog | None
    mode: CaptureMode | None
    firmware: str | None
    recording_formats: TopicCapabilities[
        VideoFormatCapability, PublishedRecordingFormat
    ]
    slow_motion_ratios: TopicCapabilities[SlowMotionCapability, SlowMotionCandidate]
    capture_aspects: TopicCapabilities[int, PublishedAspect]
    command_coverage: tuple[CommandCoverage, ...]
    preview_observations: tuple[PublishedPreviewObservation, ...]
    other_notifications: tuple[CapabilityNotification, ...]

    @property
    def model(self) -> str | None:
        return self.catalog.model if self.catalog is not None else None

    @property
    def topics(
        self,
    ) -> tuple[
        TopicCapabilities[VideoFormatCapability, PublishedRecordingFormat],
        TopicCapabilities[SlowMotionCapability, SlowMotionCandidate],
        TopicCapabilities[int, PublishedAspect],
    ]:
        return self.recording_formats, self.slow_motion_ratios, self.capture_aspects

    @property
    def complete(self) -> bool:
        """Whether all three topics have known availability, including unavailable."""
        return all(
            topic.state in (Availability.AVAILABLE, Availability.UNAVAILABLE)
            for topic in self.topics
        )

    @property
    def missing(self) -> tuple[CapabilityTopic, ...]:
        """Topics without any received notification."""
        return tuple(
            topic.topic for topic in self.topics if topic.state is Availability.MISSING
        )

    @property
    def unresolved(self) -> tuple[CapabilityTopic, ...]:
        """Topics with missing notifications or unknown availability."""
        return tuple(
            topic.topic
            for topic in self.topics
            if topic.state in (Availability.MISSING, Availability.UNKNOWN)
        )


def _resolve_topic(
    topic: CapabilityTopic,
    published: tuple[Published, ...],
    notification: CapabilityNotification | None,
    decode: Callable[[CapabilityNotification], list[Live] | None],
) -> TopicCapabilities[Live, Published]:
    if notification is None:
        return TopicCapabilities(topic, Availability.MISSING, None, published, None)
    try:
        available = notification.table().available
    except UnsupportedError as error:
        return TopicCapabilities(
            topic, Availability.UNKNOWN, None, published, notification, error
        )
    if not available:
        return TopicCapabilities(
            topic, Availability.UNAVAILABLE, (), published, notification
        )
    rows = decode(notification)
    if rows is None:
        raise ConfigurationError("Available capability table decoded as unavailable")
    return TopicCapabilities(
        topic, Availability.AVAILABLE, tuple(rows), published, notification
    )


def _validate_context(
    catalog: CapabilityCatalog | None, mode: CaptureMode | None, firmware: str | None
) -> None:
    if catalog is not None and not isinstance(catalog, CapabilityCatalog):
        raise ConfigurationError("catalog must be a CapabilityCatalog or None")
    if mode is not None and not isinstance(mode, CaptureMode):
        raise ConfigurationError("mode must be a CaptureMode or None")
    if firmware is not None and (not isinstance(firmware, str) or not firmware.strip()):
        raise ConfigurationError("firmware must be a non-empty string or None")


def resolve_capabilities(
    snapshots: Iterable[CapabilityNotification] = (),
    *,
    catalog: CapabilityCatalog | None = None,
    mode: CaptureMode | None = None,
    firmware: str | None = None,
) -> Capabilities:
    """Resolve published candidates and live snapshots without hardware access.

    Model and mode are explicitly selected, never detected. Unknown mode codes
    survive without inheriting another mode's candidates. Without a mode, every
    published recording candidate retains its individual mode scope.
    Mode filters only published recording formats, never current camera rows.
    Missing and unknown availability remain unconfirmed even with a catalog.
    Duplicate known topics and malformed data raise; unknown topics are retained.
    """
    _validate_context(catalog, mode, firmware)
    by_topic: dict[bytes, CapabilityNotification] = {}
    other = []
    for snapshot in snapshots:
        if snapshot.topic is None:
            other.append(snapshot)
            continue
        if snapshot.name in by_topic:
            raise ConfigurationError(
                "Duplicate topic snapshots require explicit selection"
            )
        by_topic[snapshot.name] = snapshot
    published = tuple(catalog.recording_formats) if catalog is not None else ()
    recording = tuple(row for row in published if mode is None or row.mode == mode)
    ratios: dict[tuple[int, int], list[PublishedRecordingFormat]] = {}
    for row in published:
        factor = row.slow_motion_factor
        if factor is not None:
            ratios.setdefault((row.frame_rate.code, factor), []).append(row)
    slow = tuple(
        SlowMotionCandidate(rows[0].frame_rate, factor, tuple(rows))
        for (_, factor), rows in ratios.items()
    )
    commands = []
    for contract in catalog.command_contracts if catalog is not None else ():
        if contract.firmware is None:
            scope = FirmwareScope.UNSPECIFIED
        elif firmware is None:
            scope = FirmwareScope.UNKNOWN
        else:
            scope = (
                FirmwareScope.MATCHES
                if contract.firmware == firmware
                else FirmwareScope.DIFFERENT
            )
        commands.append(CommandCoverage(contract, scope))
    video = CapabilityTopic.VIDEO_FORMATS
    slow_motion = CapabilityTopic.SLOW_MOTION_RATIOS
    aspect = CapabilityTopic.CAPTURE_ASPECTS
    return Capabilities(
        catalog=catalog,
        mode=mode,
        firmware=firmware,
        recording_formats=_resolve_topic(
            video,
            recording,
            by_topic.get(video.name),
            lambda item: item.video_formats(),
        ),
        slow_motion_ratios=_resolve_topic(
            slow_motion,
            slow,
            by_topic.get(slow_motion.name),
            lambda item: item.slow_motion_ratios(),
        ),
        capture_aspects=_resolve_topic(
            aspect,
            tuple(catalog.recording_aspects) if catalog is not None else (),
            by_topic.get(aspect.name),
            lambda item: item.capture_aspects(),
        ),
        command_coverage=tuple(commands),
        preview_observations=tuple(catalog.preview_observations)
        if catalog is not None
        else (),
        other_notifications=tuple(other),
    )


__all__ = [
    "Availability",
    "Capabilities",
    "CapabilityCatalog",
    "CaptureMode",
    "CommandContract",
    "CommandCoverage",
    "FirmwareScope",
    "PublishedAspect",
    "PublishedPreviewObservation",
    "PublishedRecordingFormat",
    "SlowMotionCandidate",
    "TopicCapabilities",
    "resolve_capabilities",
]
