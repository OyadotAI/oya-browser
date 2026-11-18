"""Tabs, dialogs, aimed scrolls, captcha="auto", and the live-view, stream and share links."""

import httpx
import pytest

from oya_browser import AsyncBrowser, Browser, OyaError
from oya_browser._config import Origin
from oya_browser._http import AsyncHttp, Http

BASE = "https://oya.test"
CMD = ("POST", "/api/browsers/b1/command")


def browser(f, auto_captcha=False):
    http = Http(Origin("k", BASE), httpx.Client(base_url=BASE, transport=httpx.MockTransport(f)))
    return Browser(http, {"id": "b1", "provider": "oya-cloud"}, auto_captcha)


def sent(f, i=0):
    return f.calls[i][2]


def test_url_is_the_active_tabs_url(fake):
    tabs = [{"id": "t1", "url": "https://a", "active": False}, {"id": "t2", "url": "https://b", "active": True}]
    f = fake({CMD: {"ok": True, "data": {"tabs": tabs}}})
    assert browser(f).url() == "https://b"
    assert sent(f) == {"action": "list_tabs", "params": {}}


def test_url_is_empty_when_no_tab_is_active(fake):
    assert browser(fake({CMD: {"ok": True, "data": {"tabs": []}}})).url() == ""


def test_open_tab_sends_the_url_and_returns_the_new_tabs_id(fake):
    f = fake({CMD: {"ok": True, "data": {"tab_id": "t9"}}})
    assert browser(f).open_tab("https://example.com") == "t9"
    assert sent(f) == {"action": "open_tab", "params": {"url": "https://example.com"}}


def test_open_tab_without_a_url_sends_no_url(fake):
    f = fake({CMD: {"ok": True, "data": {"tab_id": "t9"}}})
    browser(f).open_tab()
    assert sent(f) == {"action": "open_tab", "params": {}}


def test_a_numeric_tab_id_comes_back_and_goes_out_as_a_number(fake):
    f = fake({CMD: {"ok": True, "data": {"tab_id": 2}}})
    b = browser(f)
    b.switch_tab(b.open_tab())
    assert sent(f, 1) == {"action": "switch_tab", "params": {"tab_id": 2}}


@pytest.mark.parametrize("method,action", [("switch_tab", "switch_tab"), ("close_tab", "close_tab")])
def test_switch_and_close_tab_address_the_tab_by_id(fake, method, action):
    f = fake({CMD: {"ok": True}})
    getattr(browser(f), method)("t2")
    assert sent(f) == {"action": action, "params": {"tab_id": "t2"}}


def test_handle_dialog_sends_accept_and_the_prompt_text(fake):
    f = fake({CMD: {"ok": True, "data": {"type": "prompt", "accepted": True, "message": "Name?"}}})
    assert browser(f).handle_dialog(True, "Ada")["accepted"] is True
    assert sent(f) == {"action": "handle_dialog", "params": {"accept": True, "prompt_text": "Ada"}}


def test_handle_dialog_with_no_dialog_open_raises_the_commands_error(fake):
    f = fake({CMD: {"ok": False, "error": "No dialog is open"}})
    with pytest.raises(OyaError, match="No dialog is open") as e:
        browser(f).handle_dialog(False)
    assert e.value.status == 422
    assert sent(f)["params"] == {"accept": False}


def test_a_plain_scroll_sends_only_direction_and_amount(fake):
    f = fake({CMD: {"ok": True}})
    browser(f).scroll("down", 300)
    assert sent(f)["params"] == {"direction": "down", "amount": 300}


def test_an_aimed_scroll_is_one_unsmoothed_wheel_event_at_the_point_with_the_default_amount(fake):
    f = fake({CMD: {"ok": True}})
    browser(f).scroll("down", at=(120, 480))
    assert sent(f)["params"] == {"direction": "down", "amount": 500, "x": 120, "y": 480, "smooth": False}


def test_captcha_auto_solves_after_goto(fake):
    f = fake({CMD: {"ok": True}, ("POST", "/api/browsers/b1/captcha"): {"present": True, "solved": True}})
    browser(f, auto_captcha=True).goto("https://example.com")
    assert [c[1] for c in f.calls] == ["/api/browsers/b1/command", "/api/browsers/b1/captcha"]


def test_captcha_auto_raises_409_when_a_captcha_is_still_on_screen(fake):
    f = fake({CMD: {"ok": True}, ("POST", "/api/browsers/b1/captcha"): {"present": True, "solved": False}})
    with pytest.raises(OyaError, match="CAPTCHA needs attention") as e:
        browser(f, auto_captcha=True).goto("https://example.com")
    assert e.value.status == 409


def test_without_captcha_auto_goto_does_not_touch_captchas(fake):
    f = fake({CMD: {"ok": True}})
    browser(f).goto("https://example.com")
    assert len(f.calls) == 1


def test_live_view_url_is_the_console_link_and_carries_no_credential(fake):
    assert browser(fake({})).live_view_url() == f"{BASE}/dashboard/?browser=b1"


def test_live_stream_url_carries_a_fresh_single_use_ticket(fake):
    f = fake({("POST", "/api/control/sessions/b1/ticket"): {"ticket": "t k"}})
    assert browser(f).live_stream_url() == f"{BASE}/api/live/b1?ticket=t%20k"


def test_share_url_puts_the_token_in_the_fragment_and_sends_control_and_expiry(fake):
    credential = {"id": "c1", "token": "secret", "expiresAt": 123}
    f = fake({("POST", "/api/control/sessions/b1/share"): credential})
    link = browser(f).share_url(control=True, expires_in_s=60)
    assert link == {"url": f"{BASE}/live/b1#t=secret", "id": "c1", "expiresAt": 123}
    assert sent(f) == {"control": True, "expiresIn": 60}


def test_share_url_is_view_only_for_an_hour_by_default(fake):
    f = fake({("POST", "/api/control/sessions/b1/share"): {"id": "c1", "token": "s", "expiresAt": None}})
    browser(f).share_url()
    assert sent(f) == {"control": False, "expiresIn": 3600}


def test_revoke_share_deletes_the_credential(fake):
    f = fake({("DELETE", "/api/control/credentials/c1"): {"ok": True}})
    browser(f).revoke_share("c1")
    assert f.calls[0][:2] == ("DELETE", "/api/control/credentials/c1")


def test_revoke_share_refuses_an_id_that_would_change_the_path(fake):
    with pytest.raises(ValueError, match="single path segment"):
        browser(fake({})).revoke_share("../members/x")


async def test_the_async_browser_makes_the_same_tab_and_share_calls(fake):
    f = fake(
        {
            CMD: {"ok": True, "data": {"tabs": [{"id": "t1", "url": "https://a", "active": True}], "tab_id": "t2"}},
            ("POST", "/api/control/sessions/b1/share"): {"id": "c1", "token": "s", "expiresAt": 1},
        }
    )
    http = AsyncHttp(Origin("k", BASE), httpx.AsyncClient(base_url=BASE, transport=httpx.MockTransport(f)))
    b = AsyncBrowser(http, {"id": "b1"})
    assert await b.url() == "https://a"
    assert await b.open_tab() == "t2"
    await b.scroll("up", at=(1, 2))
    assert (await b.share_url())["url"] == f"{BASE}/live/b1#t=s"
    assert sent(f, 2)["params"] == {"direction": "up", "amount": 500, "x": 1, "y": 2, "smooth": False}
