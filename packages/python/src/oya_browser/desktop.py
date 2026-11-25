"""oya.desktop: the person's own desktop browser, signed in to their sites, as in the TypeScript SDK's agent.ts.

connect() returns the running desktop app, or pairs it: it opens an oya:// link, the person clicks Connect in the
app, and connect() waits until it shows up on this key. The handle is marked reused, so stop() closes only this
SDK's own tab and never disconnects the person's browser.
"""

import asyncio
import re
import subprocess
import sys
import time
from collections.abc import Callable
from typing import Any
from urllib.parse import quote

from ._constants import (
    DESKTOP_DOWNLOAD_URL,
    DESKTOP_PROVIDER,
    DESKTOP_TIMEOUT_S,
    READY_POLL_S,
    STATUS_GATEWAY_TIMEOUT,
)
from ._http import AsyncHttp, Http, segment
from .browser import AsyncBrowser, Browser
from .errors import OyaError

# rundll32 on Windows: `start` would read the & in the link as a command break.
OPENERS = {"darwin": ["open"], "linux": ["xdg-open"], "win32": ["rundll32", "url.dll,FileProtocolHandler"]}
"""How each OS opens a link in the app registered for it."""


def find_desktop(fleet: Any, persona: str | None) -> dict[str, Any] | None:
    """The live desktop app, and with `persona` (an id or name) only one signed in as that persona."""
    for b in fleet or []:
        if b.get("provider") != DESKTOP_PROVIDER or b.get("health") == "dead":
            continue
        if not persona or persona in (b.get("persona"), b.get("personaName")):
            return dict(b)
    return None


def pairing_link(base_url: str, code: str) -> str:
    """The oya:// link that pairs the desktop app with this key, through a single-use code."""
    server = re.sub(r"^http", "ws", base_url) + "/ws"
    return f"oya://connect?code={quote(code, safe='')}&server={quote(server, safe='')}"


def open_link(link: str) -> None:
    """Opens the link on this machine, which hands it to the desktop app. Best effort: a failure shows as the 504."""
    opener = OPENERS.get(sys.platform)
    if not opener:
        return
    try:
        subprocess.Popen([*opener, link], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except OSError:
        pass


def _pairing_body(persona: str | None) -> dict[str, Any]:
    return {"persona": persona} if persona else {}


def not_connected(link: str) -> OyaError:
    """The 504 when the desktop app never connected, with what to do."""
    how = f"Install it from {DESKTOP_DOWNLOAD_URL} if needed, then open {link} and click Connect."
    return OyaError(f"The Oya desktop app did not connect. {how}", STATUS_GATEWAY_TIMEOUT, {"link": link})


def handle_info(browser_id: str, record: dict[str, Any]) -> dict[str, Any]:
    """The started-browser info for a desktop handle: reused, so stop() leaves the person's browser running."""
    persona = record.get("persona") or "default"
    return {
        "id": browser_id,
        "provider": DESKTOP_PROVIDER,
        "persona": persona,
        "cdpUrl": record.get("cdpUrl"),
        "status": "ready",
        "reused": True,
    }


class Desktop:
    """oya.desktop on the sync client."""

    def __init__(self, http: Http, fleet: Callable[[], list[dict[str, Any]]]) -> None:
        self._http, self._fleet = http, fleet

    def connect(self, persona: str | None = None, open: bool = True, timeout_s: float = DESKTOP_TIMEOUT_S) -> Browser:
        """The person's desktop browser; when it is not connected (or is on another persona) this pairs it."""
        found = find_desktop(self._fleet(), persona)
        if not found:
            code = self._http.request("POST", "/api/pairing", _pairing_body(persona))["code"]
            link = pairing_link(self._http.origin.base_url, code)
            if open:
                open_link(link)
            found = self._wait(link, timeout_s, persona)
        return self._handle(found["id"])

    def _wait(self, link: str, timeout_s: float, persona: str | None) -> dict[str, Any]:
        # The check runs once more at the deadline, so an app that connects during the last pause is not a 504.
        deadline = time.monotonic() + timeout_s
        while not (found := find_desktop(self._fleet(), persona)):
            if time.monotonic() >= deadline:
                raise not_connected(link)
            time.sleep(min(READY_POLL_S, max(0.0, deadline - time.monotonic())))
        return found

    def _handle(self, browser_id: str) -> Browser:
        record = self._http.request("GET", f"/api/browsers/{segment(browser_id)}")
        return Browser(self._http, handle_info(browser_id, record))


class AsyncDesktop:
    """oya.desktop on the asyncio client."""

    def __init__(self, http: AsyncHttp, fleet: Callable[[], Any]) -> None:
        self._http, self._fleet = http, fleet

    async def connect(
        self, persona: str | None = None, open: bool = True, timeout_s: float = DESKTOP_TIMEOUT_S
    ) -> AsyncBrowser:
        """The person's desktop browser; when it is not connected (or is on another persona) this pairs it."""
        found = find_desktop(await self._fleet(), persona)
        if not found:
            code = (await self._http.request("POST", "/api/pairing", _pairing_body(persona)))["code"]
            link = pairing_link(self._http.origin.base_url, code)
            if open:
                open_link(link)
            found = await self._wait(link, timeout_s, persona)
        return await self._handle(found["id"])

    async def _wait(self, link: str, timeout_s: float, persona: str | None) -> dict[str, Any]:
        loop = asyncio.get_running_loop()
        deadline = loop.time() + timeout_s
        while not (found := find_desktop(await self._fleet(), persona)):
            if loop.time() >= deadline:
                raise not_connected(link)
            await asyncio.sleep(min(READY_POLL_S, max(0.0, deadline - loop.time())))
        return found

    async def _handle(self, browser_id: str) -> AsyncBrowser:
        record = await self._http.request("GET", f"/api/browsers/{segment(browser_id)}")
        return AsyncBrowser(self._http, handle_info(browser_id, record))
