"""Every browser call written once, as data: the sync and async Browser only differ in whether they await.

Paths, bodies and timeouts match the TypeScript SDK's browser.ts call for call.
"""

import uuid
from dataclasses import dataclass, field
from typing import Any
from urllib.parse import quote

from ._constants import (
    AGENT_TIMEOUT_S,
    AIMED_SCROLL_AMOUNT,
    CHALLENGE_TIMEOUT_S,
    DEFAULT_TIMEOUT_S,
    MS_PER_SECOND,
    NAVIGATE_TIMEOUT_S,
    PLAYBOOK_TIMEOUT_S,
    STATUS_CONFLICT,
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
    """A task in plain language; with `schema`, the answer comes back as data in that shape.

    Values left out are left out of the body, as the TypeScript SDK's undefined is: the server refuses
    "data": null and "secrets": null, so a plain ask() with neither failed.
    """
    fields = {"data": data, "secrets": secrets, "schema": schema}
    body: dict[str, Any] = {"messages": [{"role": "user", "content": prompt}]}
    body.update({k: v for k, v in fields.items() if v is not None})
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


def scroll(direction: str, amount: int | None, at: tuple[int, int] | None = None) -> dict[str, Any]:
    """Scroll params; `amount` left out scrolls about a screen.

    With `at`, the wheel is aimed at that (x, y) point, an inner scroller such as a results panel, as one wheel
    event: drivers only honour x/y in that mode, and it moves AIMED_SCROLL_AMOUNT pixels when no amount is given.
    """
    if at is None:
        return {"direction": direction} if amount is None else {"direction": direction, "amount": amount}
    x, y = at
    return {
        "direction": direction,
        "amount": AIMED_SCROLL_AMOUNT if amount is None else amount,
        "x": x,
        "y": y,
        "smooth": False,
    }


def active_url(tabs: list[dict[str, Any]]) -> str:
    """The active tab's URL, or empty when there is none."""
    return next((str(t.get("url") or "") for t in tabs if t.get("active")), "")


def handle_dialog(browser_id: str, accept: bool, prompt_text: str | None) -> Call:
    """Answers a confirm() or prompt() holding the page."""
    params: dict[str, Any] = {"accept": accept}
    if prompt_text is not None:
        params["prompt_text"] = prompt_text
    return command(browser_id, "handle_dialog", params)


def open_tab(browser_id: str, url: str | None) -> Call:
    """Opens a tab, at `url` when given; it may load a page, so it gets a navigation's budget."""
    return command(browser_id, "open_tab", {} if url is None else {"url": url}, NAVIGATE_TIMEOUT_S)


def assert_captcha_cleared(result: dict[str, Any]) -> None:
    """After captcha="auto": a CAPTCHA still on screen needs a person, so goto() raises 409 rather than go on."""
    if not result.get("present") or result.get("solved") or result.get("invisible"):
        return
    message = result.get("error") or "CAPTCHA needs attention. Call solve_captcha() again or open the live view."
    raise OyaError(str(message), STATUS_CONFLICT, result)


def session(browser_id: str, action: str) -> Call:
    """A control-plane call on this browser's session: "ticket" for the live stream, "share" for a link."""
    return Call("POST", f"/api/control/sessions/{segment(browser_id)}/{action}", {})


def share(browser_id: str, control: bool, expires_in_s: int) -> Call:
    """Mints a credential scoped to this one browser, for a share link."""
    body = {"control": control, "expiresIn": expires_in_s}
    return Call("POST", f"/api/control/sessions/{segment(browser_id)}/share", body)


def live_stream_url(base_url: str, browser_id: str, ticket: str) -> str:
    """The SSE frame stream; EventSource cannot set headers, so the single-use ticket rides in the query."""
    return f"{base_url}/api/live/{segment(browser_id)}?ticket={quote(ticket, safe='')}"


def share_link(base_url: str, browser_id: str, credential: dict[str, Any]) -> dict[str, Any]:
    """The public live-view link; the token rides in the fragment, so no server log or Referer sees it."""
    url = f"{base_url}/live/{segment(browser_id)}#t={quote(str(credential['token']), safe='')}"
    return {"url": url, "id": credential["id"], "expiresAt": credential.get("expiresAt")}


def live_view_url(base_url: str, browser_id: str) -> str:
    """The console deep link: carries no credential, unlike a stream URL with the key in it would."""
    return f"{base_url}/dashboard/?browser={quote(browser_id, safe='')}"


def submit(browser_id: str, prompt: str | None, playbook: str | None, values: dict[str, Any]) -> Call:
    """Starts a prompt or a playbook as a background run; exactly one of the two."""
    if (prompt is None) == (playbook is None):
        raise ValueError("submit() takes a prompt or playbook=..., exactly one")
    task = {"prompt": prompt} if prompt is not None else {"playbook": playbook}
    body = {**task, **{k: v for k, v in values.items() if v is not None}}
    return Call("POST", f"/api/browsers/{segment(browser_id)}/runs", body)
