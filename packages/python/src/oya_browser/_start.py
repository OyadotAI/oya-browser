"""Starting a browser, written once for sync and async: the start body, and how to read the fleet while waiting.

A cloud browser dials in on its own, so start() answers "starting" and the client polls the fleet until it shows up,
stopping early when the control plane says its creation ended, then asks for its CDP URL. Same as the TS SDK.
"""

import uuid
from typing import Any

from ._constants import STATUS_CONFLICT, STATUS_GATEWAY_TIMEOUT, STATUS_NOT_FOUND
from .errors import OyaError

ENDED = ("failed", "stopped", "unknown_outcome")
"""Session states from which a browser will never come up."""


def start_body(persona: str | None, provider: str | None, name: str | None, extra: dict[str, Any]) -> dict[str, Any]:
    """The body POST /api/browsers/start takes; options left out are not sent."""
    body = {"profile": persona, "provider": provider, "name": name, **extra}
    return {k: v for k, v in body.items() if v is not None}


def start_headers(idempotency_key: str | None) -> dict[str, str]:
    """A retried start never makes a second browser."""
    return {"Idempotency-Key": idempotency_key or str(uuid.uuid4())}


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
