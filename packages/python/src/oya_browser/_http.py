"""The one HTTP path every call takes, sync and async: the same headers, timeouts and error handling for both.

Errors come back as OyaError with the server's message, so a caller reads "Browser abc not found", not a status code.
"""

from typing import Any, TypedDict
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


class ModelOptions(TypedDict, total=False):
    """The model behind ask(), extract(), runs and healed replays, for this client only; never saved to the key."""

    provider: str
    """openai, anthropic, gemini, vertex, chatgpt or openrouter."""
    name: str
    """The model's name, e.g. claude-sonnet-5."""
    api_key: str
    """Your own key for that provider; left out, the key's saved credential is used, for the same provider."""


#: Each model option and the header the server reads it from (server/src/modules/config/request-llm.ts).
_MODEL_HEADERS = (("provider", "X-Oya-Llm-Provider"), ("name", "X-Oya-Llm-Model"), ("api_key", "X-Oya-Llm-Api-Key"))

#: The lowest printable ASCII character; tab is the one control character a header may carry.
_SPACE, _TILDE, _TAB = 0x20, 0x7E, 0x09


def model_headers(model: ModelOptions | None) -> dict[str, str]:
    """The model option as the headers the server reads it from on every call.

    Headers, so background runs and heals started by any call carry it too. A value httpx cannot send is refused
    here, by position, never echoed.
    """
    out: dict[str, str] = {}
    for field, header in _MODEL_HEADERS:
        value = (model or {}).get(field)
        if value:
            out[header] = _header_safe(str(value).strip(), f"model.{field}")
    return out


def _header_safe(value: str, field: str) -> str:
    """`value`, or a ValueError naming the field and the first character a header cannot carry."""
    for i, ch in enumerate(value):
        code = ord(ch)
        if not (_SPACE <= code <= _TILDE or code == _TAB):
            raise ValueError(f"{field} has a character HTTP headers cannot carry at position {i + 1} (U+{code:04X}).")
    return value


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
