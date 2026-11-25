"""oya.control, personas, persona_groups, proxies, config, llm, playbooks and usage: each call's method, path and
body, the list endpoints unwrapped, refusals before any request, and the same calls from AsyncOya.
"""

import json

import httpx
import pytest

from oya_browser import AsyncOya, Oya, OyaError

BASE = "https://oya.test"
ANY = {"ok": True}

# (call, (method, path, body sent), answer, what the method returns)
CASES = [
    (lambda o: o.control.overview(), ("GET", "/api/control", None), ANY, ANY),
    (lambda o: o.control.sessions(), ("GET", "/api/control/sessions", None), [], []),
    (lambda o: o.control.session("s1"), ("GET", "/api/control/sessions/s1", None), ANY, ANY),
    (lambda o: o.control.settings({"maxBrowsers": 3}), ("PATCH", "/api/control/project", {"maxBrowsers": 3}), ANY, ANY),
    (lambda o: o.control.cancel("s1"), ("POST", "/api/control/sessions/s1/cancel", {}), ANY, ANY),
    (lambda o: o.control.stop("s1"), ("POST", "/api/control/sessions/s1/stop", {"force": False}), ANY, ANY),
    (
        lambda o: o.control.takeover("s1", "acquire"),
        ("POST", "/api/control/sessions/s1/control", {"action": "acquire"}),
        ANY,
        ANY,
    ),
    (
        lambda o: o.control.input("s1", "click", {"x": 1}),
        ("POST", "/api/control/sessions/s1/input", {"action": "click", "params": {"x": 1}}),
        ANY,
        ANY,
    ),
    (lambda o: o.control.recover("s1"), ("POST", "/api/control/sessions/s1/recover", {"replace": False}), ANY, ANY),
    (
        lambda o: o.control.recover("s1", replace=True, ws_url="ws://c"),
        ("POST", "/api/control/sessions/s1/recover", {"replace": True, "wsUrl": "ws://c"}),
        ANY,
        ANY,
    ),
    (lambda o: o.control.ticket("s1"), ("POST", "/api/control/sessions/s1/ticket", {}), ANY, ANY),
    (lambda o: o.control.events(7), ("GET", "/api/control/events", None), ANY, ANY),
    (
        lambda o: o.control.create_credential({"name": "ci"}),
        ("POST", "/api/control/credentials", {"name": "ci"}),
        ANY,
        ANY,
    ),
    (lambda o: o.control.revoke_credential("c1"), ("DELETE", "/api/control/credentials/c1", None), ANY, ANY),
    (lambda o: o.control.members(), ("GET", "/api/control/members", None), ANY, ANY),
    (lambda o: o.control.invite_member(), ("POST", "/api/control/members/invite", {"role": "operator"}), ANY, ANY),
    (lambda o: o.control.remove_member("u1"), ("DELETE", "/api/control/members/u1", None), ANY, ANY),
    (
        lambda o: o.control.create_webhook("https://h"),
        ("POST", "/api/control/webhooks", {"url": "https://h", "types": []}),
        ANY,
        ANY,
    ),
    (lambda o: o.control.remove_webhook("w1"), ("DELETE", "/api/control/webhooks/w1", None), ANY, ANY),
    (lambda o: o.control.replay_delivery("d1"), ("POST", "/api/control/deliveries/d1/replay", {}), ANY, ANY),
    (lambda o: o.personas.list(), ("GET", "/api/personas", None), {"personas": [{"id": "p1"}]}, [{"id": "p1"}]),
    (lambda o: o.profiles.get("p1"), ("GET", "/api/personas/p1", None), ANY, ANY),
    (lambda o: o.personas.create(), ("POST", "/api/personas", {}), ANY, ANY),
    (lambda o: o.personas.update("p1", {"name": "n"}), ("PUT", "/api/personas/p1", {"name": "n"}), ANY, ANY),
    (lambda o: o.personas.clone("p1"), ("POST", "/api/personas/p1/clone", {}), ANY, ANY),
    (
        lambda o: o.personas.create(name="us", prefs={"locale": "en-US"}, max_concurrent=2),
        ("POST", "/api/personas", {"name": "us", "prefs": {"locale": "en-US"}, "maxConcurrent": 2}),
        ANY,
        ANY,
    ),
    (
        lambda o: o.personas.create({"name": "old"}, max_concurrent=1),
        ("POST", "/api/personas", {"name": "old", "maxConcurrent": 1}),
        ANY,
        ANY,
    ),
    (lambda o: o.personas.update("p1", name="n"), ("PUT", "/api/personas/p1", {"name": "n"}), ANY, ANY),
    (lambda o: o.personas.clone("p1", name="c"), ("POST", "/api/personas/p1/clone", {"name": "c"}), ANY, ANY),
    (
        lambda o: o.personas.preview({"platform": "mac"}),
        ("POST", "/api/personas/preview", {"prefs": {"platform": "mac"}}),
        {"fingerprint": {"ua": "x"}},
        {"ua": "x"},
    ),
    (lambda o: o.personas.options(), ("GET", "/api/personas/options", None), ANY, ANY),
    (lambda o: o.personas.pin_proxy("p1", None), ("PUT", "/api/personas/p1/proxy", {"proxyId": None}), ANY, ANY),
    (lambda o: o.personas.remove("p1"), ("DELETE", "/api/personas/p1", None), ANY, ANY),
    (lambda o: o.personas.set_mfa("p1", {"type": "totp"}), ("PUT", "/api/personas/p1/mfa", {"type": "totp"}), ANY, ANY),
    (lambda o: o.personas.clear_mfa("p1"), ("DELETE", "/api/personas/p1/mfa", None), ANY, ANY),
    (
        lambda o: o.personas.set_credentials("p1", {"domain": "a.com"}),
        ("PUT", "/api/personas/p1/credentials", {"domain": "a.com"}),
        ANY,
        ANY,
    ),
    (lambda o: o.personas.credentials("p1"), ("GET", "/api/personas/p1/credentials", None), ANY, ANY),
    (lambda o: o.personas.clear_credentials("p1", "a.com"), ("DELETE", "/api/personas/p1/credentials", None), ANY, ANY),
    (
        lambda o: o.personas.cookies("p1"),
        ("GET", "/api/pool/cookies", None),
        {"cookies": [{"name": "s"}]},
        [{"name": "s"}],
    ),
    (
        lambda o: o.personas.import_cookies("p1", [{"name": "s"}]),
        ("PUT", "/api/pool/cookies", {"cookies": [{"name": "s"}]}),
        ANY,
        ANY,
    ),
    (lambda o: o.personas.clear_cookies("p1"), ("DELETE", "/api/pool/cookies", None), ANY, ANY),
    (
        lambda o: o.persona_groups.list(),
        ("GET", "/api/persona-groups", None),
        {"groups": [{"name": "g"}]},
        [{"name": "g"}],
    ),
    (
        lambda o: o.profile_groups.create("g", ["p1"]),
        ("POST", "/api/persona-groups", {"name": "g", "personaIds": ["p1"]}),
        ANY,
        ANY,
    ),
    (lambda o: o.persona_groups.update("g", []), ("PUT", "/api/persona-groups/g", {"personaIds": []}), ANY, ANY),
    (lambda o: o.persona_groups.remove("g"), ("DELETE", "/api/persona-groups/g", None), ANY, ANY),
    (lambda o: o.proxies.list(), ("GET", "/api/proxies", None), {"proxies": [{"id": "x"}]}, [{"id": "x"}]),
    (lambda o: o.proxies.create({"url": "http://p"}), ("POST", "/api/proxies", {"url": "http://p"}), ANY, ANY),
    (lambda o: o.proxies.remove("x"), ("DELETE", "/api/proxies/x", None), ANY, ANY),
    (lambda o: o.proxies.check(), ("POST", "/api/proxies/check", {}), {"results": [{"ok": True}]}, [{"ok": True}]),
    (lambda o: o.config.get(), ("GET", "/api/config", None), ANY, ANY),
    (lambda o: o.config.set({"llm_provider": None}), ("POST", "/api/config", {"llm_provider": None}), ANY, ANY),
    (
        lambda o: o.llm.answer("Why?", {"a": 1}),
        ("POST", "/api/playbooks/answer", {"question": "Why?", "values": {"a": 1}}),
        {"answer": "Because"},
        "Because",
    ),
    (
        lambda o: o.llm.answer("Why?", task="t"),
        ("POST", "/api/playbooks/answer", {"question": "Why?", "values": {}, "task": "t"}),
        {"answer": "B"},
        "B",
    ),
    (lambda o: o.playbooks.list(), ("GET", "/api/playbooks", None), {"playbooks": [{"name": "p"}]}, [{"name": "p"}]),
    (lambda o: o.playbooks.promote("p"), ("POST", "/api/playbooks/p/promote", {}), ANY, ANY),
    (lambda o: o.playbooks.export("p"), ("GET", "/api/playbooks/p/export", None), ANY, ANY),
    (
        lambda o: o.playbooks.import_({"format": "oya-playbook"}),
        ("POST", "/api/playbooks/import", {"playbook": {"format": "oya-playbook"}}),
        ANY,
        ANY,
    ),
    (
        lambda o: o.playbooks.import_({"format": "oya-playbook"}, name="q", overwrite=True),
        ("POST", "/api/playbooks/import", {"playbook": {"format": "oya-playbook"}, "name": "q", "overwrite": True}),
        ANY,
        ANY,
    ),
    (lambda o: o.playbooks.remove("p:draft"), ("DELETE", "/api/playbooks/p%3Adraft", None), ANY, ANY),
    (lambda o: o.usage(), ("GET", "/api/usage", None), ANY, ANY),
]


def recording(answer):
    """A handler answering `answer`, recording (method, raw path, JSON body or None, raw query) per request."""
    calls = []

    def handler(request):
        body = json.loads(request.content) if request.content else None
        calls.append((request.method, request.url.raw_path.decode().split("?")[0], body, request.url.query.decode()))
        return httpx.Response(200, json=answer)

    return calls, handler


def sync_client(handler):
    return Oya(api_key="k", base_url=BASE, client=httpx.Client(base_url=BASE, transport=httpx.MockTransport(handler)))


@pytest.mark.parametrize("call,expected,answer,returned", CASES)
def test_each_call_sends_its_method_path_and_body_and_unwraps_lists(call, expected, answer, returned):
    calls, handler = recording(answer)
    assert call(sync_client(handler)) == returned
    assert calls[0][:3] == expected


@pytest.mark.parametrize("call,expected,answer,returned", CASES)
async def test_async_oya_makes_the_same_call(call, expected, answer, returned):
    calls, handler = recording(answer)
    client = httpx.AsyncClient(base_url=BASE, transport=httpx.MockTransport(handler))
    async with AsyncOya(api_key="k", base_url=BASE, client=client) as oya:
        assert await call(oya) == returned
    assert calls[0][:2] == expected[:2]


@pytest.mark.parametrize(
    "call,query",
    [
        (lambda o: o.control.events(7), "after=7"),
        (lambda o: o.personas.clear_mfa("p1", "a b.com"), "domain=a%20b.com"),
        (lambda o: o.personas.clear_credentials("p1", "a.com"), "domain=a.com"),
        (lambda o: o.personas.cookies("p1", "playwright"), "persona=p1&format=playwright"),
        (lambda o: o.personas.clear_cookies("p 1"), "persona=p%201"),
    ],
)
def test_query_strings_are_encoded(call, query):
    calls, handler = recording(ANY)
    call(sync_client(handler))
    assert calls[0][3] == query


def test_copy_cookies_reads_one_jar_and_writes_it_into_the_other():
    calls, handler = recording({"cookies": [{"name": "sid"}]})
    sync_client(handler).personas.copy_cookies("from", "to")
    assert [(c[0], c[3]) for c in calls] == [("GET", "persona=from&format=json"), ("PUT", "persona=to")]
    assert calls[1][2] == {"cookies": [{"name": "sid"}]}


async def test_async_copy_cookies_does_the_same():
    calls, handler = recording({"cookies": [{"name": "sid"}]})
    client = httpx.AsyncClient(base_url=BASE, transport=httpx.MockTransport(handler))
    await AsyncOya(api_key="k", base_url=BASE, client=client).personas.copy_cookies("from", "to")
    assert calls[1][2] == {"cookies": [{"name": "sid"}]}


def test_recover_refuses_a_ws_url_without_replace_before_any_request():
    calls, handler = recording(ANY)
    with pytest.raises(ValueError, match="replace=True"):
        sync_client(handler).control.recover("s1", ws_url="ws://c")
    assert calls == []


@pytest.mark.parametrize(
    "call",
    [
        lambda o: o.personas.get("../config"),
        lambda o: o.persona_groups.remove("a/b"),
        lambda o: o.control.session(".."),
        lambda o: o.proxies.remove(""),
        lambda o: o.playbooks.promote("x/y"),
    ],
)
def test_an_id_that_would_change_the_path_is_refused_before_any_request(call):
    calls, handler = recording(ANY)
    with pytest.raises(ValueError):
        call(sync_client(handler))
    assert calls == []


def test_a_server_error_raises_with_its_message_and_status():
    client = sync_client(lambda _r: httpx.Response(409, json={"error": "Group name taken"}))
    with pytest.raises(OyaError, match="Group name taken") as e:
        client.persona_groups.create("g")
    assert e.value.status == 409
