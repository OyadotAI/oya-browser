"""oya.desktop.connect(): the running desktop app, or pairing it and waiting, and a handle that never stops it."""

import httpx
import pytest

from oya_browser import AsyncOya, Oya, OyaError
from oya_browser import desktop as desktop_module

BASE = "https://oya.test"
DESK = {"id": "d1", "provider": "oya-desktop", "persona": "p1", "personaName": "Work", "health": "ok"}


def client(f):
    return Oya(api_key="k", base_url=BASE, client=httpx.Client(base_url=BASE, transport=httpx.MockTransport(f)))


@pytest.fixture(autouse=True)
def opened(monkeypatch):
    links = []
    monkeypatch.setattr(desktop_module, "open_link", links.append)
    return links


def test_a_running_desktop_app_is_returned_as_a_reused_handle_without_pairing(fake, opened):
    f = fake({("GET", "/api/browsers"): [DESK], ("GET", "/api/browsers/d1"): {"persona": "p1", "cdpUrl": "wss://d"}})
    b = client(f).desktop.connect()
    assert (b.id, b.provider, b.reused, b.cdp_url) == ("d1", "oya-desktop", True, "wss://d")
    assert opened == []


def test_a_handle_on_the_desktop_ends_only_its_own_lane_when_stopped(fake):
    f = fake(
        {("GET", "/api/browsers"): [DESK], ("GET", "/api/browsers/d1"): {}, ("POST", "/api/browsers/d1/lane/end"): {}}
    )
    assert client(f).desktop.connect().stop() == {"id": "d1", "ok": True, "reused": True}
    assert f.calls[-1][1] == "/api/browsers/d1/lane/end"


def test_stopping_a_borrowed_desktop_never_raises_when_lane_end_fails(fake):
    f = fake(
        {
            ("GET", "/api/browsers"): [DESK],
            ("GET", "/api/browsers/d1"): {},
            ("POST", "/api/browsers/d1/lane/end"): lambda _r: httpx.Response(404, json={"error": "not found"}),
        }
    )
    with client(f).desktop.connect() as b:
        assert b.reused is True


@pytest.mark.parametrize("persona", ["p1", "Work"])
def test_a_persona_matches_by_id_or_name(fake, persona, opened):
    f = fake({("GET", "/api/browsers"): [DESK], ("GET", "/api/browsers/d1"): {}})
    assert client(f).desktop.connect(persona=persona).id == "d1"


def test_a_dead_or_other_persona_desktop_is_not_used_and_pairing_starts(fake, opened):
    fleets = iter([[{**DESK, "health": "dead"}], [], [DESK]])
    f = fake(
        {
            ("GET", "/api/browsers"): lambda _r: httpx.Response(200, json=next(fleets)),
            ("POST", "/api/pairing"): {"code": "c 1"},
            ("GET", "/api/browsers/d1"): {},
        }
    )
    assert client(f).desktop.connect().id == "d1"
    assert opened == ["oya://connect?code=c%201&server=wss%3A%2F%2Foya.test%2Fws"]
    assert [c[2] for c in f.calls if c[1] == "/api/pairing"] == [{}]


def test_pairing_names_the_persona_and_open_false_does_not_open_the_link(fake, opened):
    fleets = iter([[], [DESK]])
    f = fake(
        {
            ("GET", "/api/browsers"): lambda _r: httpx.Response(200, json=next(fleets)),
            ("POST", "/api/pairing"): {"code": "c"},
            ("GET", "/api/browsers/d1"): {},
        }
    )
    client(f).desktop.connect(persona="p1", open=False)
    assert opened == []
    assert [c[2] for c in f.calls if c[1] == "/api/pairing"] == [{"persona": "p1"}]


def test_a_desktop_that_never_connects_is_a_504_carrying_the_link(fake):
    f = fake({("GET", "/api/browsers"): [], ("POST", "/api/pairing"): {"code": "c"}})
    with pytest.raises(OyaError, match="did not connect") as e:
        client(f).desktop.connect(timeout_s=0)
    assert e.value.status == 504 and e.value.body["link"].startswith("oya://connect?code=c")


async def test_the_async_desktop_pairs_and_waits_the_same_way(fake, opened):
    fleets = iter([[], [DESK]])
    f = fake(
        {
            ("GET", "/api/browsers"): lambda _r: httpx.Response(200, json=next(fleets)),
            ("POST", "/api/pairing"): {"code": "c"},
            ("GET", "/api/browsers/d1"): {},
        }
    )
    http = httpx.AsyncClient(base_url=BASE, transport=httpx.MockTransport(f))
    b = await AsyncOya(api_key="k", base_url=BASE, client=http).desktop.connect(timeout_s=5)
    assert b.reused and len(opened) == 1
