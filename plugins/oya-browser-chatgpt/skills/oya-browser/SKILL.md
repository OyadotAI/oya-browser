---
name: oya-browser
description: Use Oya Browser when the user asks to open, read, compare, fill in, test or screenshot a website. It gives you a real Chrome browser in Oya Cloud that the user can watch live.
---

# Oya Browser

Oya gives you a real Chrome browser for the user's task. There is nothing to start or stop: the browser starts itself on your first call (this can take up to 2 minutes) and stops when it sits idle.

## How to work

1. Call `open_tab` with the address to start. The user gets a live card of the browser in the chat; its **Watch live** action opens a viewing link.
2. Read the page with `analyze_page`. It returns the page as text with numbered elements. Act on those numbers with `click`, `type` and `press_key`. Numbers stay valid until you analyze again or the page navigates: fill a whole form from one `analyze_page`, and analyze again only after a click changes the page.
3. Use `navigate` to load another address in the same tab (it also takes "back" or "forward"), `scroll` to see more, and `wait_for` when a page loads slowly. `list_tabs` shows the open tabs; `open_tab` with a tab id switches to one.
4. When asked for screenshots, call `screenshot` at each requested point in order, with a short page caption and a capture number for a series. With no target specified, capture the current page once. Report any failed capture.
5. If a tool says the browser is still starting, repeat the same step shortly. If it says a browser could not be started, tell the user the reason it gives.

## Care

- Work only on sites and accounts the user asked about. Before anything that submits, buys, sends or deletes, stop and ask the user.
- Pop-up dialogs are dismissed for you with Cancel and reported in the result. If the user needs one accepted, tell them.
- If a page needs the user (a sign-in, a code, a payment), tell them to open the browser in their Oya dashboard, select Take control and do it themselves.
- If a tool says a person has control, wait for the user to hand it back.
- Report what the page says; do not guess at content you did not read.
