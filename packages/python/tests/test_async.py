"""The asyncio client makes the same calls as the sync one."""

import httpx

from oya_browser import AsyncOya

BASE = "https://oya.test"


async def test_async_start_waits_for_a_cloud_browser_then_drives_it(fake):
    fleets = iter([[], [{"id": "b1"}]])
    f = fake(
        {
            ("POST", "/api/browsers/start"): {"id": "b1", "status": "starting"},
            ("GET", "/api/browsers"): lambda _r: httpx.Response(200, json=next(fleets)),
            ("GET", "/api/control/sessions/b1"): httpx.Response(404, json={}),
            ("GET", "/api/browsers/b1"): {"id": "b1", "cdpUrl": "wss://x"},
            ("POST", "/api/browsers/b1/chat"): {"text": "done"},
            ("POST", "/api/browsers/b1/stop"): {"ok": True},
        }
    )
    client = httpx.AsyncClient(base_url=BASE, transport=httpx.MockTransport(f))
    async with AsyncOya(api_key="k", base_url=BASE, client=client) as oya:
        async with await oya.browser.start(ready_timeout_s=5) as browser:
            assert browser.cdp_url == "wss://x"
            assert await browser.ask("go") == "done"
    assert f.calls[-1][1] == "/api/browsers/b1/stop"
