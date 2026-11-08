"""Lossless DUML inspection and bounded requests with uninterpreted payloads.

Raw decoding still validates framing/checksums. Sending uses the native session's
approval policy; this is not unchecked socket access.
"""

from .._native import Message, MessageContext, Request

__all__ = ["Message", "MessageContext", "Request"]
