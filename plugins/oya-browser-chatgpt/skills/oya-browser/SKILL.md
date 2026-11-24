---
name: oya-browser
description: Use Oya Browser when the user asks to open, read, compare, fill in, test or screenshot a website. It starts a real Chrome browser in Oya Cloud that the user can watch live and take over.
---

# Oya Browser

Oya gives you a real Chrome browser for the user's task. The user can watch it in the browser card in the chat after each browsing request.

## How to work

1. Reuse the browser from the current conversation. If a new browser is needed, call `start_browser` once and keep its browser ID.
2. Read the page with `analyze_page`. It returns the page as text with numbered elements. Act on those numbers with `click`, `type`, `select_option` and `press_key`, then read again: numbers change when the page changes.
3. Use `navigate` to go to a new address, `scroll` to see more, and `wait_for` when a page loads slowly. When asked for screenshots, call `screenshot` at each requested point in order, with a short page caption and a capture number for a series. With no target specified, capture the current page once. Report any failed capture.
4. After the final browser action in each browsing turn, call `show_browser` once with the ID used for that request. The card’s **Watch live** action opens a viewing link.
5. When asked to finish the session, call `stop_browser` and skip the live card. A running browser uses a session on the user's Oya plan.

## Care

- Work only on sites and accounts the user asked about. Before anything that submits, buys, sends or deletes, stop and ask the user.
- If a page needs the user (a sign-in, a code, a payment), tell them to open the browser in their authorized Oya dashboard and select Take control and do it themselves.
- If a tool says a person has control, wait for the user to hand it back.
- Report what the page says; do not guess at content you did not read.
