# oya-browser

Real browsers for AI agents, from Python. Start a browser on Oya Cloud, your own infrastructure or any
provider your key routes to; drive it; have an agent do a task once; save it as a playbook; replay it with no
model. The Python twin of [`@oya-ai/browser`](https://www.npmjs.com/package/@oya-ai/browser).

```bash
pip install oya-browser
export OYA_API_KEY=oya_...    # from https://oyabrowser.com, or run `oya login` once
```

## Quickstart

```python
from oya_browser import Oya

with Oya() as oya, oya.browser.start() as browser:      # stopped when the block exits
    browser.goto("https://httpbin.org/forms/post")
    browser.ask("Order a large pizza for {{name}}, then submit the order", data={"name": "Ada"})
    browser.to_playbook("pizza")                         # the run, saved as Playwright code
    browser.play("pizza", {"name": "Alan"})              # replayed with no model
```

Async, for agent frameworks:

```python
from oya_browser import AsyncOya

async with AsyncOya() as oya:
    async with await oya.browser.start() as browser:
        print(await browser.ask("What is the title of https://example.com?"))
```

## browser-use on Oya

```bash
pip install "oya-browser[browser-use]"   # Python 3.11+
```

```python
from browser_use import Agent, ChatAnthropic
from oya_browser import AsyncOya

async with AsyncOya() as oya:
    async with await oya.browser.start() as browser:
        agent = Agent("Find the cheapest flight to Lisbon", llm=ChatAnthropic(model="claude-sonnet-5"),
                      browser=await browser.for_browser_use())
        print((await agent.run()).final_result())
```

browser-use drives the Oya browser over CDP, unchanged. Each Oya CDP URL carries a single-use ticket, so
`for_browser_use()` asks for a fresh one every time; use `fresh_cdp_url()` the same way for any other CDP tool.

## Playwright, Puppeteer and Stagehand

`browser.cdp_url` (or `browser.fresh_cdp_url()`) is a CDP endpoint for Playwright's `connect_over_cdp`, and for
Stagehand v3's `cdpUrl`. Stagehand v4 runs inside its own browser extension, which Oya browsers do not load yet.

## What a browser can do

| | |
|---|---|
| `goto(url)`, `analyze(format=None)`, `elements()` | Open a page; read it as TOON, markdown or JSONL with numbered elements |
| `click(id)`, `type(id, text)`, `press_key(key)`, `scroll(direction)`, `wait_for(selector)` | Act on elements by their number from `analyze()` |
| `ask(prompt, data=, secrets=)`, `extract(prompt, schema)` | A task in plain language; `secrets` are typed but never shown to the model |
| `to_playbook(name)`, `play(name, data, auto_heal=True)` | Save the last task; replay it with no model, repairing a step a page changed |
| `solve_captcha()`, `complete_mfa()`, `screenshot()`, `status()`, `stop()` | |

`oya.browser`: `start(persona=, provider=, name=)`, `get(id)`, `list()`, `stop(ids)`, `stop_all()`.
`oya.playbooks`: `list()`, `get(name)`, `export(name)`, `remove(name)`.

Every failure raises `OyaError` with the server's message, `status` and `body`.

## Configuration

The key and address come from the arguments (`Oya(api_key=..., base_url=...)`), then `OYA_API_KEY` and
`OYA_BASE_URL`, then the key `oya login` saved in `~/.oya/config.json`, the same order as the TypeScript SDK.

## HIPAA

HIPAA mode runs on Oya Cloud (BAA on request) or self-hosted in your own cloud. See
[oyabrowser.com/baa](https://oyabrowser.com/baa).

## License

MIT
