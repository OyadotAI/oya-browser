# Product design

For a new surface, a redesign, or anything that looks dated or machine-made, also read
`design-modern.md`: how to choose a direction, what reads as current, what dates a design, and
the templates in `../assets/templates/` a new surface starts from.

## Hard limits

These hold for every surface: an app, a landing page, a dashboard, an email, CLI output with
colour. A design that breaks one is not finished. Where the product already breaks one, say so
with the count and keep the change inside the limit rather than adding to the breach. Fill in
the limits table of `../assets/design.md` with measured values, never "ok".

### Colour: five at most

- Count hues, not tokens. Tints and shades of one hue are one colour; dark mode is the same
  five remapped, never a second palette.
- The five: **one neutral** (background, surface, border, text and muted text are all steps of
  it), **one primary** (the brand and the main action), and at most three more. In an app those
  three are normally the states: error, success, warning. A landing page rarely needs more than
  the neutral, the primary and one accent.
- Roughly 60% neutral background, 30% neutral surface and text, 10% primary. The primary means
  "act here"; a screen where it appears on ten things has no primary action.
- Gradients, glows and coloured shadows count every hue in them. A photo or product screenshot
  does not count; an illustration drawn for the page does.
- Text on its background is at least 4.5:1; large text (24px, or 19px bold) and the outline of a
  control are at least 3:1. Do not round: 4.49 fails. Measure it, in both themes.
- Muted text is a darker or lighter step of the neutral that still passes 4.5:1. On a coloured
  background use a tint of that colour, never grey.
- Colour is never the only signal for a state: pair it with an icon, a label or a shape.

### Type: readable before stylish

- Two families at most, and one is usually right. Two or three weights. A scale of at most six
  sizes, each a step that is visibly different from its neighbours (about 1.2 to 1.333 apart).
- Body text is 16px or larger on websites and on mobile. A dense desktop tool may go to 14px for
  body and 12px for metadata; nothing a person must read is below 12px.
- Line height 1.5 for body and unitless; 1.1 to 1.3 for headings. Lines of prose run 45 to 75
  characters (`max-width: 65ch`); a paragraph the full width of a desktop window is unread.
- Body weight 400 or more. No thin (100 to 300) weights for text, no light grey for body, no
  paragraphs in capitals, centred or justified. Left-align anything longer than three lines.
- Hierarchy comes from size, weight and the neutral's steps together, not from more colours or
  more fonts.
- Sizes are in `rem`, so the person's own text setting works; the layout holds at 200% zoom.

### Speed: part of the design

A design that needs a slow page is a slow design. Specify these with the layout, because they
cannot be added afterwards.

- Budget, at the 75th percentile on a mid-range phone: largest content painted within 2.5s,
  an interaction answered within 200ms, layout shift 0.1 or less.
- What a person waits for: under 0.1s feels instant, so every press shows a pressed or pending
  state at once. Past 1s show a skeleton shaped like the result. Past 10s show progress and a
  way to cancel. Where the outcome is nearly certain, show it immediately and reconcile.
- The first screen is text and CSS. No carousel, no autoplaying video, no hero that waits on
  JavaScript, no animation library for a fade. The largest element above the fold is either
  text or one image that is preloaded, sized, and in AVIF or WebP.
- Fonts: the system stack costs nothing; otherwise one self-hosted variable WOFF2, subset,
  preloaded, with `font-display: swap` and a fallback matched in size so the swap does not move
  the page. Never more than two font files on the first screen.
- Everything that arrives later has its space reserved: `width` and `height` on images, a fixed
  box for embeds and ads, skeletons the size of the data.
- Animate `transform` and `opacity` only, 150 to 300ms, and nothing between a person and the
  thing they are waiting for. Honour `prefers-reduced-motion`.

### UX: the task, then the look

- One primary action per screen, visually unlike everything else. The rest are secondary or
  plain links. If two things compete, the design has not decided yet.
- Use the pattern people already know from other products (navigation where it is expected,
  links that look like links, a form that looks like a form). Novelty is spent on the product,
  not on its controls.
- Fewer choices decide faster: cut options, group the rest, set a sensible default.
- Targets are at least 24 by 24px, and 44 by 44px on touch. Destructive actions sit apart from
  the primary one and can be undone, or confirm by naming what is lost.
- People scan from the top left and read the first words of each line. Put the point first in a
  heading, a label, a button and an error. Left-align; break text with headings and short lists.
- A label beats an icon; an icon alone needs an accessible name and a tooltip. Words are the
  product's own, in sentence case, and the same thing has the same name everywhere.
- Forms: a visible label above each field (a placeholder is not a label), the right input type
  and `autocomplete`, validation when the field is left, an error beside the field saying how to
  fix it, and what was typed is never cleared.
- Space on a scale of 4 (4, 8, 12, 16, 24, 32, 48, 64). Things that belong together sit closer
  than things that do not; spacing groups before borders and boxes do.
- A landing page says what the product is and who it is for in one headline of about ten words,
  shows the real product, and asks for one thing, repeated, never three different things.

### What marks a design as unconsidered

Do not hand off any of these: a purple-to-blue gradient, gradient text, glass blur as
decoration, emoji used as icons, three identical icon-title-text cards because a grid was the
default, everything centred, a different accent colour per section, invented testimonials,
logos or numbers, and placeholder names or lorem ipsum presented as content.

## The work

1. Inspect the rendered surface and the current component code. Inventory existing tokens,
   typography, controls, interaction patterns, and brand assets, and count the colours, families
   and sizes already in use against the limits. Read the project's frontend skill when
   available. Reuse working patterns before adding another visual language.
2. Describe the task flow, hierarchy, primary action, and recovery path. For each changed region,
   map empty, loading, partial, success, denied, failed, disconnected, and long-content states.
   Show what action remains available in each; avoid badges that imply an unavailable fix.
3. Specify concrete layout, spacing, labels, truncation, overflow, and responsive behavior. Check
   the smallest supported app window as well as a comfortable desktop size; for a website check
   375, 768 and 1440px. Dense tools need readable rows, quiet metadata, and controls that retain
   their labels under pressure.
4. Check keyboard order, visible focus, escape/dismiss behavior, accessible names, semantic
   controls, contrast, reduced motion, and light/dark themes. Trace focus after dialogs and
   asynchronous updates.
5. Inspect actual screenshots through Oya for browser surfaces. A browser fixture of native
   components cannot establish native integration correctness. Identify the evidence type.
6. Hand off `../assets/design.md` with implementable values and state transitions, the limits
   table filled in, and existing image/icon assets to reuse. Name every colour as a token with
   its value and its job. Generate or request missing imagery only when it serves the task;
   never disguise a placeholder as a finished brand asset.

Return a design or findings. In a review, a broken limit is a finding with the measured value
and the change that fixes it. Change implementation only when explicitly assigned that work.
