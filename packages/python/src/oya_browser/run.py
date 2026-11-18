"""A task submitted with browser.submit(), running on the server: Run (sync) and AsyncRun.

AsyncRun matches the TypeScript SDK's Run: it starts polling in the background as soon as it exists, fires the
callbacks as the run changes, and `done` (an asyncio.Task) settles with the result. A synchronous program has no
background to poll in without a thread, and a thread firing callbacks into code that does not expect it is a worse
surprise than a blocking call, so Run polls when you call wait(): it fires the same callbacks, in the same order,
and returns the result or raises the failure. on_human_attention runs inline there, so a handler may call
request["respond"]() (or run.respond()) before returning; polling resumes after it.
"""

import asyncio
import inspect
import time
from typing import Any

from . import _run_watch as rw
from ._constants import RUN_POLL_S
from ._http import AsyncHttp, Http
from .errors import OyaError


class Run:
    """A submitted task, polled when you call wait()."""

    def __init__(self, http: Http, run_id: str, callbacks: rw.Callbacks, poll_s: float = RUN_POLL_S) -> None:
        self.id = run_id
        self._http, self._cb, self._poll_s = http, callbacks, poll_s
        self._watch = rw.Watch(http.origin.base_url, self.respond)

    def status(self) -> dict[str, Any]:
        """The run's current state."""
        c = rw.status_call(self.id)
        result: dict[str, Any] = self._http.request(c.method, c.path)
        return result

    def respond(self, response: str = "done") -> None:
        """Answers the open attention request: "done" after handling it by hand, or your reply to the agent."""
        c = rw.respond_call(self.id, response)
        self._http.request(c.method, c.path, c.body)

    def wait(self) -> dict[str, Any]:
        """Polls until the run ends, firing the callbacks; returns the result or raises the OyaError it failed with."""
        while True:
            result = self._poll()
            if result is not None:
                return result
            time.sleep(self._poll_s)

    def _poll(self) -> dict[str, Any] | None:
        try:
            info = self.status()
        except Exception as e:
            failure = self._watch.failed_poll(e)
            return self._fail(failure) if failure else None
        step = self._watch.step(info)
        if step.attention:
            rw.fire(self._cb.on_human_attention, step.attention)
        if step.failure:
            self._fail(step.failure)
        if step.result is not None:
            self._succeed(step.result)
        return step.result

    def _succeed(self, result: dict[str, Any]) -> None:
        """Fires on_healed first when a replay was healed, then on_success."""
        if result.get("healed"):
            rw.fire(self._cb.on_healed, result)
        rw.fire(self._cb.on_success, result)

    def _fail(self, failure: OyaError) -> Any:
        rw.fire(self._cb.on_failure, failure)
        raise failure


async def _fire(callback: rw.Callback | None, value: Any) -> None:
    """Calls a callback that may be sync or async; one that raises is logged, never allowed to break the loop."""
    try:
        result = rw.fire(callback, value)
        if inspect.isawaitable(result):
            await result
    except Exception:
        rw.log.exception("run callback raised")


class AsyncRun:
    """A submitted task, polled in the background from the moment it exists; callbacks may be sync or async."""

    def __init__(self, http: AsyncHttp, run_id: str, callbacks: rw.Callbacks, poll_s: float = RUN_POLL_S) -> None:
        self.id = run_id
        self._http, self._cb, self._poll_s = http, callbacks, poll_s
        self._watch = rw.Watch(http.origin.base_url, self.respond)
        self._handlers: set[asyncio.Task[None]] = set()
        self.done: asyncio.Task[dict[str, Any]] = asyncio.ensure_future(self._loop())
        """Settles with the result when the run succeeds, or raises the OyaError it failed with."""
        # Callers may rely on on_failure alone, so a failure nobody awaits is not reported as unretrieved.
        self.done.add_done_callback(lambda t: t.cancelled() or t.exception())

    async def status(self) -> dict[str, Any]:
        """The run's current state."""
        c = rw.status_call(self.id)
        result: dict[str, Any] = await self._http.request(c.method, c.path)
        return result

    async def respond(self, response: str = "done") -> None:
        """Answers the open attention request: "done" after handling it by hand, or your reply to the agent."""
        c = rw.respond_call(self.id, response)
        await self._http.request(c.method, c.path, c.body)

    async def wait(self) -> dict[str, Any]:
        """The result once the run ends (the same as awaiting `done`)."""
        return await self.done

    async def _loop(self) -> dict[str, Any]:
        while True:
            result = await self._poll()
            if result is not None:
                return result
            await asyncio.sleep(self._poll_s)

    async def _poll(self) -> dict[str, Any] | None:
        try:
            info = await self.status()
        except Exception as e:
            failure = self._watch.failed_poll(e)
            return await self._fail(failure) if failure else None
        step = self._watch.step(info)
        if step.attention:
            self._attend(step.attention)
        if step.failure:
            await self._fail(step.failure)
        if step.result is not None:
            await self._succeed(step.result)
        return step.result

    def _attend(self, request: dict[str, Any]) -> None:
        """Not awaited: a handler that waits on a person must not stall polling."""
        task = asyncio.ensure_future(_fire(self._cb.on_human_attention, request))
        self._handlers.add(task)
        task.add_done_callback(self._handlers.discard)

    async def _succeed(self, result: dict[str, Any]) -> None:
        if result.get("healed"):
            await _fire(self._cb.on_healed, result)
        await _fire(self._cb.on_success, result)

    async def _fail(self, failure: OyaError) -> Any:
        await _fire(self._cb.on_failure, failure)
        raise failure
