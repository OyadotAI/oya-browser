"""Every browser call written once, as data: the sync and async Browser only differ in whether they await.

Paths, bodies and timeouts match the TypeScript SDK's browser.ts call for call.
"""

import uuid
from dataclasses import dataclass, field
from typing import Any

from ._constants import (
    AGENT_TIMEOUT_S,
    CHALLENGE_TIMEOUT_S,
    DEFAULT_TIMEOUT_S,
    MS_PER_SECOND,
    NAVIGATE_TIMEOUT_S,
    PLAYBOOK_TIMEOUT_S,
    STATUS_SERVER_ERROR,
    STATUS_UNPROCESSABLE,
    STOP_TIMEOUT_S,
    WAIT_FOR_GRACE_S,
)
from ._http import segment
from .errors import OyaError

LANE_HEADER = "X-Oya-Agent"
"""A desktop browser handed to start() gives each lane a tab of its own, so the person keeps theirs."""


def sdk_lane() -> str:
    """A lane name of this SDK's own, unique per Browser object."""
    return f"sdk-{uuid.uuid4()}"


@dataclass(frozen=True)
class Call:
    """One HTTP call: method, path, JSON body and time budget."""

    method: str
    path: str
    body: Any = None
    timeout: float = DEFAULT_TIMEOUT_S
    headers: dict[str, str] = field(default_factory=dict)


def command(browser_id: str, action: str, params: dict[str, Any], timeout: float = DEFAULT_TIMEOUT_S) -> Call:
    """A browser command, such as navigate, analyze, click or type."""
    return Call("POST", f"/api/browsers/{segment(browser_id)}/command", {"action": action, "params": params}, timeout)


def command_data(result: Any, action: str) -> Any:
    """A command's data; a command that ran and failed raises."""
    if isinstance(result, dict) and result.get("ok") is False:
        raise OyaError(result.get("error") or f"{action} failed", STATUS_UNPROCESSABLE, result)
    return result.get("data") if isinstance(result, dict) else result


def agent_answer(result: Any) -> dict[str, Any]:
    """An agent answer; the server sends 200 up front to keep long runs alive, so failures arrive in the body."""
    if not isinstance(result, dict):
        raise OyaError("The agent sent no answer", STATUS_SERVER_ERROR, result)
    if result.get("error"):
        raise OyaError(str(result["error"]), int(result.get("status") or STATUS_SERVER_ERROR), result)
    return result


def element(element_id: int | str) -> dict[str, Any]:
    """The params that address an element by its id from analyze()."""
    try:
        number = int(element_id)
    except (TypeError, ValueError):
        raise ValueError(f"element ids come from analyze(); {element_id!r} is not one") from None
    return {"element_id": number, "selector": f'[data-ac-id="{number}"]'}


def navigate(browser_id: str, url: str) -> Call:
    """Opens `url` and waits for it to load."""
    return command(browser_id, "navigate", {"url": url}, NAVIGATE_TIMEOUT_S)


def wait_for(browser_id: str, selector: str, timeout_s: float) -> Call:
    """Waits until `selector` matches."""
    params = {"selector": selector, "timeout": int(timeout_s * MS_PER_SECOND)}
    return command(browser_id, "wait", params, timeout_s + WAIT_FOR_GRACE_S)


def challenge(browser_id: str, kind: str) -> Call:
    """Solves a CAPTCHA ("captcha") or completes MFA ("mfa")."""
    return Call("POST", f"/api/browsers/{segment(browser_id)}/{kind}", {}, CHALLENGE_TIMEOUT_S)


def ask(browser_id: str, prompt: str, data: Any, secrets: Any, schema: Any = None) -> Call:
    """A task in plain language; with `schema`, the answer comes back as data in that shape."""
    body: dict[str, Any] = {"messages": [{"role": "user", "content": prompt}], "data": data, "secrets": secrets}
    if schema is not None:
        body["schema"] = schema
    return Call("POST", f"/api/browsers/{segment(browser_id)}/chat", body, AGENT_TIMEOUT_S)


def to_playbook(browser_id: str, name: str) -> Call:
    """Saves the last ask() on this browser as a named playbook."""
    return Call("POST", f"/api/browsers/{segment(browser_id)}/playbooks", {"name": name}, PLAYBOOK_TIMEOUT_S)


def play(browser_id: str, name: str, data: Any, auto_heal: bool) -> Call:
    """Replays a playbook with no model in the loop."""
    path = f"/api/browsers/{segment(browser_id)}/playbooks/{segment(name, 'name')}/play"
    return Call("POST", path, {"variables": data or {}, "autoHeal": auto_heal}, AGENT_TIMEOUT_S)


def stop(browser_id: str, reused: bool) -> Call:
    """Stops a browser this SDK started; for one it borrowed (a person's desktop), ends only its own lane."""
    action = "lane/end" if reused else "stop"
    return Call("POST", f"/api/browsers/{segment(browser_id)}/{action}", {}, STOP_TIMEOUT_S)


def scroll(direction: str, amount: int | None) -> dict[str, Any]:
    """Scroll params; `amount` left out scrolls about a screen."""
    return {"direction": direction} if amount is None else {"direction": direction, "amount": amount}
