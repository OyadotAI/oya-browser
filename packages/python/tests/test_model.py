"""The per-client model option: sent as the headers the server reads on every call, kept by a browser that adds its
own lane header, and refused before any request when a value cannot travel in a header, without echoing it."""

import httpx
import pytest

from oya_browser import AsyncOya, Oya

BASE = "https://oya.test"
MODEL = {"provider": "anthropic", "name": "claude-sonnet-5", "api_key": " sk-ant-test "}
EXPECTED = {"x-oya-llm-provider": "anthropic", "x-oya-llm-model": "claude-sonnet-5", "x-oya-llm-api-key": "sk-ant-test"}


def recorder():
    seen: list[httpx.Headers] = []

    def handler(request: httpx.Request) -> httpx.Response:
        seen.append(request.headers)
        return httpx.Response(200, json=[])

    return seen, handler


def test_the_model_rides_on_every_call_as_headers():
    seen, handler = recorder()
    client = httpx.Client(base_url=BASE, transport=httpx.MockTransport(handler))
    with Oya(api_key="k", base_url=BASE, client=client, model=MODEL) as oya:
        oya.browser.list()
        oya.personas.list()
    assert all({k: h.get(k) for k in EXPECTED} == EXPECTED for h in seen)


def test_only_the_fields_given_are_sent():
    seen, handler = recorder()
    client = httpx.Client(base_url=BASE, transport=httpx.MockTransport(handler))
    with Oya(api_key="k", base_url=BASE, client=client, model={"name": "gpt-5"}) as oya:
        oya.browser.list()
    assert seen[0].get("x-oya-llm-model") == "gpt-5"
    assert "x-oya-llm-provider" not in seen[0] and "x-oya-llm-api-key" not in seen[0]


def test_no_model_sends_no_model_headers():
    seen, handler = recorder()
    client = httpx.Client(base_url=BASE, transport=httpx.MockTransport(handler))
    with Oya(api_key="k", base_url=BASE, client=client) as oya:
        oya.browser.list()
    assert not any(k.startswith("x-oya-llm") for k in seen[0])


def test_a_value_a_header_cannot_carry_is_refused_by_position_without_echoing_it():
    with pytest.raises(ValueError) as refused:
        Oya(api_key="k", base_url=BASE, model={"api_key": "sk-ant-sécret"})
    assert "model.api_key" in str(refused.value) and "position 9" in str(refused.value)
    assert "sécret" not in str(refused.value)


async def test_async_oya_sends_the_same_headers():
    seen: list[httpx.Headers] = []

    async def handler(request: httpx.Request) -> httpx.Response:
        seen.append(request.headers)
        return httpx.Response(200, json=[])

    client = httpx.AsyncClient(base_url=BASE, transport=httpx.MockTransport(handler))
    async with AsyncOya(api_key="k", base_url=BASE, client=client, model=MODEL) as oya:
        await oya.browser.list()
    assert {k: seen[0].get(k) for k in EXPECTED} == EXPECTED
