"""Starting a browser, written once for sync and async: the start body, and how to read the fleet while waiting.

A cloud browser dials in on its own, so start() answers "starting" and the client polls the fleet until it shows up,
stopping early when the control plane says its creation ended, then asks for its CDP URL. Same as the TS SDK.
"""

import uuid
from typing import Any

from ._constants import MS_PER_SECOND, READY_TIMEOUT_S, STATUS_CONFLICT, STATUS_GATEWAY_TIMEOUT, STATUS_NOT_FOUND
from ._http import _header_safe
from .errors import OyaError

ENDED = ("failed", "stopped", "unknown_outcome")
"""Session states from which a browser will never come up."""


def _camel(name: str) -> str:
    """Converts snake_case option names to the API's camelCase spelling."""
    head, *rest = name.split("_")
    camel = head + "".join(part.title() for part in rest)
    return "personaGroup" if camel == "profileGroup" else camel


def start_body(persona: str | None, provider: str | None, name: str | None, extra: dict[str, Any]) -> dict[str, Any]:
    """The body POST /api/browsers/start takes; options left out are not sent."""
    normalized = {_camel(k): v for k, v in extra.items() if v is not None}
    body = {"profile": persona, "provider": provider, "name": name, **normalized}
    return {k: v for k, v in body.items() if v is not None}


def ready_timeout(ready_timeout_s: float | None, body: dict[str, Any]) -> float:
    """How long to wait for a starting browser: caller's budget, or READY_TIMEOUT_S plus any queueMs."""
    if ready_timeout_s is not None:
        return ready_timeout_s
    queue_ms = body.get("queueMs")
    extra_s = (
        float(queue_ms) / MS_PER_SECOND
        if isinstance(queue_ms, (int, float)) and not isinstance(queue_ms, bool) and queue_ms > 0
        else 0.0
    )
    return READY_TIMEOUT_S + extra_s


def start_headers(idempotency_key: str | None) -> dict[str, str]:
    """A retried start never makes a second browser."""
    key = _header_safe(idempotency_key, "idempotency_key") if idempotency_key is not None else str(uuid.uuid4())
    return {"Idempotency-Key": key}


def is_up(fleet: Any, browser_id: str) -> bool:
    """Whether the browser is in the fleet and alive."""
    return any(isinstance(b, dict) and b.get("id") == browser_id and b.get("health") != "dead" for b in fleet or [])


def raise_if_ended(session: Any) -> None:
    """Raises when the browser's creation has ended."""
    if isinstance(session, dict) and session.get("state") in ENDED:
        raise OyaError(f"Browser creation ended in {session['state']}", STATUS_CONFLICT, session)


def not_recorded(error: OyaError) -> bool:
    """A session not recorded yet is fine while a browser starts."""
    return error.status == STATUS_NOT_FOUND


def not_ready(browser_id: str, timeout_s: float) -> OyaError:
    """The browser did not dial in before its budget ran out."""
    return OyaError(f"Browser {browser_id} did not come up within {round(timeout_s)}s", STATUS_GATEWAY_TIMEOUT)
