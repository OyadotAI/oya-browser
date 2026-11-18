"""What a submitted run's poll means, written once for Run and AsyncRun.

A poll is a new attention request, the result, or the failure. Same rules as the TypeScript SDK's run-watch.ts.
"""

import logging
from collections.abc import Callable
from dataclasses import dataclass
from typing import Any
from urllib.parse import urljoin

from ._constants import MAX_RUN_POLL_ERRORS, STATUS_SERVER_ERROR, STATUS_UNKNOWN
from ._http import segment
from ._ops import Call
from .errors import OyaError

log = logging.getLogger("oya_browser")

Callback = Callable[[Any], Any]


@dataclass(frozen=True)
class Callbacks:
    """What a run fires: on_success(result), on_failure(OyaError), on_human_attention(request), on_healed(result)."""

    on_success: Callback | None = None
    on_failure: Callback | None = None
    on_human_attention: Callback | None = None
    on_healed: Callback | None = None


@dataclass
class Step:
    """One poll's meaning: a new attention request to report, and the result or failure once the run ended."""

    attention: dict[str, Any] | None = None
    result: dict[str, Any] | None = None
    failure: OyaError | None = None


@dataclass
class Watch:
    """What the loop carries between polls."""

    base_url: str
    respond: Callable[..., Any]
    seen: str | None = None
    errors: int = 0

    def failed_poll(self, error: Exception) -> OyaError | None:
        """None while polls fail now and then; the failure once MAX_RUN_POLL_ERRORS have failed in a row."""
        self.errors += 1
        if self.errors < MAX_RUN_POLL_ERRORS:
            return None
        return error if isinstance(error, OyaError) else OyaError(str(error), STATUS_UNKNOWN)

    def step(self, info: dict[str, Any]) -> Step:
        """Reads one successful poll; the attention request is reported once, however often it is polled."""
        self.errors = 0
        step = Step(attention=self._attention(info))
        if info.get("status") == "succeeded":
            step.result = info.get("result") or {}
        elif info.get("status") == "failed":
            status = info.get("errorStatus") or STATUS_SERVER_ERROR
            step.failure = OyaError(info.get("error") or "Run failed", int(status), info)
        return step

    def _attention(self, info: dict[str, Any]) -> dict[str, Any] | None:
        request = info.get("attention")
        if info.get("status") != "needs_attention" or not request or request.get("id") == self.seen:
            return None
        self.seen = request.get("id")
        link = request.get("liveViewUrl")
        return {**request, "liveViewUrl": urljoin(self.base_url, link) if link else link, "respond": self.respond}


def fire(callback: Callback | None, value: Any) -> Any:
    """Calls a caller's callback; one that raises is logged, never allowed to break the loop."""
    if callback is None:
        return None
    try:
        return callback(value)
    except Exception:
        log.exception("run callback raised")
        return None


def status_call(run_id: str) -> Call:
    """The run's current state."""
    return Call("GET", f"/api/runs/{segment(run_id)}")


def respond_call(run_id: str, response: str) -> Call:
    """Answers the open attention request."""
    return Call("POST", f"/api/runs/{segment(run_id)}/respond", {"response": response})
