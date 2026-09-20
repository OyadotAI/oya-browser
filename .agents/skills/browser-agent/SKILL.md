---
name: browser-agent
description: Control a real browser via Oya Browser MCP tools. Use this skill when the user asks you to browse the web, interact with websites, fill forms, scrape data, or automate browser tasks. Provides the mental model and strategies for effective browser automation through structured markdown and numbered element IDs.
---

You have access to a real Chrome browser through Oya Browser MCP tools. The browser has real cookies, logins, extensions — sites see a real user, not a bot. You control it through structured commands, not raw HTML or CSS selectors.

## Core Mental Model

The browser gives you pages as **structured markdown with numbered interactive elements**. You act by referencing element numbers:

```
[#9 input:text placeholder="Search"]   →   type(9, "query")
[#13 button "Google Search"]           →   click(13)
[#4 link "Images" → /images]          →   click(4)
```

**You never write CSS selectors, XPath, or pixel coordinates** (unless using coordinate tools for precise clicks). You analyze the page, read the markdown, and act by element ID.

## Workflow

Every browser task follows this loop:

1. **Navigate** — go to the URL
2. **Analyze** — call `analyze_page` to see the page as markdown with numbered elements
3. **Act** — click, type, scroll, press keys using element IDs
4. **Re-analyze** — after actions that change the page (clicks, form submissions, scrolls), analyze again to see the new state
5. **Repeat** until the task is done

**CRITICAL**: Always call `analyze_page` after navigation or significant interactions. Element IDs change between analyses. Never assume an old ID is still valid.

## Available Tools

### Page Understanding
- **`analyze_page()`** — Returns full page as markdown with `[#id type "label"]` annotations. Always start here.
- **`screenshot()`** — Capture visible viewport as PNG. Use when you need to verify visual state or when markdown isn't enough.
- **`read_elements(selector?, limit?)`** — Lighter than analyze_page, returns element metadata without full markdown.

### Navigation
- **`navigate(url)`** — Go to a URL. Wait for page load automatically.
- **`scroll(direction, amount?)`** — Scroll `"up"` or `"down"` by N pixels (default 500). Returns updated page analysis.
- **`wait(selector, timeout?)`** — Wait for a CSS selector to appear (max 10s default).

### Element Interaction
- **`click(element_id)`** — Click an element by its ID from `analyze_page`. Scrolls element into view automatically.
- **`type(element_id, text)`** — Clear field and type text into an input/textarea/contenteditable.
- **`press_key(key)`** — Press Enter, Escape, Tab, ArrowDown, ArrowUp, Backspace, or any key.

### Mouse & Keyboard (Low-Level)
- **`click_coordinates(x, y)`** — Click at exact pixel position. Use with screenshot when element IDs don't work.
- **`mouse_move(x, y)`** — Hover without clicking. Triggers tooltips, dropdown menus, hover states.
- **`double_click(element_id?, x?, y?)`** — Double-click by element ID or coordinates. For text selection, file opening.
- **`keyboard_type(text)`** — Type into whatever is currently focused. No element targeting.
- **`drag(from_x, from_y, to_x, to_y)`** — Drag between two points. For sliders, drag-and-drop, range selection.

### Tab Management
- **`list_tabs()`** — Show all open tabs with IDs.
- **`open_tab(url?)`** — Open new tab, optionally navigating to URL.
- **`switch_tab(tab_id)`** — Switch to a tab by ID.
- **`close_tab(tab_id?)`** — Close a tab (default: active tab).

### Pool (Multi-Browser)
- **`pool_status()`** — Show connected browsers in the pool.

## Strategy Guide

### Reading `analyze_page` Output

The output has two parts:

**1. Header** — metadata about the page:
```
---
url: https://example.com
title: Example
viewport: 1280x720
scroll: 0% (0px / 3200px)
elements: 47 total, 23 visible
---
```

**2. Body** — full page as markdown with inline element annotations:
```
# Welcome to Example

[#1 link "Home" → /] [#2 link "About" → /about] [#3 link "Login" → /login]

## Search
[#9 input:text placeholder="Search..."]
[#10 button "Search"]

## Results
- [#11 link "First result" → /result/1]
- [#12 link "Second result" → /result/2]
```

**3. Element Index** — grouped by visibility:
```
### Visible
  [#9] input: Search...
  [#10] button: Search
### Off-screen (scroll to reveal)
  [#35] link: Footer link
```

### Handling Common Patterns

**Forms**: Find the input elements, type into each, then click submit:
```
analyze_page → find [#5 input:email], [#6 input:password], [#7 button "Sign in"]
type(5, "user@example.com")
type(6, "password123")
click(7)
```

**Dropdowns/Selects**: Click to open, then click the option:
```
click(dropdown_id)
analyze_page → find the option elements that appeared
click(option_id)
```

**Infinite scroll**: Scroll down, re-analyze to see new content:
```
scroll("down", 800)
→ returns updated analysis with new elements
```

**Modals/Dialogs**: When a modal is open, `analyze_page` automatically scopes to the modal content. The header will say `modal: "dialog name"`.

**Pagination**: Look for "Next", "More", or page number links in the element index.

### Site-Specific Tips

**LinkedIn**:
- Feed posts use `data-urn` attributes — the analyzer detects these
- Messaging opens with `aria-hidden` on main content — the analyzer handles this
- Use the search bar input, type query, press Enter
- Connection buttons: look for "Connect" or "Follow" button elements

**X (Twitter)**:
- Tweet compose: click the "Post" or compose area, then type
- Interactions: like/retweet/reply buttons detected via `data-testid`
- Thread navigation: click "Show replies" or individual tweets

**Reddit**:
- Uses web components (`<shreddit-post>`) — the analyzer traverses shadow DOM
- Vote buttons and comment links are properly detected
- Subreddit navigation: use the search or navigate directly

**HackerNews**:
- Layout tables are handled as containers (not data tables)
- Vote arrows show as `[#N button "upvote"]`
- Comment reply links show as `[#N link "reply to username"]`
- Comment links show context: `[#N link "189 comments" on "Post Title"]`

**Amazon**:
- Product cards detected via `data-asin` and `data-cel-widget`
- Search: type in the search box, press Enter
- Add to cart: find the "Add to Cart" button element

### When Element IDs Don't Work

Sometimes elements aren't detected or clickable by ID. Fallback strategy:

1. **Take a screenshot** → visually identify the target
2. **Use `click_coordinates(x, y)`** → click at the pixel position you see in the screenshot
3. **Use `mouse_move(x, y)`** → hover first to reveal hidden elements (dropdowns, tooltips)
4. **Use `keyboard_type(text)`** → type into whatever is focused without targeting

### Error Recovery

- **"Element not found"** → IDs are stale. Call `analyze_page` again.
- **Click didn't work** → The element may need scrolling first, or try `click_coordinates` with screenshot.
- **Page didn't load** → Check URL, try `navigate` again, or `wait` for a specific selector.
- **Modal blocking** → `analyze_page` will scope to the modal. Dismiss it with Escape or clicking X.
- **Content truncated** → Very long page. Scroll down and re-analyze to see more content.

## Important Rules

1. **Always analyze before acting.** Never guess element IDs.
2. **Re-analyze after major actions.** Element IDs change after page changes.
3. **Scroll to find elements.** If an element is in the "Off-screen" index, scroll first.
4. **Read the element index.** It tells you exactly what's visible and what's not.
5. **Use screenshot as backup.** When markdown isn't enough, see the page visually.
6. **Don't over-analyze.** If you already see the element you need, act immediately.
7. **Be efficient.** Navigate directly to URLs when possible instead of clicking through menus.
