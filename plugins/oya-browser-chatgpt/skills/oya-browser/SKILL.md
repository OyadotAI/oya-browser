---
name: oya-browser
description: Use Oya Browser when the user asks to open, read, compare, fill in, test or screenshot a website. It starts a real Chrome browser in Oya Cloud that the user can watch live and take over.
---

# Oya Browser

Oya gives you a real Chrome browser for the user's task. The user can watch it in the browser card in the chat and take over at any time.

## How to work

1. Call `start_browser` once, with `url` when you know where to go. The browser card shows the page live; mention that the user can take over from it.
2. Read the page with `analyze_page`. It returns the page as text with numbered elements. Act on those numbers with `click`, `type`, `select_option` and `press_key`, then read again: numbers change when the page changes.
3. Use `navigate` to go to a new address, `scroll` to see more, and `wait_for` when a page loads slowly. `screenshot` returns what the page looks like.
4. Call `stop_browser` when the task is done. A running browser uses a session on the user's Oya plan.

## Care

- Work only on sites and accounts the user asked about. Before anything that submits, buys, sends or deletes, stop and ask the user.
- If a page needs the user (a sign-in, a code, a payment), tell them to press Take over in the browser card and do it themselves.
- If a tool says a person has control, wait for the user to hand it back.
- Report what the page says; do not guess at content you did not read.
