"""A fake control plane for the tests: records each request and answers from a route table. No network, no real key."""

import json
from collections.abc import Callable
from typing import Any

import httpx
import pytest

Route = dict[tuple[str, str], Any]


class Fake:
    """Answers (method, path) from `routes`; a callable route gets the request and may answer differently each time."""

    def __init__(self, routes: Route) -> None:
        self.routes = routes
        self.calls: list[tuple[str, str, Any, httpx.Headers]] = []

    def __call__(self, request: httpx.Request) -> httpx.Response:
        body = json.loads(request.content) if request.content else None
        self.calls.append((request.method, request.url.path, body, request.headers))
        answer = self.routes.get((request.method, request.url.path))
        if answer is None:
            return httpx.Response(404, json={"error": f"no route {request.method} {request.url.path}"})
        if callable(answer):
            answer = answer(request)
        return answer if isinstance(answer, httpx.Response) else httpx.Response(200, json=answer)


@pytest.fixture(autouse=True)
def isolated(monkeypatch: pytest.MonkeyPatch, tmp_path: Any) -> None:
    """No real environment or saved config leaks into a test."""
    for name in ("OYA_API_KEY", "OYA_BASE_URL"):
        monkeypatch.delenv(name, raising=False)
    monkeypatch.setenv("OYA_CONFIG_HOME", str(tmp_path))
    monkeypatch.setattr("time.sleep", lambda _s: None)


@pytest.fixture
def fake() -> Callable[[Route], Fake]:
    return Fake
