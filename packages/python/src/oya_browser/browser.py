"""One running browser, sync (Browser) and async (AsyncBrowser): commands, agent tasks, playbooks, CDP for other tools.

The same calls as the TypeScript SDK's Browser. `for_browser_use()` hands the browser to browser-use, which drives
it over CDP; each Oya CDP URL carries a single-use ticket, so every hand-off asks the server for a fresh one.
"""

from typing import Any

from . import _ops as ops
from ._constants import WAIT_FOR_DEFAULT_S
from ._http import AsyncHttp, Http, segment

NO_CDP = "This browser has no CDP endpoint (its provider does not offer one), so browser-use cannot drive it."


def _browser_use(cdp_url: str | None) -> Any:
    """A browser_use.Browser on `cdp_url`; browser-use is an optional extra, imported only when asked for."""
    if not cdp_url:
        raise ValueError(NO_CDP)
    try:
        from browser_use import Browser as BrowserUseBrowser
    except ImportError as e:
        raise ImportError('browser-use is not installed: pip install "oya-browser[browser-use]"') from e
    return BrowserUseBrowser(cdp_url=cdp_url)


class _Base:
    """What every browser knows about itself."""

    def __init__(self, info: dict[str, Any]) -> None:
        self.id: str = info["id"]
        self.provider: str = info.get("provider") or "cdp"
        self.persona: str = info.get("persona") or "default"
        self.cdp_url: str | None = info.get("cdpUrl")
        """Where Playwright, Puppeteer or browser-use connect; single use, see fresh_cdp_url()."""
        self.reused: bool = info.get("reused") is True
        """True when start() handed over a browser that was already running, such as the desktop app."""

    def __repr__(self) -> str:
        return f"<{type(self).__name__} {self.id} on {self.provider}>"


class Browser(_Base):
    """One running browser, driven synchronously."""

    def __init__(self, http: Http, info: dict[str, Any]) -> None:
        super().__init__(info)
        self._http = http.with_headers({ops.LANE_HEADER: ops.sdk_lane()}) if self.reused else http

    def _call(self, call: ops.Call) -> Any:
        return self._http.request(call.method, call.path, call.body, call.timeout, headers=call.headers)

    def _command(self, call: ops.Call, action: str) -> Any:
        return ops.command_data(self._call(call), action)

    def goto(self, url: str) -> None:
        """Opens `url` and waits for it to load."""
        self._command(ops.navigate(self.id, url), "navigate")

    def analyze(self, format: str | None = None) -> dict[str, Any]:
        """The page as TOON (default), markdown or JSONL, with its numbered elements to act on."""
        params = {"format": format} if format else {}
        result: dict[str, Any] = self._command(ops.command(self.id, "analyze", params), "analyze")
        return result

    def elements(self) -> list[dict[str, Any]]:
        """Only the visible elements, which is what an agent almost always wants."""
        return [e for e in self.analyze().get("elements", []) if e.get("visible")]

    def click(self, element_id: int | str) -> None:
        """Clicks an element by its id from analyze()."""
        self._command(ops.command(self.id, "click", ops.element(element_id)), "click")

    def type(self, element_id: int | str, text: str) -> Any:
        """Types into an element by its id from analyze()."""
        return self._command(ops.command(self.id, "type", {**ops.element(element_id), "text": text}), "type")

    def press_key(self, key: str) -> None:
        """Presses one key, such as Enter or Escape."""
        self._command(ops.command(self.id, "press_key", {"key": key}), "press_key")

    def scroll(self, direction: str, amount: int | None = None) -> None:
        """Scrolls "up", "down", "top" or "bottom"."""
        self._command(ops.command(self.id, "scroll", ops.scroll(direction, amount)), "scroll")

    def wait_for(self, selector: str, timeout_s: float = WAIT_FOR_DEFAULT_S) -> None:
        """Waits until `selector` matches."""
        self._command(ops.wait_for(self.id, selector, timeout_s), "wait")

    def screenshot(self) -> str:
        """A data:image/...;base64, URL of the page."""
        return str(self._command(ops.command(self.id, "screenshot", {}), "screenshot")["screenshot"])

    def solve_captcha(self) -> dict[str, Any]:
        """Detects and solves a CAPTCHA on the page."""
        result: dict[str, Any] = self._call(ops.challenge(self.id, "captcha"))
        return result

    def complete_mfa(self) -> dict[str, Any]:
        """Completes an MFA prompt with the persona's stored factor; otherwise returns a link for a person."""
        result: dict[str, Any] = self._call(ops.challenge(self.id, "mfa"))
        return result

    def ask(self, prompt: str, data: dict[str, Any] | None = None, secrets: dict[str, str] | None = None) -> str:
        """A task in plain language, on this key's model. `secrets` are typed for the agent but never shown to it."""
        return str(ops.agent_answer(self._call(ops.ask(self.id, prompt, data, secrets))).get("text", ""))

    def extract(self, prompt: str, schema: dict[str, Any], data: dict[str, Any] | None = None) -> Any:
        """ask() for data: the answer comes back in the shape of the JSON `schema`."""
        return ops.agent_answer(self._call(ops.ask(self.id, prompt, data, None, schema))).get("data")

    def to_playbook(self, name: str) -> dict[str, Any]:
        """Saves the last ask() as a playbook: Playwright code with the values as variables."""
        result: dict[str, Any] = self._call(ops.to_playbook(self.id, name))
        return result

    def play(self, name: str, data: dict[str, Any] | None = None, auto_heal: bool = True) -> dict[str, Any]:
        """Replays a playbook with no model; with auto_heal, the agent finishes a step a page changed."""
        return ops.agent_answer(self._call(ops.play(self.id, name, data, auto_heal)))

    def status(self) -> dict[str, Any]:
        """The browser's record, with a fresh CDP URL when it offers one."""
        result: dict[str, Any] = self._http.request("GET", f"/api/browsers/{segment(self.id)}")
        return result

    def fresh_cdp_url(self) -> str | None:
        """A new single-use CDP URL, for each tool that connects."""
        return self.status().get("cdpUrl")

    def for_browser_use(self) -> Any:
        """A browser_use.Browser on this Oya browser: pass it as Agent(task, llm, browser=...)."""
        return _browser_use(self.fresh_cdp_url())

    def stop(self) -> dict[str, Any]:
        """Stops it; a borrowed desktop browser only closes this SDK's own tab."""
        result: dict[str, Any] = self._call(ops.stop(self.id, self.reused))
        return result

    def __enter__(self) -> "Browser":
        return self

    def __exit__(self, *exc: object) -> None:
        self.stop()


class AsyncBrowser(_Base):
    """One running browser, driven with asyncio."""

    def __init__(self, http: AsyncHttp, info: dict[str, Any]) -> None:
        super().__init__(info)
        self._http = http.with_headers({ops.LANE_HEADER: ops.sdk_lane()}) if self.reused else http

    async def _call(self, call: ops.Call) -> Any:
        return await self._http.request(call.method, call.path, call.body, call.timeout, headers=call.headers)

    async def _command(self, call: ops.Call, action: str) -> Any:
        return ops.command_data(await self._call(call), action)

    async def goto(self, url: str) -> None:
        """Opens `url` and waits for it to load."""
        await self._command(ops.navigate(self.id, url), "navigate")

    async def analyze(self, format: str | None = None) -> dict[str, Any]:
        """The page as TOON (default), markdown or JSONL, with its numbered elements to act on."""
        params = {"format": format} if format else {}
        result: dict[str, Any] = await self._command(ops.command(self.id, "analyze", params), "analyze")
        return result

    async def elements(self) -> list[dict[str, Any]]:
        """Only the visible elements, which is what an agent almost always wants."""
        return [e for e in (await self.analyze()).get("elements", []) if e.get("visible")]

    async def click(self, element_id: int | str) -> None:
        """Clicks an element by its id from analyze()."""
        await self._command(ops.command(self.id, "click", ops.element(element_id)), "click")

    async def type(self, element_id: int | str, text: str) -> Any:
        """Types into an element by its id from analyze()."""
        return await self._command(ops.command(self.id, "type", {**ops.element(element_id), "text": text}), "type")

    async def press_key(self, key: str) -> None:
        """Presses one key, such as Enter or Escape."""
        await self._command(ops.command(self.id, "press_key", {"key": key}), "press_key")

    async def scroll(self, direction: str, amount: int | None = None) -> None:
        """Scrolls "up", "down", "top" or "bottom"."""
        await self._command(ops.command(self.id, "scroll", ops.scroll(direction, amount)), "scroll")

    async def wait_for(self, selector: str, timeout_s: float = WAIT_FOR_DEFAULT_S) -> None:
        """Waits until `selector` matches."""
        await self._command(ops.wait_for(self.id, selector, timeout_s), "wait")

    async def screenshot(self) -> str:
        """A data:image/...;base64, URL of the page."""
        return str((await self._command(ops.command(self.id, "screenshot", {}), "screenshot"))["screenshot"])

    async def solve_captcha(self) -> dict[str, Any]:
        """Detects and solves a CAPTCHA on the page."""
        result: dict[str, Any] = await self._call(ops.challenge(self.id, "captcha"))
        return result

    async def complete_mfa(self) -> dict[str, Any]:
        """Completes an MFA prompt with the persona's stored factor; otherwise returns a link for a person."""
        result: dict[str, Any] = await self._call(ops.challenge(self.id, "mfa"))
        return result

    async def ask(self, prompt: str, data: dict[str, Any] | None = None, secrets: dict[str, str] | None = None) -> str:
        """A task in plain language, on this key's model. `secrets` are typed for the agent but never shown to it."""
        return str(ops.agent_answer(await self._call(ops.ask(self.id, prompt, data, secrets))).get("text", ""))

    async def extract(self, prompt: str, schema: dict[str, Any], data: dict[str, Any] | None = None) -> Any:
        """ask() for data: the answer comes back in the shape of the JSON `schema`."""
        return ops.agent_answer(await self._call(ops.ask(self.id, prompt, data, None, schema))).get("data")

    async def to_playbook(self, name: str) -> dict[str, Any]:
        """Saves the last ask() as a playbook: Playwright code with the values as variables."""
        result: dict[str, Any] = await self._call(ops.to_playbook(self.id, name))
        return result

    async def play(self, name: str, data: dict[str, Any] | None = None, auto_heal: bool = True) -> dict[str, Any]:
        """Replays a playbook with no model; with auto_heal, the agent finishes a step a page changed."""
        return ops.agent_answer(await self._call(ops.play(self.id, name, data, auto_heal)))

    async def status(self) -> dict[str, Any]:
        """The browser's record, with a fresh CDP URL when it offers one."""
        result: dict[str, Any] = await self._http.request("GET", f"/api/browsers/{segment(self.id)}")
        return result

    async def fresh_cdp_url(self) -> str | None:
        """A new single-use CDP URL, for each tool that connects."""
        return (await self.status()).get("cdpUrl")

    async def for_browser_use(self) -> Any:
        """A browser_use.Browser on this Oya browser: pass it as Agent(task, llm, browser=...)."""
        return _browser_use(await self.fresh_cdp_url())

    async def stop(self) -> dict[str, Any]:
        """Stops it; a borrowed desktop browser only closes this SDK's own tab."""
        result: dict[str, Any] = await self._call(ops.stop(self.id, self.reused))
        return result

    async def __aenter__(self) -> "AsyncBrowser":
        return self

    async def __aexit__(self, *exc: object) -> None:
        await self.stop()
