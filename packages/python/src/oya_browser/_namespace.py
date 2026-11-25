"""The base of every namespace whose methods are one call each (oya.personas, oya.control, ...).

One class serves both clients: on Oya a method returns the answer, on AsyncOya the same method returns an awaitable
of it. Each call is written once, so the sync and async surfaces cannot drift apart.
"""

from typing import Any

from ._http import AsyncHttp, Http
from ._ops import Call


def _picked(result: Any, pick: str | None) -> Any:
    """The answer, or the one field of it a list endpoint wraps its items in."""
    return result.get(pick) if pick and isinstance(result, dict) else result


class Namespace:
    """One call per method, sync or async depending on the client that built it."""

    def __init__(self, http: Http | AsyncHttp) -> None:
        self._http = http

    @property
    def _is_async(self) -> bool:
        return isinstance(self._http, AsyncHttp)

    def _do(self, call: Call, pick: str | None = None) -> Any:
        """Makes the call; on AsyncOya returns a coroutine to await."""
        if isinstance(self._http, AsyncHttp):
            return self._ado(self._http, call, pick)
        return _picked(self._http.request(call.method, call.path, call.body, call.timeout, headers=call.headers), pick)

    @staticmethod
    async def _ado(http: AsyncHttp, call: Call, pick: str | None) -> Any:
        return _picked(await http.request(call.method, call.path, call.body, call.timeout, headers=call.headers), pick)
