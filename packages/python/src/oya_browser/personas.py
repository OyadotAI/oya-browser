"""oya.personas (alias oya.profiles): identities, as in the TypeScript SDK's api/personas.ts.

Each persona is one device with its cookie jar and proxy, plus the second factors and site logins stored for it.

Every persona id goes into a path through segment(), which refuses one that would change the path, before any
request. On AsyncOya every method returns an awaitable.
"""

import builtins
from typing import Any
from urllib.parse import quote

from ._http import segment
from ._namespace import Namespace
from ._ops import Call


def _persona(persona_id: str, action: str = "") -> str:
    return f"/api/personas/{segment(persona_id)}{action}"


def _domain(domain: str) -> str:
    return f"?domain={quote(domain, safe='')}"


def _jar(persona_id: str) -> str:
    return f"/api/pool/cookies?persona={segment(persona_id)}"


class Personas(Namespace):
    """oya.personas: find, create and change identities, their second factors, site logins and cookies."""

    def list(self) -> Any:
        """Every persona on this key."""
        return self._do(Call("GET", "/api/personas"), "personas")

    def get(self, persona_id: str) -> Any:
        """One persona."""
        return self._do(Call("GET", _persona(persona_id)))

    def create(self, options: dict[str, Any] | None = None) -> Any:
        """A new identity; its device is chosen here and fixed for life (preview() shows what a choice makes)."""
        return self._do(Call("POST", "/api/personas", options or {}))

    def update(self, persona_id: str, changes: dict[str, Any]) -> Any:
        """Name, concurrency cap and proxy hint. Never the device: clone for that."""
        return self._do(Call("PUT", _persona(persona_id), changes))

    def clone(self, persona_id: str, options: dict[str, Any] | None = None) -> Any:
        """A new persona of the same kind of device: same choices, fresh identity, empty jar."""
        return self._do(Call("POST", _persona(persona_id, "/clone"), options or {}))

    def preview(self, prefs: dict[str, Any] | None = None) -> Any:
        """The fingerprint these choices would produce. Persists nothing."""
        return self._do(Call("POST", "/api/personas/preview", {"prefs": prefs or {}}), "fingerprint")

    def options(self) -> Any:
        """Platforms, and the timezones and locales each may coherently claim."""
        return self._do(Call("GET", "/api/personas/options"))

    def pin_proxy(self, persona_id: str, proxy_id: str | None) -> Any:
        """Pins the persona to one of your proxies, or None to let assignment happen at connect."""
        return self._do(Call("PUT", _persona(persona_id, "/proxy"), {"proxyId": proxy_id}))

    def remove(self, persona_id: str) -> Any:
        """Deletes a persona."""
        return self._do(Call("DELETE", _persona(persona_id)))

    def set_mfa(self, persona_id: str, config: dict[str, Any]) -> Any:
        """Stores the second factor for this identity. Sealed at rest, never read back."""
        return self._do(Call("PUT", _persona(persona_id, "/mfa"), config))

    def clear_mfa(self, persona_id: str, domain: str | None = None) -> Any:
        """Removes the persona-wide factor, or the one filed against `domain`."""
        return self._do(Call("DELETE", _persona(persona_id, "/mfa") + (_domain(domain) if domain else "")))

    def set_credentials(self, persona_id: str, config: dict[str, Any]) -> Any:
        """Stores a site login, for portals that expire sessions server-side. Sealed at rest, never read back."""
        return self._do(Call("PUT", _persona(persona_id, "/credentials"), config))

    def credentials(self, persona_id: str) -> Any:
        """Which sites this identity can sign in to. Usernames only."""
        return self._do(Call("GET", _persona(persona_id, "/credentials")))

    def clear_credentials(self, persona_id: str, domain: str) -> Any:
        """Removes the login stored for one site."""
        return self._do(Call("DELETE", _persona(persona_id, "/credentials") + _domain(domain)))

    def cookies(self, persona_id: str, format: str = "json") -> Any:
        """Every cookie in the jar; format="playwright" is ready for context.add_cookies()."""
        return self._do(Call("GET", f"{_jar(persona_id)}&format={quote(format, safe='')}"), "cookies")

    def import_cookies(self, persona_id: str, cookies: builtins.list[dict[str, Any]]) -> Any:
        """Merges cookies into the jar; the persona's browsers pick them up on their next visit to each site."""
        return self._do(Call("PUT", _jar(persona_id), {"cookies": cookies}))

    def copy_cookies(self, source: str, target: str) -> Any:
        """Copies every login of persona `source` into persona `target`, which keeps its own device."""
        if self._is_async:
            return self._acopy(source, target)
        return self.import_cookies(target, self.cookies(source))

    async def _acopy(self, source: str, target: str) -> Any:
        return await self.import_cookies(target, await self.cookies(source))

    def clear_cookies(self, persona_id: str) -> Any:
        """Forgets every cookie in the jar: signs the persona out everywhere."""
        return self._do(Call("DELETE", _jar(persona_id)))
