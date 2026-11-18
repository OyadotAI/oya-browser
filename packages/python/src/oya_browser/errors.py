"""OyaError: every failure the SDK raises, with the HTTP status and the server's body (as the TS SDK's)."""

from typing import Any


class OyaError(Exception):
    """A call that failed: the server's message, its HTTP status (or the SDK's own) and the parsed body."""

    def __init__(self, message: str, status: int, body: Any = None) -> None:
        super().__init__(message)
        self.status = status
        self.body = body
