"""oya.playbooks (saved with browser.to_playbook(), and the drafts healed replays leave) and oya.llm.

Same paths and bodies as the TypeScript SDK's api/playbooks.ts. On AsyncOya every method returns an awaitable.
"""

from typing import Any

from ._http import segment
from ._namespace import Namespace
from ._ops import Call


def _playbook(name: str, action: str = "") -> str:
    return f"/api/playbooks/{segment(name, 'name')}{action}"


class Playbooks(Namespace):
    """oya.playbooks: list, read, promote a healed draft, and move playbooks between environments."""

    def list(self) -> Any:
        """Every saved playbook, with any draft waiting on it."""
        return self._do(Call("GET", "/api/playbooks"), "playbooks")

    def get(self, name: str) -> Any:
        """One playbook, with its steps, variables and Playwright code."""
        return self._do(Call("GET", _playbook(name)))

    def remove(self, name: str) -> Any:
        """Deletes a playbook and its draft, or only the draft with "<name>:draft"."""
        return self._do(Call("DELETE", _playbook(name)))

    def promote(self, name: str) -> Any:
        """Replaces a playbook with the draft a healed replay saved; try it first with play("<name>:draft")."""
        return self._do(Call("POST", _playbook(name, "/promote"), {}))

    def export(self, name: str) -> Any:
        """The playbook as one JSON document, for import_() elsewhere. Secrets travel by name only."""
        return self._do(Call("GET", _playbook(name, "/export")))

    def import_(self, doc: dict[str, Any], name: str | None = None, overwrite: bool | None = None) -> Any:
        """Saves an exported playbook here, as `name` or its exported name; a taken name needs overwrite=True."""
        options = {k: v for k, v in {"name": name, "overwrite": overwrite}.items() if v is not None}
        return self._do(Call("POST", "/api/playbooks/import", {"playbook": doc, **options}))


class Llm(Namespace):
    """oya.llm: this key's model for one-off text."""

    def answer(self, question: str, values: dict[str, Any] | None = None, task: str | None = None) -> Any:
        """The text for one free-text field, which an exported playbook calls for a comment or an answer."""
        body: dict[str, Any] = {"question": question, "values": values or {}}
        if task is not None:
            body["task"] = task
        return self._do(Call("POST", "/api/playbooks/answer", body), "answer")
