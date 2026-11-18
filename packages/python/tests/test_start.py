"""Starting a browser: ready at once, a cloud browser that dials in, one that never does, one whose creation ended."""

import httpx
import pytest

from oya_browser import Oya, OyaError

BASE = "https://oya.test"


def client(fake_transport):
    return Oya(
        api_key="k", base_url=BASE, client=httpx.Client(base_url=BASE, transport=httpx.MockTransport(fake_transport))
    )


def test_a_ready_browser_comes_back_with_its_cdp_url(fake):
    f = fake({("POST", "/api/browsers/start"): {"id": "b1", "provider": "cdp", "status": "ready", "cdpUrl": "wss://x"}})
    browser = client(f).browser.start()
    assert (browser.id, browser.cdp_url) == ("b1", "wss://x")


def test_every_start_carries_an_idempotency_key_and_the_key(fake):
    f = fake({("POST", "/api/browsers/start"): {"id": "b1", "status": "ready"}})
    client(f).browser.start(persona="p1")
    _, _, body, headers = f.calls[0]
    assert body == {"profile": "p1"}
    assert headers["Idempotency-Key"] and headers["Authorization"] == "Bearer k"


def test_a_cloud_browser_is_waited_for_then_asked_for_its_cdp_url(fake):
    fleets = iter([[], [{"id": "b1", "health": "ok"}]])
    f = fake(
        {
            ("POST", "/api/browsers/start"): {"id": "b1", "provider": "oya-cloud", "status": "starting"},
            ("GET", "/api/browsers"): lambda _r: httpx.Response(200, json=next(fleets)),
            ("GET", "/api/control/sessions/b1"): httpx.Response(404, json={"error": "not yet"}),
            ("GET", "/api/browsers/b1"): {"id": "b1", "cdpUrl": "wss://oya.test/connect?ticket=t"},
        }
    )
    browser = client(f).browser.start()
    assert browser.cdp_url == "wss://oya.test/connect?ticket=t"


def test_a_browser_whose_creation_ended_raises_at_once(fake):
    f = fake(
        {
            ("POST", "/api/browsers/start"): {"id": "b1", "status": "starting"},
            ("GET", "/api/browsers"): [],
            ("GET", "/api/control/sessions/b1"): {"state": "failed"},
        }
    )
    with pytest.raises(OyaError) as e:
        client(f).browser.start()
    assert e.value.status == 409


def test_a_browser_that_never_dials_in_times_out_with_504(fake):
    f = fake(
        {
            ("POST", "/api/browsers/start"): {"id": "b1", "status": "starting"},
            ("GET", "/api/browsers"): [],
            ("GET", "/api/control/sessions/b1"): httpx.Response(404, json={}),
        }
    )
    with pytest.raises(OyaError) as e:
        client(f).browser.start(ready_timeout_s=0)
    assert e.value.status == 504


def test_a_server_error_carries_the_servers_message(fake):
    f = fake({("POST", "/api/browsers/start"): httpx.Response(402, json={"error": "Out of browser hours"})})
    with pytest.raises(OyaError, match="Out of browser hours") as e:
        client(f).browser.start()
    assert e.value.status == 402
