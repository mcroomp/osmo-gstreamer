"""Thin Python bindings to the native Osmo camera library."""

from . import capabilities, protocol
from ._native import (
    CameraError,
    CameraRejectedError,
    CameraTimeoutError,
    Cancellation,
    CancelledError,
    ConfigurationError,
    ControlSession,
    OwnershipError,
    ProtocolError,
    UnsupportedError,
    version,
)

__all__ = [
    "CameraError",
    "CameraRejectedError",
    "CameraTimeoutError",
    "Cancellation",
    "CancelledError",
    "ConfigurationError",
    "ControlSession",
    "OwnershipError",
    "ProtocolError",
    "UnsupportedError",
    "capabilities",
    "protocol",
    "version",
]
