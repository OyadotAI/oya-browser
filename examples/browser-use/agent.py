"""Run a browser-use agent on an Oya browser.

Oya starts the browser (on Oya Cloud, or whichever provider your project routes to), puts every
action on its audit trail and lets a person take over; browser-use drives it over CDP, unchanged.
Oya's CDP URL carries a single-use ticket, which is what lets browser-use connect without the
custom headers it does not support.

    pip install browser-use httpx
    export OYA_API_KEY=oya_...          # from oyabrowser.com
    export ANTHROPIC_API_KEY=sk-ant-... # the model browser-use calls
    python agent.py
"""

import asyncio
import os

import httpx
from browser_use import Agent, Browser, ChatAnthropic

BASE_URL = os.environ.get("OYA_BASE_URL", "https://oyabrowser.com").rstrip("/")
MODEL = os.environ.get("BROWSER_USE_MODEL", "claude-sonnet-5")
# Starting a cloud browser can take a while on a cold pool; the API answers once it is ready.
START_TIMEOUT_S = 120
TASK = "Open https://example.com and tell me the page title."


async def main() -> None:
    headers = {"Authorization": f"Bearer {os.environ['OYA_API_KEY']}"}
    async with httpx.AsyncClient(base_url=BASE_URL, headers=headers, timeout=START_TIMEOUT_S) as oya:
        started = (await oya.post("/api/browsers/start", json={})).raise_for_status().json()
        try:
            agent = Agent(task=TASK, llm=ChatAnthropic(model=MODEL), browser=Browser(cdp_url=started["cdpUrl"]))
            history = await agent.run()
            print(history.final_result())
        finally:
            # Stop it even when the agent fails: a running cloud browser keeps billing hours.
            await oya.post(f"/api/browsers/{started['id']}/stop")


if __name__ == "__main__":
    asyncio.run(main())
