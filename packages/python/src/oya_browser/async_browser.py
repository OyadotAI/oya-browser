"""One running browser, driven with asyncio: the same calls as Browser (see browser.py), awaited."""

from typing import Any

from . import _ops as ops
from ._browser_base import BrowserBase, browser_use
from ._constants import RUN_POLL_S, SHARE_EXPIRES_S, WAIT_FOR_DEFAULT_S
from ._http import AsyncHttp, segment
from ._run_watch import Callback, Callbacks
from .run import AsyncRun


class AsyncBrowser(BrowserBase):
    """One running browser, driven with asyncio. Element ids from analyze() are valid until the page changes."""

    def __init__(self, http: AsyncHttp, info: dict[str, Any], auto_captcha: bool = False) -> None:
        super().__init__(http, info, auto_captcha)
        self._http = http.with_headers({ops.LANE_HEADER: ops.sdk_lane()}) if self.reused else http

    async def _call(self, call: ops.Call) -> Any:
        return await self._http.request(call.method, call.path, call.body, call.timeout, headers=call.headers)

    async def _command(self, call: ops.Call, action: str) -> Any:
        return ops.command_data(await self._call(call), action)

    async def _do(self, action: str, params: dict[str, Any] | None = None) -> Any:
        return await self._command(ops.command(self.id, action, params or {}), action)

    async def goto(self, url: str) -> None:
        """Opens `url` and waits for it to load; with start(captcha="auto"), then clears any CAPTCHA or raises 409."""
        await self._command(ops.navigate(self.id, url), "navigate")
        if self._auto_captcha:
            ops.assert_captcha_cleared(await self.solve_captcha())

    async def analyze(self, format: str | None = None) -> dict[str, Any]:
        """The page as TOON (default), markdown or JSONL, with its numbered elements to act on."""
        result: dict[str, Any] = await self._do("analyze", {"format": format} if format else {})
        return result

    async def elements(self) -> list[dict[str, Any]]:
        """Only the visible elements, which is what an agent almost always wants."""
        return [e for e in (await self.analyze()).get("elements", []) if e.get("visible")]

    async def click(self, element_id: int | str) -> None:
        """Clicks an element by its id from analyze()."""
        await self._do("click", ops.element(element_id))

    async def type(self, element_id: int | str, text: str) -> Any:
        """Types into an element by its id from analyze()."""
        return await self._do("type", {**ops.element(element_id), "text": text})

    async def press_key(self, key: str) -> None:
        """Presses one key, such as Enter or Escape."""
        await self._do("press_key", {"key": key})

    async def handle_dialog(self, accept: bool, prompt_text: str | None = None) -> dict[str, Any]:
        """Answers a confirm() or prompt() holding the page; until then every other command fails with its text."""
        result: dict[str, Any] = await self._command(ops.handle_dialog(self.id, accept, prompt_text), "handle_dialog")
        return result

    async def scroll(self, direction: str, amount: int | None = None, at: tuple[int, int] | None = None) -> None:
        """Scrolls "up", "down", "top" or "bottom"; `at` (x, y) aims at an inner scroller instead of the page."""
        await self._do("scroll", ops.scroll(direction, amount, at))

    async def wait_for(self, selector: str, timeout_s: float = WAIT_FOR_DEFAULT_S) -> None:
        """Waits until `selector` matches."""
        await self._command(ops.wait_for(self.id, selector, timeout_s), "wait")

    async def screenshot(self) -> str:
        """A data:image/...;base64, URL of the page."""
        return str((await self._do("screenshot"))["screenshot"])

    async def url(self) -> str:
        """The active tab's URL, or empty when there is none."""
        return ops.active_url(await self.tabs())

    async def tabs(self) -> list[dict[str, Any]]:
        """Every open tab: id, url, title and whether it is the active one."""
        return list(((await self._do("list_tabs")) or {}).get("tabs") or [])

    async def open_tab(self, url: str | None = None) -> str | int:
        """Opens a tab, at `url` when given, and returns its id, as the driver gives it (a number on Oya Cloud)."""
        tab_id: str | int = (await self._command(ops.open_tab(self.id, url), "open_tab"))["tab_id"]
        return tab_id

    async def switch_tab(self, tab_id: str | int) -> None:
        """Makes a tab the one commands act on."""
        await self._do("switch_tab", {"tab_id": tab_id})

    async def close_tab(self, tab_id: str | int) -> None:
        """Closes a tab."""
        await self._do("close_tab", {"tab_id": tab_id})

    async def solve_captcha(self) -> dict[str, Any]:
        """Detects and solves a CAPTCHA on the page."""
        result: dict[str, Any] = await self._call(ops.challenge(self.id, "captcha"))
        return result

    async def complete_mfa(self) -> dict[str, Any]:
        """Completes an MFA prompt with the persona's stored factor; otherwise returns a link for a person."""
        result: dict[str, Any] = await self._call(ops.challenge(self.id, "mfa"))
        return ops.mfa_result(self._base_url, result)

    async def ask(self, prompt: str, data: dict[str, Any] | None = None, secrets: dict[str, Any] | None = None) -> str:
        """A task in plain language, on this key's model. `secrets` are typed for the agent but never shown to it."""
        return str(ops.agent_answer(await self._call(ops.ask(self.id, prompt, data, secrets))).get("text", ""))

    async def extract(
        self,
        prompt: str,
        schema: dict[str, Any],
        data: dict[str, Any] | None = None,
        secrets: dict[str, Any] | None = None,
    ) -> Any:
        """ask() for data: the answer comes back in the shape of the JSON `schema`; raises 422 when it could not."""
        return ops.extract_data(await self._call(ops.ask(self.id, prompt, data, secrets, schema)))

    async def to_playbook(self, name: str) -> dict[str, Any]:
        """Saves the last ask() as a playbook: Playwright code with the values as variables."""
        result: dict[str, Any] = await self._call(ops.to_playbook(self.id, name))
        return result

    async def play(self, name: str, data: dict[str, Any] | None = None, auto_heal: bool = True) -> dict[str, Any]:
        """Replays a playbook with no model; with auto_heal, the agent finishes a step a page changed."""
        return ops.agent_answer(await self._call(ops.play(self.id, name, data, auto_heal)))

    async def submit(
        self,
        prompt: str | None = None,
        *,
        playbook: str | None = None,
        data: dict[str, Any] | None = None,
        secrets: dict[str, Any] | None = None,
        auto_heal: bool | None = None,
        on_success: Callback | None = None,
        on_failure: Callback | None = None,
        on_human_attention: Callback | None = None,
        on_healed: Callback | None = None,
        poll_s: float = RUN_POLL_S,
    ) -> AsyncRun:
        """Starts a prompt or a playbook as a background run on the server; polled in the background, firing callbacks.

        on_human_attention fires for an unsolved CAPTCHA, an unfinished MFA, the agent asking for help, or a replay
        it could not heal; the run waits (up to 30 minutes) until you call request["respond"]().
        """
        values = {"data": data, "secrets": secrets, "autoHeal": auto_heal}
        started = await self._call(ops.submit(self.id, prompt, playbook, values))
        cb = Callbacks(
            on_success=on_success, on_failure=on_failure, on_human_attention=on_human_attention, on_healed=on_healed
        )
        return AsyncRun(self._http, started["id"], cb, poll_s)

    async def live_stream_url(self) -> str:
        """The SSE stream of JPEG frames, for your own UI; its ticket is single use for 60s, so mint one per viewer."""
        ticket = (await self._call(ops.session(self.id, "ticket")))["ticket"]
        return ops.live_stream_url(self._base_url, self.id, str(ticket))

    async def share_url(self, control: bool = False, expires_in_s: int = SHARE_EXPIRES_S) -> dict[str, Any]:
        """A link to this browser's live view, scoped to it alone: {url, id, expiresAt}. Treat it like a password.

        control=True lets whoever opens it act in the browser; otherwise it is view-only. revoke_share(id) ends it.
        """
        return ops.share_link(self._base_url, self.id, await self._call(ops.share(self.id, control, expires_in_s)))

    async def revoke_share(self, share_id: str) -> None:
        """Revokes a link from share_url() before it expires, by the id it returned."""
        await self._http.request("DELETE", f"/api/control/credentials/{segment(share_id)}")

    async def status(self) -> dict[str, Any]:
        """The browser's record, with a fresh CDP URL when it offers one."""
        result: dict[str, Any] = await self._http.request("GET", f"/api/browsers/{segment(self.id)}")
        return result

    async def fresh_cdp_url(self) -> str | None:
        """A new single-use CDP URL, for each tool that connects."""
        return (await self.status()).get("cdpUrl")

    async def for_browser_use(self) -> Any:
        """A browser_use.Browser on this Oya browser: pass it as Agent(task, llm, browser=...)."""
        return browser_use(await self.fresh_cdp_url())

    async def stop(self) -> dict[str, Any]:
        """Stops it; a borrowed desktop browser only closes this SDK's own tab, best effort."""
        if not self.reused:
            result: dict[str, Any] = await self._call(ops.stop(self.id, False))
            return result
        try:
            await self._call(ops.stop(self.id, True))
        except Exception:
            pass
        return {"id": self.id, "ok": True, "reused": True}

    async def __aenter__(self) -> "AsyncBrowser":
        return self

    async def __aexit__(self, *exc: object) -> None:
        await self.stop()
