"""submit(): a background run, its polling, its callbacks and respond(), sync (Run.wait) and async (AsyncRun.done)."""

import asyncio

import httpx
import pytest

from oya_browser import AsyncBrowser, Browser, OyaError
from oya_browser._config import Origin
from oya_browser._http import AsyncHttp, Http

BASE = "https://oya.test"
RUNS = ("POST", "/api/browsers/b1/runs")
STATUS = ("GET", "/api/runs/r1")
RESPOND = ("POST", "/api/runs/r1/respond")
ATTENTION = {"id": "a1", "reason": "captcha", "message": "Solve it", "liveViewUrl": "/dashboard/?browser=b1"}


def polls(*answers):
    """A status route answering each poll in turn; an int answers that HTTP status."""
    it = iter(answers)

    def route(_request):
        answer = next(it)
        return (
            httpx.Response(answer, json={"error": "down"})
            if isinstance(answer, int)
            else httpx.Response(200, json=answer)
        )

    return route


def browser(f):
    return Browser(Http(Origin("k", BASE), httpx.Client(base_url=BASE, transport=httpx.MockTransport(f))), {"id": "b1"})


def recorder():
    seen = []
    names = ("on_success", "on_failure", "on_human_attention", "on_healed")
    return seen, {n: (lambda v, n=n: seen.append((n, v))) for n in names}


def test_submit_sends_a_prompt_with_only_the_values_given(fake):
    f = fake({RUNS: {"id": "r1"}})
    run = browser(f).submit("Order a pizza", data={"name": "Ada"})
    assert run.id == "r1"
    assert f.calls[0][2] == {"prompt": "Order a pizza", "data": {"name": "Ada"}}


def test_submit_sends_a_playbook_with_auto_heal(fake):
    f = fake({RUNS: {"id": "r1"}})
    browser(f).submit(playbook="pizza", auto_heal=False)
    assert f.calls[0][2] == {"playbook": "pizza", "autoHeal": False}


@pytest.mark.parametrize("args", [{}, {"prompt": "x", "playbook": "y"}])
def test_submit_takes_exactly_one_of_prompt_and_playbook(fake, args):
    f = fake({})
    with pytest.raises(ValueError, match="exactly one"):
        browser(f).submit(**args)
    assert f.calls == []


def test_wait_polls_until_success_and_fires_on_success_with_the_result(fake):
    f = fake(
        {RUNS: {"id": "r1"}, STATUS: polls({"status": "running"}, {"status": "succeeded", "result": {"text": "ok"}})}
    )
    seen, cb = recorder()
    assert browser(f).submit("x", **cb).wait() == {"text": "ok"}
    assert seen == [("on_success", {"text": "ok"})]


def test_a_healed_replay_fires_on_healed_before_on_success(fake):
    f = fake(
        {RUNS: {"id": "r1"}, STATUS: polls({"status": "succeeded", "result": {"healed": True, "draft": "p:draft"}})}
    )
    seen, cb = recorder()
    browser(f).submit(playbook="p", **cb).wait()
    assert [n for n, _ in seen] == ["on_healed", "on_success"]


def test_a_failed_run_fires_on_failure_and_raises_with_its_status(fake):
    f = fake({RUNS: {"id": "r1"}, STATUS: polls({"status": "failed", "error": "Quota", "errorStatus": 429})})
    seen, cb = recorder()
    with pytest.raises(OyaError, match="Quota") as e:
        browser(f).submit("x", **cb).wait()
    assert e.value.status == 429
    assert seen[0][0] == "on_failure" and seen[0][1] is e.value


def test_a_failure_without_a_status_is_a_500(fake):
    f = fake({RUNS: {"id": "r1"}, STATUS: polls({"status": "failed"})})
    with pytest.raises(OyaError, match="Run failed") as e:
        browser(f).submit("x").wait()
    assert e.value.status == 500


def test_an_attention_request_fires_once_with_an_absolute_live_view_link_and_a_respond(fake):
    waiting = {"status": "needs_attention", "attention": ATTENTION}
    f = fake(
        {
            RUNS: {"id": "r1"},
            STATUS: polls(waiting, waiting, {"status": "succeeded", "result": {}}),
            RESPOND: {"ok": True},
        }
    )
    seen, cb = recorder()
    cb["on_human_attention"] = lambda req: (seen.append(("attention", req)), req["respond"]())
    browser(f).submit("x", **cb).wait()
    attention = [v for n, v in seen if n == "attention"]
    assert len(attention) == 1
    assert attention[0]["liveViewUrl"] == f"{BASE}/dashboard/?browser=b1"
    assert ("POST", "/api/runs/r1/respond", {"response": "done"}) in [c[:3] for c in f.calls]


def test_a_few_failed_polls_are_retried(fake):
    f = fake({RUNS: {"id": "r1"}, STATUS: polls(503, 503, {"status": "succeeded", "result": {"text": "ok"}})})
    assert browser(f).submit("x").wait() == {"text": "ok"}


def test_five_failed_polls_in_a_row_end_the_run_with_on_failure(fake):
    f = fake({RUNS: {"id": "r1"}, STATUS: polls(503, 503, 503, 503, 503)})
    seen, cb = recorder()
    with pytest.raises(OyaError) as e:
        browser(f).submit("x", **cb).wait()
    assert e.value.status == 503
    assert seen == [("on_failure", e.value)]


def test_a_callback_that_raises_does_not_break_the_run(fake):
    f = fake({RUNS: {"id": "r1"}, STATUS: polls({"status": "succeeded", "result": {"text": "ok"}})})

    def boom(_result):
        raise RuntimeError("callback bug")

    assert browser(f).submit("x", on_success=boom).wait() == {"text": "ok"}


def test_respond_sends_the_reply(fake):
    f = fake({RUNS: {"id": "r1"}, RESPOND: {"ok": True}})
    browser(f).submit("x").respond("Use the second address")
    assert f.calls[-1][2] == {"response": "Use the second address"}


def async_browser(f):
    http = AsyncHttp(Origin("k", BASE), httpx.AsyncClient(base_url=BASE, transport=httpx.MockTransport(f)))
    return AsyncBrowser(http, {"id": "b1"})


async def test_an_async_run_polls_in_the_background_and_awaits_async_callbacks(fake):
    waiting = {"status": "needs_attention", "attention": ATTENTION}
    f = fake(
        {
            RUNS: {"id": "r1"},
            STATUS: polls(waiting, {"status": "succeeded", "result": {"text": "ok"}}),
            RESPOND: {"ok": True},
        }
    )
    seen = []

    async def on_success(result):
        seen.append(("success", result))

    async def on_attention(request):
        await request["respond"]("done")
        seen.append(("attention", request["id"]))

    run = await async_browser(f).submit("x", on_success=on_success, on_human_attention=on_attention, poll_s=0)
    assert await run.done == {"text": "ok"}
    await asyncio.sleep(0)
    assert ("success", {"text": "ok"}) in seen and ("attention", "a1") in seen


async def test_an_async_run_that_fails_raises_from_done_and_fires_on_failure(fake):
    f = fake({RUNS: {"id": "r1"}, STATUS: polls({"status": "failed", "error": "Nope"})})
    seen = []
    run = await async_browser(f).submit("x", on_failure=seen.append, poll_s=0)
    with pytest.raises(OyaError, match="Nope"):
        await run.wait()
    assert isinstance(seen[0], OyaError)
