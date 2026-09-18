---
name: oya-browser
description: Drive real Chrome browsers through Oya Browser. Start a browser on Oya Cloud, Browserbase, Steel, Anchor or Browser Use under a persistent persona, read pages as markdown with numbered elements, click and type, solve CAPTCHAs and MFA, and hand off to a human when needed. Use when the user asks to browse a site, automate a web task, fill a form, log in somewhere, scrape, or run several browsers at once. Works through the Oya MCP tools (start_browser, analyze_page, click, type…), the `oya` CLI, or the @oya-ai/browser SDK.
---

# Oya Browser

Oya gives you real Chrome browsers behind one API. Each browser runs as a **persona**: a fingerprint, cookie jar and proxy that stay the same across runs, so a site sees the same device every time. You drive it by page structure, not selectors: read the page as markdown with numbered elements, then act on the numbers.

## Setup (once)

1. The user needs an API key from https://browser.getoya.ai (Dashboard → API keys). Ask for it; never invent one.
2. `export OYA_API_KEY=...` — the CLI and SDK read it.
3. Use whichever interface you have:
   - **MCP tools** (best inside an agent). If they are missing, the user can add them:
     `claude mcp add --transport http oya https://browser.getoya.ai/mcp/pool --header "Authorization: Bearer $OYA_API_KEY"`
   - **CLI**: `npm install -g @oya-ai/cli`
   - **SDK**, in code: `npm install @oya-ai/browser`

## Over MCP

1. `start_browser` — starts a browser and makes every other tool drive it. Optional: `persona` (`"auto"`, `"default"` or an id), `provider`, `url`.
2. `navigate(url)`, then `analyze_page()`.
3. Act: `click(element_id)`, `type(element_id, text)`, `press_key(key)`, `scroll(direction, amount?)`.
4. `analyze_page()` again after anything that changes the page. Element ids are reassigned on every analysis.
5. `stop_browser()` when you are done. A running browser costs money.

Also available: `screenshot`, `wait(selector)`, `click_coordinates(x, y)`, `mouse_move(x, y)`, `double_click`, `keyboard_type(text)`, `drag`, `pool_status`.

**Native dialogs.** An `alert()` or `beforeunload` is answered for you and its
text comes back on the next tool result — read it, it usually says why the last
action did not do what you expected. A `confirm()` or `prompt()` holds the page:
every other command fails immediately with the dialog's message until you call
`handle_dialog(accept, prompt_text?)`. Accept only what the task asks for — a
confirm is often guarding something destructive.

If a tool says no browser is running, call `start_browser`. If `pool_status` already lists browsers (the user's desktop app, say), you can drive those without starting one.

## Over the CLI

```bash
oya start --persona auto          # prints the browser id
oya goto https://example.com      # newest browser; --id <id> picks one
oya ask "Find the pricing page and summarize the plans"   # needs a model, set with `oya init`
oya status                        # health and recent commands
oya open                          # the live view, for a human
oya rm <id>                       # stop it; `oya rm --all` stops everything
```

Add `--json` to any command for machine-readable output.

## In code (SDK)

```ts
import { Oya } from "@oya-ai/browser";

const oya = new Oya(); // reads OYA_API_KEY
const browser = await oya.browser.start({ persona: "auto", captcha: "auto" });
try {
  await browser.goto("https://example.com");
  const { markdown, elements } = await browser.analyze();
  const link = elements.find((e) => e.visible && e.text?.includes("More information"));
  if (link) await browser.click(link.id);
} finally {
  await browser.stop();
}
```

`browser.cdpUrl` connects Playwright or Puppeteer: `chromium.connectOverCDP(browser.cdpUrl)`.

## Reading analyze_page

A header (url, title, viewport, scroll position, element counts), the page as markdown with elements inline, then an index split into visible and off-screen:

```
[#9 input:text placeholder="Search"]   → type(9, "query")
[#13 button "Search"]                  → click(13)
[#4 link "Pricing" → /pricing]         → click(4)
```

Off-screen elements need a `scroll` first. While a modal is open, the analysis is scoped to it.

## Patterns

- **Forms**: type into each field, click submit, then analyze to confirm it worked.
- **Dropdowns**: click to open, analyze, click the option.
- **Infinite scroll**: `scroll("down", 800)` returns a fresh analysis.
- **Nothing clickable by id**: take a `screenshot`, then `click_coordinates(x, y)`. Hover with `mouse_move` to reveal menus.

## Personas

- `"auto"` picks the least recently used persona under its concurrency cap; `"default"` is the key's own.
- A persona's device never changes. Don't try to refresh a fingerprint; for another device of the same kind, clone the persona (`oya personas clone <id>`).
- Logged-in state lives on the persona. If the user signed in through the Oya desktop app, start on that persona and the site is already logged in.

## CAPTCHA, MFA and humans

- `captcha: "auto"` (SDK) solves CAPTCHAs as they appear; `browser.solveCaptcha()` solves one on demand.
- `browser.completeMfa()` enters a TOTP code when the persona has one sealed. If it returns a `liveViewUrl`, a person has to approve (push, passkey, SMS): give the user that URL and wait.
- CLI takeover: `oya takeover <id>`, the human works in `oya open --id <id>`, then `oya release <id>` and `oya resume <id>`.
- Never type the user's passwords or codes into a page unless they gave them to you for that site.

## Rules

1. Analyze before acting; never guess an element id.
2. Re-analyze after every page change.
3. Navigate straight to URLs instead of clicking through menus.
4. Stop every browser you start.
5. "Every persona is at its concurrency cap" (429): stop a browser or ask the user to raise the cap. Don't retry in a loop.
6. "Element not found": the ids are stale, analyze again. A page that won't load: retry `navigate` once, then `wait` for a selector or take a screenshot.

Docs: https://browser.getoya.ai/docs · For agents: https://browser.getoya.ai/llms.txt
