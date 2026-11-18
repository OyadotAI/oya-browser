"""oya.proxies: proxy exits for personas, as in the TypeScript SDK's api/proxies.ts.

A persona takes one at first connect (by its geo hint) or by personas.pin_proxy(), and keeps it.
On AsyncOya every method returns an awaitable.
"""

from typing import Any

from ._http import segment
from ._namespace import Namespace
from ._ops import Call


class Proxies(Namespace):
    """oya.proxies: list, add, remove and check proxies."""

    def list(self) -> Any:
        """Every proxy this key can use, shared ones included."""
        return self._do(Call("GET", "/api/proxies"), "proxies")

    def create(self, proxy: dict[str, Any]) -> Any:
        """Adds a proxy. Its credentials are never read back."""
        return self._do(Call("POST", "/api/proxies", proxy))

    def remove(self, proxy_id: str) -> Any:
        """Removes one of this key's proxies."""
        return self._do(Call("DELETE", f"/api/proxies/{segment(proxy_id)}"))

    def check(self) -> Any:
        """Dials each proxy to learn its real exit IP; failing ones cool down and are skipped."""
        return self._do(Call("POST", "/api/proxies/check", {}), "results")
