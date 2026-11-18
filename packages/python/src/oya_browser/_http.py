"""The one HTTP path every call takes, sync and async: the same headers, timeouts and error handling for both.

Errors come back as OyaError with the server's message, so a caller reads "Browser abc not found", not a status code.
"""

from typing import Any
from urllib.parse import quote

import httpx

from ._config import Origin
from ._constants import DEFAULT_TIMEOUT_S, STATUS_BAD_REQUEST_FLOOR
from .errors import OyaError

USER_AGENT = "oya-browser-python"


def segment(value: object, field: str = "id") -> str:
    """One path segment, encoded; refused when it is empty or would change the path ("/", "." or "..")."""
    if not isinstance(value, str) or not value:
        raise ValueError(f"{field} must be a non-empty string, not {value!r}")
    if "/" in value or value in (".", ".."):
        raise ValueError(f"{field} must be a single path segment, not {value!r}")
    return quote(value, safe="")


def _headers(origin: Origin, extra: dict[str, str] | None) -> dict[str, str]:
    """The key, the client name, and any per-call headers."""
    return {"Authorization": f"Bearer {origin.api_key}", "User-Agent": USER_AGENT, **(extra or {})}


def _parsed(response: httpx.Response, call: str) -> Any:
    """The JSON body, or an OyaError carrying the server's message when the call failed."""
    try:
        body: Any = response.json() if response.content else None
    except ValueError:
        body = response.text
    if response.status_code >= STATUS_BAD_REQUEST_FLOOR:
        message = body.get("error") if isinstance(body, dict) else None
        raise OyaError(message or f"{call} failed ({response.status_code})", response.status_code, body)
    return body


class Http:
    """Synchronous calls to the control plane."""

    def __init__(self, origin: Origin, client: httpx.Client | None = None, headers: dict[str, str] | None = None):
        self.origin = origin
        self._client = client or httpx.Client(base_url=origin.base_url)
        self._extra = headers

    def with_headers(self, headers: dict[str, str]) -> "Http":
        """The same client sending `headers` on every call too."""
        return Http(self.origin, self._client, {**(self._extra or {}), **headers})

    def request(self, method: str, path: str, body: Any = None, timeout: float = DEFAULT_TIMEOUT_S, **kw: Any) -> Any:
        """One call; returns the parsed JSON body."""
        headers = _headers(self.origin, {**(self._extra or {}), **kw.get("headers", {})})
        response = self._client.request(method, path, json=body, headers=headers, timeout=timeout)
        return _parsed(response, f"{method} {path}")

    def close(self) -> None:
        """Closes the connection pool."""
        self._client.close()


class AsyncHttp:
    """Asynchronous calls to the control plane."""

    def __init__(self, origin: Origin, client: httpx.AsyncClient | None = None, headers: dict[str, str] | None = None):
        self.origin = origin
        self._client = client or httpx.AsyncClient(base_url=origin.base_url)
        self._extra = headers

    def with_headers(self, headers: dict[str, str]) -> "AsyncHttp":
        """The same client sending `headers` on every call too."""
        return AsyncHttp(self.origin, self._client, {**(self._extra or {}), **headers})

    async def request(
        self, method: str, path: str, body: Any = None, timeout: float = DEFAULT_TIMEOUT_S, **kw: Any
    ) -> Any:
        """One call; returns the parsed JSON body."""
        headers = _headers(self.origin, {**(self._extra or {}), **kw.get("headers", {})})
        response = await self._client.request(method, path, json=body, headers=headers, timeout=timeout)
        return _parsed(response, f"{method} {path}")

    async def aclose(self) -> None:
        """Closes the connection pool."""
        await self._client.aclose()
