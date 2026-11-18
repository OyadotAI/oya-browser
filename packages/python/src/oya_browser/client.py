"""The clients, Oya (sync) and AsyncOya: one API key, every browser behind it.

    from oya_browser import Oya
    oya = Oya()                       # OYA_API_KEY, or the key `oya login` saved
    with oya.browser.start() as browser:
        browser.goto("https://example.com")

Which provider runs the browser (Oya Cloud, your own machines, Browserbase, Steel, Anchor, Browser Use) is
configuration on the key, not something this code has to know.
"""

import asyncio
import builtins
import time
from typing import Any

import httpx

from . import _start as st
from ._config import resolve
from ._constants import READY_POLL_S, READY_TIMEOUT_S, START_TIMEOUT_S
from ._http import AsyncHttp, Http, segment
from .browser import AsyncBrowser, Browser
from .errors import OyaError


def _playbook(name: str) -> str:
    return f"/api/playbooks/{segment(name, 'name')}"


class Browsers:
    """oya.browser: start, reattach to, list and stop browsers."""

    def __init__(self, http: Http) -> None:
        self._http = http

    def start(
        self,
        persona: str | None = None,
        provider: str | None = None,
        name: str | None = None,
        ready_timeout_s: float = READY_TIMEOUT_S,
        idempotency_key: str | None = None,
        **extra: Any,
    ) -> Browser:
        """Starts a browser and waits until it can take commands."""
        body, headers = st.start_body(persona, provider, name, extra), st.start_headers(idempotency_key)
        started = self._http.request("POST", "/api/browsers/start", body, START_TIMEOUT_S, headers=headers)
        if started.get("status") == "starting":
            self._wait(started["id"], ready_timeout_s)
            started["cdpUrl"] = self._record(started["id"]).get("cdpUrl")
        return Browser(self._http, started)

    def _record(self, browser_id: str) -> dict[str, Any]:
        result: dict[str, Any] = self._http.request("GET", f"/api/browsers/{segment(browser_id)}")
        return result

    def _wait(self, browser_id: str, timeout_s: float) -> None:
        deadline = time.monotonic() + timeout_s
        while not st.is_up(self.list(), browser_id):
            try:
                st.raise_if_ended(self._http.request("GET", f"/api/control/sessions/{segment(browser_id)}"))
            except OyaError as e:
                if not st.not_recorded(e):
                    raise
            if time.monotonic() >= deadline:
                raise st.not_ready(browser_id, timeout_s)
            time.sleep(min(READY_POLL_S, max(0.0, deadline - time.monotonic())))

    def get(self, browser_id: str) -> Browser:
        """Reattaches to a browser that is already running."""
        return Browser(self._http, {**self._record(browser_id), "reused": False})

    def list(self) -> builtins.list[dict[str, Any]]:
        """Every browser on this key."""
        result: builtins.list[dict[str, Any]] = self._http.request("GET", "/api/browsers")
        return result

    def stop(self, ids: builtins.list[str]) -> dict[str, Any]:
        """Stops these browsers; each reports separately."""
        result: dict[str, Any] = self._http.request("POST", "/api/browsers/stop", {"ids": ids}, START_TIMEOUT_S)
        return result

    def stop_all(self) -> int:
        """Stops every browser on this key; returns how many stopped."""
        return int(self._http.request("POST", "/api/browsers/stop", {"all": True}, START_TIMEOUT_S).get("stopped", 0))


class Playbooks:
    """oya.playbooks: the playbooks saved with browser.to_playbook()."""

    def __init__(self, http: Http) -> None:
        self._http = http

    def list(self) -> builtins.list[dict[str, Any]]:
        """Every playbook on this key."""
        result: builtins.list[dict[str, Any]] = self._http.request("GET", "/api/playbooks").get("playbooks", [])
        return result

    def get(self, name: str) -> dict[str, Any]:
        """One playbook, with its steps, variables and Playwright code."""
        result: dict[str, Any] = self._http.request("GET", _playbook(name))
        return result

    def export(self, name: str) -> dict[str, Any]:
        """A playbook as a portable document, to import on another key or keep."""
        result: dict[str, Any] = self._http.request("GET", f"{_playbook(name)}/export")
        return result

    def remove(self, name: str) -> None:
        """Deletes a playbook."""
        self._http.request("DELETE", _playbook(name))


class Oya:
    """The synchronous client. Use it as a context manager, or call close()."""

    def __init__(self, api_key: str | None = None, base_url: str | None = None, client: httpx.Client | None = None):
        origin = resolve(api_key, base_url)
        self._http = Http(origin, client)
        self.browser = Browsers(self._http)
        self.playbooks = Playbooks(self._http)

    def close(self) -> None:
        """Closes the connection pool; running browsers keep running until stopped."""
        self._http.close()

    def __enter__(self) -> "Oya":
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()


class AsyncBrowsers:
    """oya.browser, with asyncio: start, reattach to, list and stop browsers."""

    def __init__(self, http: AsyncHttp) -> None:
        self._http = http

    async def start(
        self,
        persona: str | None = None,
        provider: str | None = None,
        name: str | None = None,
        ready_timeout_s: float = READY_TIMEOUT_S,
        idempotency_key: str | None = None,
        **extra: Any,
    ) -> AsyncBrowser:
        """Starts a browser and waits until it can take commands."""
        body, headers = st.start_body(persona, provider, name, extra), st.start_headers(idempotency_key)
        started = await self._http.request("POST", "/api/browsers/start", body, START_TIMEOUT_S, headers=headers)
        if started.get("status") == "starting":
            await self._wait(started["id"], ready_timeout_s)
            started["cdpUrl"] = (await self._record(started["id"])).get("cdpUrl")
        return AsyncBrowser(self._http, started)

    async def _record(self, browser_id: str) -> dict[str, Any]:
        result: dict[str, Any] = await self._http.request("GET", f"/api/browsers/{segment(browser_id)}")
        return result

    async def _wait(self, browser_id: str, timeout_s: float) -> None:
        loop = asyncio.get_running_loop()
        deadline = loop.time() + timeout_s
        while not st.is_up(await self.list(), browser_id):
            try:
                st.raise_if_ended(await self._http.request("GET", f"/api/control/sessions/{segment(browser_id)}"))
            except OyaError as e:
                if not st.not_recorded(e):
                    raise
            if loop.time() >= deadline:
                raise st.not_ready(browser_id, timeout_s)
            await asyncio.sleep(min(READY_POLL_S, max(0.0, deadline - loop.time())))

    async def get(self, browser_id: str) -> AsyncBrowser:
        """Reattaches to a browser that is already running."""
        return AsyncBrowser(self._http, {**(await self._record(browser_id)), "reused": False})

    async def list(self) -> builtins.list[dict[str, Any]]:
        """Every browser on this key."""
        result: builtins.list[dict[str, Any]] = await self._http.request("GET", "/api/browsers")
        return result

    async def stop(self, ids: builtins.list[str]) -> dict[str, Any]:
        """Stops these browsers; each reports separately."""
        result: dict[str, Any] = await self._http.request("POST", "/api/browsers/stop", {"ids": ids}, START_TIMEOUT_S)
        return result

    async def stop_all(self) -> int:
        """Stops every browser on this key; returns how many stopped."""
        result = await self._http.request("POST", "/api/browsers/stop", {"all": True}, START_TIMEOUT_S)
        return int(result.get("stopped", 0))


class AsyncPlaybooks:
    """oya.playbooks, with asyncio."""

    def __init__(self, http: AsyncHttp) -> None:
        self._http = http

    async def list(self) -> builtins.list[dict[str, Any]]:
        """Every playbook on this key."""
        result: builtins.list[dict[str, Any]] = (await self._http.request("GET", "/api/playbooks")).get("playbooks", [])
        return result

    async def get(self, name: str) -> dict[str, Any]:
        """One playbook, with its steps, variables and Playwright code."""
        result: dict[str, Any] = await self._http.request("GET", _playbook(name))
        return result

    async def export(self, name: str) -> dict[str, Any]:
        """A playbook as a portable document, to import on another key or keep."""
        result: dict[str, Any] = await self._http.request("GET", f"{_playbook(name)}/export")
        return result

    async def remove(self, name: str) -> None:
        """Deletes a playbook."""
        await self._http.request("DELETE", _playbook(name))


class AsyncOya:
    """The asyncio client. Use it as an async context manager, or await aclose()."""

    def __init__(
        self, api_key: str | None = None, base_url: str | None = None, client: httpx.AsyncClient | None = None
    ):
        origin = resolve(api_key, base_url)
        self._http = AsyncHttp(origin, client)
        self.browser = AsyncBrowsers(self._http)
        self.playbooks = AsyncPlaybooks(self._http)

    async def aclose(self) -> None:
        """Closes the connection pool; running browsers keep running until stopped."""
        await self._http.aclose()

    async def __aenter__(self) -> "AsyncOya":
        return self

    async def __aexit__(self, *exc: object) -> None:
        await self.aclose()
