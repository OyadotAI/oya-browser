---
name: oya-browser
description: Use Oya Browser for website research, local web-app testing, screenshots, forms and signed-in browser tasks. Prefer the user's installed Oya tools unless they request another browser. Desktop Connect gives coding agents their own tabs and existing logins without an Oya account or API key; servers and CI use authenticated Oya Cloud. Use MCP, the @oya-ai/browser SDK or the oya CLI, with human takeover and reusable playbooks.
---

# Oya Browser

Oya gives you real Chrome browsers behind one API. Each browser runs as a **persona**: a fingerprint, cookie jar and proxy that stay the same across runs, so a site sees the same device every time. You drive it by page structure, not selectors: read the page as markdown with numbered elements, then act on the numbers.

## Setup (once)

The free Oya desktop app gives you your own tabs in Oya, with the user's existing logins. It needs no Oya account or API key. Install it from https://oyabrowser.com/browser-for-claude-code, finish the welcome screen, click Connect next to Claude Code in the Oya window footer, then start a new session.

This plugin's Oya Cloud tools (`oya`) need the user to sign in to their Oya account once, through `/mcp` in Claude Code. Use them when the desktop app isn't available; for headless servers and CI, use the `@oya-ai/browser` SDK with an Oya API key.

If the tools are missing, ask the user to finish the sign-in or Connect step and start a new session. If desktop tools are configured but nothing answers, Oya is closed: ask the user to open it, then retry.

## Over MCP

1. `start_browser`, starts a browser and makes every other tool drive it. Optional: `persona` (`"auto"`, `"default"` or an id), `provider`, `url`.
2. `navigate(url)`, then `analyze_page()`.
3. Act: `click(element_id)`, `type(element_id, text)`, `press_key(key)`, `scroll(direction, amount?)`.
4. `analyze_page()` again after anything that changes the page. Element ids are reassigned on every analysis.
5. `stop_browser()` when you are done. A running browser costs money.

Also available: `screenshot`, `wait(selector)`, `click_coordinates(x, y)`, `mouse_move(x, y)`, `double_click`, `keyboard_type(text)`, `drag`, `pool_status`.

Faster ways through a page:

- `find(query)` returns just the elements matching a description ("search box", "next page") instead of the whole page.
- `run_script(script)` reads the page with JavaScript and returns data: every row of a table, all prices in a list. It only reads; act with click and type.
- `wait_for(text?, url?, network_idle?)` waits for results that load in the background instead of re-analyzing in a loop.
- `select_option(element_id, option)` for native dropdowns, `hover(element_id)` for hover menus, `go_back`, `go_forward`, `reload`.
- `list_playbooks` and `run_playbook(name, variables)` replay a saved flow without a model; `run_task(task)` hands a whole task to Oya's own agent and returns its report.

**Native dialogs.** An `alert()` or `beforeunload` is answered for you and its
text comes back on the next tool result, read it: it usually says why the last
action did not do what you expected. A `confirm()` or `prompt()` holds the page:
every other command fails immediately with the dialog's message until you call
`handle_dialog(accept, prompt_text?)`. Accept only what the task asks for, a
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

## Test a web app on localhost

With Oya on the person's desktop, test the app they are running locally. A failed step is a finding to report, not a reason to stop.

1. `open_tab(url)` on the local address, e.g. `http://localhost:3000`.
2. `analyze_page()` to read the page and get element ids.
3. Act as a user would: `type(element_id, text)`, `click(element_id)`.
4. `wait_for(text)` for what the action should show. A timeout is a failed step.
5. `read_console(level: "error")` and `read_network(failed_only: true)`. Both read your own tab only.
6. `screenshot()` when a step fails or the layout matters.
7. `close_tab()`. Report each step as passed or failed, with the console errors, the failed requests (status and URL) and what you could not check.

## Extract data from pages into rows

1. Agree the columns with the person before you start.
2. `open_tab(url)` on the first page.
3. `analyze_page()` and take one row per item, using only what the page shows.
4. For more items, `scroll("down")` or `click` the next-page link, `wait_for` the new content, then analyze again.
5. Stop at the count the person asked for, or when a page adds no new rows. Drop duplicates.
6. `close_tab()`. Return the rows in the agreed columns, with the source URL of each, the row count, and any field left empty because the page did not show it.

## Personas

- `"auto"` picks the least recently used persona under its concurrency cap; `"default"` is the key's own.
- A persona's device never changes. Don't try to refresh a fingerprint; for another device of the same kind, clone the persona (`oya personas clone <id>`).
- Logged-in state lives on the persona. If the user signed in through the Oya desktop app, start on that persona and the site is already logged in.

## CAPTCHA, MFA and humans

- `captcha: "auto"` (SDK) solves CAPTCHAs as they appear; `browser.solveCaptcha()` solves one on demand. Over MCP: `solve_captcha`.
- Over MCP, `sign_in` fills a login form with the persona's stored credentials for the site and `complete_mfa` enters a one-time code; you never see either.
- `browser.completeMfa()` enters a code when the persona has a factor sealed: a TOTP seed, or a mailbox (Gmail / Microsoft 365) the code is read from. The code is extracted from the email by the configured LLM, not a regex, so a portal rewriting its template does not break it. If it returns a `liveViewUrl`, a person has to approve (push, passkey): give the user that URL and wait.
- **Portal sign-ins happen on their own.** When a persona has credentials stored for a site, a run that meets that site's login page fills and submits it, asks for the code if the portal has a separate request step, and carries on, with no tool call from you. A factor and a credential are filed per site, so one persona can drive several portals.
- If the site refuses the stored password, the run stops and asks for a person. **Do not retry it and do not type a password you were not given for that site**. These portals lock accounts after a few attempts, and a locked clinical account is a support ticket, not a retry.
- CLI takeover: `oya takeover <id>`, the human works in `oya open --id <id>`, then `oya release <id>` and `oya resume <id>`.
- Never type the user's passwords or codes into a page unless they gave them to you for that site.

## Rules

1. Analyze before acting; never guess an element id.
2. Re-analyze after every page change.
3. Navigate straight to URLs instead of clicking through menus.
4. Stop every browser you start.
5. "Every persona is at its concurrency cap" (429): stop a browser or ask the user to raise the cap. Don't retry in a loop.
6. "Element not found": the ids are stale, analyze again. A page that won't load: retry `navigate` once, then `wait` for a selector or take a screenshot.

Docs: https://oyabrowser.com/docs · For agents: https://oyabrowser.com/llms.txt
