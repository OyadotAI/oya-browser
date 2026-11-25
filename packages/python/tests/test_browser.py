"""Driving a browser: commands, agent answers, playbooks, a borrowed desktop browser, and the browser-use hand-off."""

import builtins
import sys

import httpx
import pytest

from oya_browser import Browser, OyaError
from oya_browser._config import Origin
from oya_browser._http import Http

BASE = "https://oya.test"


def browser(f, **info):
    http = Http(Origin("k", BASE), httpx.Client(base_url=BASE, transport=httpx.MockTransport(f)))
    return Browser(http, {"id": "b1", "provider": "oya-cloud", **info})


def test_click_addresses_the_element_by_its_analyze_id(fake):
    f = fake({("POST", "/api/browsers/b1/command"): {"ok": True, "data": {}}})
    browser(f).click(13)
    assert f.calls[0][2] == {"action": "click", "params": {"element_id": 13, "selector": '[data-ac-id="13"]'}}


def test_an_element_id_that_is_not_a_number_is_refused_before_any_call(fake):
    f = fake({})
    with pytest.raises(ValueError, match="analyze"):
        browser(f).click("Submit")
    assert f.calls == []


def test_a_command_that_ran_and_failed_raises_422_with_its_error(fake):
    f = fake({("POST", "/api/browsers/b1/command"): {"ok": False, "error": "Element 13 is gone"}})
    with pytest.raises(OyaError, match="Element 13 is gone") as e:
        browser(f).click(13)
    assert e.value.status == 422


def test_ask_sends_data_and_secrets_and_returns_the_text(fake):
    f = fake({("POST", "/api/browsers/b1/chat"): {"text": "Example Domain"}})
    assert browser(f).ask("Title?", data={"a": 1}, secrets={"pw": "x"}) == "Example Domain"
    assert f.calls[0][2] == {
        "messages": [{"role": "user", "content": "Title?"}],
        "data": {"a": 1},
        "secrets": {"pw": "x"},
    }


def test_ask_with_no_data_leaves_data_and_secrets_out_rather_than_sending_null(fake):
    f = fake({("POST", "/api/browsers/b1/chat"): {"text": "Example Domain"}})
    browser(f).ask("Title?")
    assert f.calls[0][2] == {"messages": [{"role": "user", "content": "Title?"}]}


def test_an_agent_failure_in_a_200_body_raises(fake):
    f = fake({("POST", "/api/browsers/b1/chat"): {"error": "Model refused", "status": 400}})
    with pytest.raises(OyaError, match="Model refused") as e:
        browser(f).ask("x")
    assert e.value.status == 400


def test_play_replays_with_variables_and_auto_heal(fake):
    f = fake({("POST", "/api/browsers/b1/playbooks/prior-auth/play"): {"outcome": "ok"}})
    assert browser(f).play("prior-auth", {"member": "M1"})["outcome"] == "ok"
    assert f.calls[0][2] == {"variables": {"member": "M1"}, "autoHeal": True}


def test_a_playbook_name_that_would_change_the_path_is_refused(fake):
    with pytest.raises(ValueError, match="single path segment"):
        browser(fake({})).play("../admin")


def test_a_borrowed_desktop_browser_gets_its_own_lane_and_stop_only_ends_it(fake):
    f = fake({("POST", "/api/browsers/b1/command"): {"ok": True}, ("POST", "/api/browsers/b1/lane/end"): {}})
    b = browser(f, reused=True)
    b.goto("https://example.com")
    b.stop()
    assert f.calls[0][3]["X-Oya-Agent"].startswith("sdk-")
    assert f.calls[1][1] == "/api/browsers/b1/lane/end"


def test_a_started_browser_is_stopped_when_the_with_block_exits(fake):
    f = fake({("POST", "/api/browsers/b1/stop"): {"ok": True}})
    with browser(f):
        pass
    assert f.calls[-1][1] == "/api/browsers/b1/stop"


def test_for_browser_use_connects_on_a_fresh_single_use_cdp_url(fake, monkeypatch):
    f = fake({("GET", "/api/browsers/b1"): {"id": "b1", "cdpUrl": "wss://oya.test/connect?ticket=fresh"}})

    class FakeBrowserUse:
        def __init__(self, cdp_url):
            self.cdp_url = cdp_url

    monkeypatch.setitem(sys.modules, "browser_use", type(sys)("browser_use"))
    sys.modules["browser_use"].Browser = FakeBrowserUse
    assert browser(f, cdpUrl="wss://stale").for_browser_use().cdp_url == "wss://oya.test/connect?ticket=fresh"


def test_for_browser_use_says_how_to_install_it_when_it_is_missing(fake, monkeypatch):
    f = fake({("GET", "/api/browsers/b1"): {"id": "b1", "cdpUrl": "wss://x"}})
    real_import = builtins.__import__

    def no_browser_use(name, *args, **kwargs):
        if name == "browser_use":
            raise ImportError(name)
        return real_import(name, *args, **kwargs)

    monkeypatch.delitem(sys.modules, "browser_use", raising=False)
    monkeypatch.setattr(builtins, "__import__", no_browser_use)
    with pytest.raises(ImportError, match=r"oya-browser\[browser-use\]"):
        browser(f).for_browser_use()


def test_for_browser_use_refuses_a_browser_with_no_cdp_endpoint(fake):
    f = fake({("GET", "/api/browsers/b1"): {"id": "b1"}})
    with pytest.raises(ValueError, match="no CDP endpoint"):
        browser(f).for_browser_use()


def test_ask_without_data_or_secrets_sends_neither_since_the_server_refuses_null(fake):
    f = fake({("POST", "/api/browsers/b1/chat"): {"text": "Example Domain"}})
    browser(f).ask("Title?")
    assert f.calls[0][2] == {"messages": [{"role": "user", "content": "Title?"}]}


def test_extract_sends_its_schema_and_only_the_values_given(fake):
    f = fake({("POST", "/api/browsers/b1/chat"): {"text": "ok", "data": {"heading": "Example Domain"}}})
    schema = {"type": "object"}
    assert browser(f).extract("Heading?", schema) == {"heading": "Example Domain"}
    assert f.calls[0][2] == {"messages": [{"role": "user", "content": "Heading?"}], "schema": schema}


def test_extract_raises_422_when_failed_or_missing_data(fake):
    f = fake({("POST", "/api/browsers/b1/chat"): {"text": "FAILED: price missing", "failed": True}})
    with pytest.raises(OyaError, match="FAILED: price missing") as e:
        browser(f).extract("Price?", {"type": "number"})
    assert e.value.status == 422
    f_none = fake({("POST", "/api/browsers/b1/chat"): {"text": "Nothing found"}})
    with pytest.raises(OyaError, match="Nothing found") as e2:
        browser(f_none).extract("Price?", {"type": "number"})
    assert e2.value.status == 422


def test_element_id_rejects_booleans_floats_and_negatives(fake):
    b = browser(fake({}))
    for bad in (True, False, -1, 1.5, "1.5", "-1", "", " 2 "):
        with pytest.raises(ValueError, match="non-negative integer"):
            b.click(bad)  # type: ignore[arg-type]
