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
| `goto(url)`, `analyze(format=None)`, `elements()` | Open a page; read it as TOON, markdown or JSONL with numbered elements; `elements()` is only those in view now |
| `click(id)`, `type(id, text)`, `press_key(key)`, `wait_for(selector)` | Act on elements by their number from `analyze()` |
| `scroll(direction, amount=None, at=None)` | Scroll the page, or aim at an inner scroller with `at=(x, y)` |
| `url()`, `tabs()`, `open_tab(url=None)`, `switch_tab(id)`, `close_tab(id)` | Tabs |
| `handle_dialog(accept, prompt_text=None)` | Answer a `confirm()` or `prompt()` holding the page |
| `ask(prompt, data=, secrets=)`, `extract(prompt, schema)` | A task in plain language; `secrets` are typed but never shown to the model |
| `to_playbook(name)`, `play(name, data, auto_heal=True)` | Save the last task; replay it with no model, repairing a step a page changed |
| `submit(prompt)` or `submit(playbook=...)` | A background run with `on_success`, `on_failure`, `on_human_attention`, `on_healed` callbacks |
| `live_view_url()`, `live_stream_url()`, `share_url(control=False)`, `revoke_share(id)` | Watch it, embed its frames, or hand a person a scoped, expiring link |
| `solve_captcha()`, `complete_mfa()`, `screenshot()`, `status()`, `stop()` | |

`oya.browser.start(captcha="auto")` makes every `goto()` clear a CAPTCHA, raising `OyaError` (409) when one needs
a person.

`submit()` on `AsyncOya` polls in the background as soon as it returns; `await run.done` for the result. The
sync client has no background to poll in without a thread, so `run.wait()` polls, fires the same callbacks in the
same order, and returns the result or raises. Answer an attention request with `request["respond"]()`.

```python
run = browser.submit(playbook="prior-auth", data={"member": "M1"},
                     on_human_attention=lambda req: print("Needs you:", req["liveViewUrl"]))
result = run.wait()
```

Files ride in `data`: `browser.ask("Attach my resume", data={"resume": file("cv.pdf")})`, up to
`MAX_FILE_BYTES` (10MB).

## The client

| | |
|---|---|
| `oya.browser` | `start(persona=, provider=, name=, captcha=)`, `get(id)`, `list()`, `stop(ids)`, `stop_all()` |
| `oya.desktop` | `connect(persona=None)`: the person's own desktop browser with their logins, paired if needed |
| `Oya.signup(email)` | An agent's own key, saved to `~/.oya/config.json` |
| `oya.playbooks` | `list()`, `get(name)`, `promote(name)`, `export(name)`, `import_(doc, name=, overwrite=)`, `remove(name)` |
| `oya.llm` | `answer(question, values)`: the text for a free-text field in an exported playbook |
| `oya.personas` (`oya.profiles`) | `list`, `get`, `create`, `update`, `clone`, `preview`, `options`, `pin_proxy`, `remove`, `set_mfa`, `clear_mfa`, `set_credentials`, `credentials`, `clear_credentials`, `cookies`, `import_cookies`, `copy_cookies`, `clear_cookies` |
| `oya.persona_groups` (`oya.profile_groups`) | `list`, `create`, `update`, `remove` |
| `oya.proxies` | `list`, `create`, `remove`, `check` |
| `oya.control` | Sessions, takeover, recovery, events, service credentials, members, webhooks |
| `oya.config` | `get()`, `set(values)`: this key's model, provider and solver settings |
| `oya.usage()` | Usage on this key |

Options are keyword arguments, as with `browser.start()`:
`oya.personas.create(name="us-shopper", prefs={"locale": "en-US"}, max_concurrent=2)`; a dict still works too.
Results are the server's JSON as plain dicts. On `AsyncOya` the same calls are awaited.

Every failure the server reports raises `OyaError` with its message, `status` and `body`. A connection that breaks
before an answer comes back raises httpx's own error (`httpx.TransportError`).

## Configuration

The key and address come from the arguments (`Oya(api_key=..., base_url=...)`), then `OYA_API_KEY` and
`OYA_BASE_URL`, then the key `oya login` saved in `~/.oya/config.json`, the same order as the TypeScript SDK.

`ask()`, `extract()`, runs and healed replays use the key's saved model. Pick one in code, for this client only and
never saved to the key:

```python
oya = Oya(model={"provider": "anthropic", "name": "claude-sonnet-5", "api_key": os.environ["ANTHROPIC_API_KEY"]})
```

Without `api_key`, `name` runs on the credential your key already uses, which must be the same provider.

## HIPAA

HIPAA mode runs on Oya Cloud (BAA on request) or self-hosted in your own cloud. See
[oyabrowser.com/baa](https://oyabrowser.com/baa).

## License

MIT
