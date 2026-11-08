"""Known wire layouts and validation, not verified model/firmware support.

Typed requests convert to raw requests. Replies and notifications retain their
original raw frame when decoded from it. Documented unknown values stay lossless;
unsupported interpretations remain explicit errors.
"""

from .._native import (
    CapabilityNotification,
    CapabilitySubscribe,
    CapabilitySubscriptionReply,
    CapabilityTable,
    CapabilityTopic,
    SlowMotionCapability,
    VideoFormatCapability,
    VideoFrameRateCode,
    VideoResolutionCode,
)

__all__ = [
    "CapabilityNotification",
    "CapabilitySubscribe",
    "CapabilitySubscriptionReply",
    "CapabilityTable",
    "CapabilityTopic",
    "SlowMotionCapability",
    "VideoFormatCapability",
    "VideoFrameRateCode",
    "VideoResolutionCode",
]
