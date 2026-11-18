"""oya.control: the durable control plane, as in the TypeScript SDK's api/control.ts.

Sessions (disconnected and cleanup-pending ones included), human takeover, recovery, lifecycle events, service
credentials, members and webhooks. On AsyncOya every method returns an awaitable.
"""

from typing import Any

from ._http import segment
from ._namespace import Namespace
from ._ops import Call


def _session(session_id: str, action: str = "") -> str:
    return f"/api/control/sessions/{segment(session_id)}{action}"


def _item(kind: str, item_id: str) -> str:
    return f"/api/control/{kind}/{segment(item_id)}"


def _recover_body(replace: bool, ws_url: str | None) -> dict[str, Any]:
    """Refuses ws_url without replace: recovering in place keeps the endpoint it had."""
    if ws_url and not replace:
        raise ValueError("ws_url is only used with replace=True; recovering in place keeps the endpoint it had.")
    return {"replace": replace, "wsUrl": ws_url} if ws_url else {"replace": replace}


class Control(Namespace):
    """oya.control: sessions, takeover, recovery, events, credentials, members and webhooks."""

    def overview(self) -> Any:
        """Settings, sessions and recent events at a glance."""
        return self._do(Call("GET", "/api/control"))

    def sessions(self) -> Any:
        """Every session, including disconnected and cleanup-pending ones."""
        return self._do(Call("GET", "/api/control/sessions"))

    def session(self, session_id: str) -> Any:
        """One session."""
        return self._do(Call("GET", _session(session_id)))

    def settings(self, changes: dict[str, Any]) -> Any:
        """Updates limits, rate cards and retention."""
        return self._do(Call("PATCH", "/api/control/project", changes))

    def cancel(self, session_id: str) -> Any:
        """Stops a session in any state; answers its final state."""
        return self._do(Call("POST", _session(session_id, "/cancel"), {}))

    def stop(self, session_id: str, force: bool = False) -> Any:
        """Stops a session; `force` stops despite a profile-save error, or reconciles."""
        return self._do(Call("POST", _session(session_id, "/stop"), {"force": force}))

    def takeover(self, session_id: str, action: str) -> Any:
        """Acquires ("acquire") or releases ("release") human control, or acknowledges the agent's "resume"."""
        return self._do(Call("POST", _session(session_id, "/control"), {"action": action}))

    def input(self, session_id: str, action: str, params: dict[str, Any]) -> Any:
        """Sends one input as the human holding the control lease."""
        return self._do(Call("POST", _session(session_id, "/input"), {"action": action, "params": params}))

    def recover(self, session_id: str, replace: bool = False, ws_url: str | None = None) -> Any:
        """Recovers a session, or replaces it with a fresh one; a cdp session's replacement needs its ws_url."""
        return self._do(Call("POST", _session(session_id, "/recover"), _recover_body(replace, ws_url)))

    def ticket(self, session_id: str) -> Any:
        """A single-use ticket for the live stream."""
        return self._do(Call("POST", _session(session_id, "/ticket"), {}))

    def events(self, after: int = 0) -> Any:
        """Durable lifecycle events after a cursor."""
        return self._do(Call("GET", f"/api/control/events?after={int(after)}"))

    def create_credential(self, options: dict[str, Any]) -> Any:
        """Mints a service credential. Its token is returned this once."""
        return self._do(Call("POST", "/api/control/credentials", options))

    def revoke_credential(self, credential_id: str) -> Any:
        """Revokes a service credential."""
        return self._do(Call("DELETE", _item("credentials", credential_id)))

    def members(self) -> Any:
        """The owner and every member."""
        return self._do(Call("GET", "/api/control/members"))

    def invite_member(self, role: str = "operator") -> Any:
        """An invitation code for a new member."""
        return self._do(Call("POST", "/api/control/members/invite", {"role": role}))

    def remove_member(self, user_id: str) -> Any:
        """Removes a member."""
        return self._do(Call("DELETE", _item("members", user_id)))

    def create_webhook(self, url: str, types: list[str] | None = None) -> Any:
        """Registers a signed event webhook; `types` limits which events it receives."""
        return self._do(Call("POST", "/api/control/webhooks", {"url": url, "types": types or []}))

    def remove_webhook(self, webhook_id: str) -> Any:
        """Disables a webhook."""
        return self._do(Call("DELETE", _item("webhooks", webhook_id)))

    def replay_delivery(self, delivery_id: str) -> Any:
        """Sends a webhook delivery again."""
        return self._do(Call("POST", _item("deliveries", delivery_id) + "/replay", {}))
