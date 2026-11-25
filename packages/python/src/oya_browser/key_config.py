"""oya.config: this key's settings (LLM credentials, browser provider, solver), as in api/config.ts.

On AsyncOya every method returns an awaitable.
"""

from typing import Any

from ._namespace import Namespace
from ._ops import Call


class KeyConfig(Namespace):
    """oya.config: read and change this key's settings."""

    def get(self) -> Any:
        """The settings on this key. Secrets read back masked."""
        return self._do(Call("GET", "/api/config"))

    def set(self, values: dict[str, Any]) -> Any:
        """Changes settings; None clears a field back to the deployment default."""
        return self._do(Call("POST", "/api/config", values))
