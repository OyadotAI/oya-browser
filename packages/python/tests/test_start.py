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


def test_captcha_auto_is_kept_by_the_client_not_sent_and_makes_goto_solve(fake):
    f = fake(
        {
            ("POST", "/api/browsers/start"): {"id": "b1", "status": "ready"},
            ("POST", "/api/browsers/b1/command"): {"ok": True},
            ("POST", "/api/browsers/b1/captcha"): {"present": False},
        }
    )
    client(f).browser.start(captcha="auto").goto("https://example.com")
    assert "captcha" not in f.calls[0][2]
    assert f.calls[-1][1] == "/api/browsers/b1/captcha"


def test_start_normalizes_snake_case_options_and_validates_idempotency_key(fake):
    from oya_browser import _start as st

    f = fake({("POST", "/api/browsers/start"): {"id": "b1", "status": "ready"}})
    client(f).browser.start(profile_group="pool", queue_ms=45_000, ws_url="ws://127.0.0.1:9222")
    body = f.calls[0][2]
    assert body == {"personaGroup": "pool", "queueMs": 45_000, "wsUrl": "ws://127.0.0.1:9222"}
    assert st.ready_timeout(None, body) == 165.0
    assert st.ready_timeout(10.0, body) == 10.0
    with pytest.raises(ValueError, match="idempotency_key"):
        client(f).browser.start(idempotency_key="bad\nkey")
