---
name: frontend
description: "Use when building or changing anything in the frontend: a page, a component, a layout, styling, copy on screen. Produces distinctive, production-grade UI that fits this project's existing design, covers every state, is accessible, and is checked in a browser before it is called done."
---

# Frontend

Build interfaces that look designed for this product, work in every state, and are verified by
looking at them.

## 1. Read before drawing

- Find the project's design system before inventing one: tokens (CSS variables, Tailwind config,
  theme file), the component library, how an existing page of the same kind is built. Grep for
  the pattern you are about to write; reuse it. Two ways to draw a button is one too many.
- Read the repository instructions (`CLAUDE.md` or `AGENTS.md`) for who the users are. A dashboard for engineers wants density; a landing page
  wants one idea per screen.

## 2. Commit to a direction

When there is no system yet, or the ask is a new surface, choose a clear aesthetic direction and
execute it with precision — refined minimalism and bold maximalism both work; intentionality is
the point. Never the generic AI look:

- choose typography for the product; preserve established app fonts and use native system fonts
  where appropriate; introduce a display face only when the content benefits from one
- no purple-to-blue gradient on white, no evenly spread timid palette: one dominant colour, sharp
  accents, and colour reserved for meaning
- no card grid because it is the default; compose for the content — asymmetry, overlap, generous
  space or deliberate density
- colours, spacing, radii and type sizes are tokens (CSS variables), never literals scattered
  through components

## 3. Every state, not the happy one

For each surface, design and build: empty, loading, partial, error (with what to do next),
permission denied, offline or stale, and far more data than expected (long names, 10,000 rows).
A blank area with no reason is a bug. Skeletons match the final layout so nothing jumps.

## 4. Accessible by default

Semantic elements first (`button`, `a`, `label`, `nav`, headings in order). Everything works from
the keyboard with visible focus. Text contrast at least 4.5:1 in light and dark. Images have alt
text; icon-only buttons have labels. Honour `prefers-reduced-motion` and `prefers-color-scheme`.

## 5. Motion and performance

Motion carries information — an arrival, a change, a relationship — or it goes. Prefer CSS
transitions; animate `transform` and `opacity` only. Nothing animates between a person and the
thing they are waiting for. Images are sized and lazy below the fold; fonts are subset and
`font-display: swap`; no layout shift when data lands.

## 6. Responsive

Check the supported minimum width and a comfortable desktop width with no accidental horizontal
scroll. For responsive websites, check 375, 768 and 1440px. Use at least 44px touch targets on
touch interfaces; keep desktop controls consistent with the existing application.

## 7. Look at it

Read the `oya-browser` skill and use Oya Browser exclusively. If Oya is unavailable,
report the blocker without substituting another browser or harness. Run the dev server and open
the page in an owned Oya tab. Take a screenshot at each width and in each theme and look
at it; read the console for errors and warnings. Check the empty and error states by forcing
them. A UI change is not done until you have seen it render — the type checker cannot see a
button that is white on white.

## Report

Say which direction you chose and why, the states you built, what you verified by looking (with
the widths and themes), and anything you could not check.
