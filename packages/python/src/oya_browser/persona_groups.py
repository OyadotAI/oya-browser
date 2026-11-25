"""oya.persona_groups (alias oya.profile_groups): named sets of personas a start spreads its browsers across.

As in the TypeScript SDK's api/persona-groups.ts. On AsyncOya every method returns an awaitable.
"""

import builtins
from typing import Any

from ._http import segment
from ._namespace import Namespace
from ._ops import Call


def _group(name: str) -> str:
    return f"/api/persona-groups/{segment(name, 'name')}"


class PersonaGroups(Namespace):
    """oya.persona_groups: list, create, update and remove groups."""

    def list(self) -> Any:
        """Every group on this key."""
        return self._do(Call("GET", "/api/persona-groups"), "groups")

    def create(self, name: str, persona_ids: builtins.list[str] | None = None) -> Any:
        """A new group; its name is what a start passes, so it cannot change later, and a taken one is a 409."""
        return self._do(Call("POST", "/api/persona-groups", {"name": name, "personaIds": persona_ids or []}))

    def update(self, name: str, persona_ids: builtins.list[str]) -> Any:
        """Replaces the group's members."""
        return self._do(Call("PUT", _group(name), {"personaIds": persona_ids}))

    def remove(self, name: str) -> Any:
        """Deletes the group; its personas and their browsers are not touched."""
        return self._do(Call("DELETE", _group(name)))
