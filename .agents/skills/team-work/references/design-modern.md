# Designing something current

Read this when the assignment is a new surface, a redesign, or a review of something that looks
dated or machine-made. The hard limits in `designer.md` still bind; this is how to be good
inside them. The templates in `../assets/templates/` are where a new surface starts.

## 1. Decide the direction before any layout

Write three lines first, and put them at the top of the handoff:

- **Who and what for**, from the repository instructions and the product itself.
- **The direction**, in one sentence a stranger could picture: "a quiet, dense tool for people
  who live in it all day", "an editorial page that reads like a well-set magazine", "a warm,
  plain-spoken consumer app". The subject chooses it. A default chooses nothing.
- **The one thing a visitor remembers.** One: a headline, the product itself on screen, a single
  unusual layout decision. Everything else is quiet so that this is not.

Then look at two or three real products in the same field through the browser, and name what
they do that you will and will not do. Designing from memory produces the average of memory.

## 2. What reads as current

**Type carries the design.**
- A large, tight headline: `clamp()` for size so it scales without breakpoints, letter-spacing
  about `-0.02em` on display sizes, line height near 1.05, `text-wrap: balance`. The headline is
  three to five times the body size; a timid 1.5 times reads as a template.
- Body `text-wrap: pretty`. Numbers in columns `font-variant-numeric: tabular-nums`.
- One face chosen for this product, and the system stack is a legitimate choice. A second face
  only for a real job (display, or code). The same geometric sans on everything, untouched, is
  the look of nobody having chosen.
- Typographic care: curly quotes, the ellipsis character, a non-breaking space in "10 MB".

**Colour is quiet until it means something.**
- Define colours in `oklch()`: steps are even to the eye, and a ramp is one hue at several
  lightnesses. Tint the neutral slightly toward the primary (chroma 0.005 to 0.02) so greys
  belong to the brand. Not pure `#000` on pure `#fff`.
- One primary, used for the action and almost nothing else. Large areas are neutral.
- Light and dark from one set of tokens with `light-dark()` and `color-scheme`; both are
  designed, neither is an inversion.

**Depth is restrained.**
- A hairline border in a low-contrast neutral step, or one soft layered shadow. Not both on the
  same element, and no shadow on something that does not float.
- One radius scale. A nested corner is the outer radius minus the padding between them.

**Layout has intent.**
- Compose for the content: an asymmetric split, one large element beside several small ones,
  text columns of different widths. A grid of unequal cells only when the cells hold content of
  genuinely different weight; equal cards for unequal ideas is the default grid again.
- Space generously and unevenly: sections do not all get the same padding, and the rhythm marks
  what matters. Content is capped (about 72rem; prose 65ch) and sits on a consistent edge.
- Components respond to their container (`@container`), pages to the viewport.

**The product is shown.**
- The real interface, a real screenshot or a live component, in the first screen. Not an
  abstract blob, a 3D shape, or an illustration of people pointing at charts.
- Real copy, written from the visitor's side, specific to this product. Specific button labels
  ("Create invoice", not "Get started"). No invented customers, numbers or logos: where proof
  does not exist yet, the section does not exist yet.

**Motion is rare and meaningful.**
- One composed entrance for the page at most. After that, motion answers the person: a press, a
  state change, an item arriving. 150 to 300ms, ease-out, interruptible, `transform` and
  `opacity` only, never `transition: all`.
- Navigation may use view transitions as an enhancement. No scroll-jacking, no parallax, no
  fade-and-rise on every section, no hover animation on every card.

**Apps feel fast and direct.**
- Sidebar or top navigation that stays put, content that fills the rest, density suited to
  someone who uses it daily. Tables and lists before cards for anything compared row by row.
- The URL holds filters, tabs and pagination. Updates appear at once and reconcile. Keyboard
  reaches everything; frequent actions have shortcuts and a command palette when there are many.
- Empty states say what goes here and offer the action that fills it.

**Details that separate finished from generated.**
- Hover, active, focus-visible and disabled states on every control, each more prominent than
  rest. One icon set at one stroke width. Text truncates or wraps on purpose (`min-width: 0` on
  flex children). Dates and numbers through `Intl`. A favicon, a title and a share image.

## 3. Platform features to reach for

Prefer the platform to a library, and check Baseline for the project's browser targets before
relying on the newer ones; treat those as enhancement with a working fallback.

`oklch()` and `color-mix()`; `light-dark()`; `clamp()`; CSS nesting; `:has()`; container
queries; `subgrid`; `text-wrap`; `dvh` units; `<dialog>`, `popover` and `<details>` instead of
hand-built overlays; `@starting-style` for entry transitions; view transitions; scroll-driven
animations (enhancement only).

## 4. What dates a design

Each is a default nobody decided on. Finding one in your own draft means going back to step 1.

- **Template era:** a hero carousel, parallax, stock photography of meetings, heavy drop
  shadows, gradient buttons, three centred icon features, a hamburger menu on a wide screen,
  "Welcome to" headlines.
- **Generated, first wave:** purple-to-blue gradients, gradient text, glass blur, emoji as
  icons, a sparkle icon for anything, a pill badge announcing "New" above the headline, floating
  blurred blobs, neon glow on dark, identical rounded cards with the same faint shadow, an
  untouched component-library theme, twin "Get started" and "Learn more" buttons, cards inside
  cards, a rounded icon tile above every heading.
- **Generated, second wave:** cream background with a serif display and a terracotta accent;
  near-black with one acid-green accent; tracked all-capitals eyebrow labels above every
  heading; monospace for small labels; 01 / 02 / 03 numbering on things that are not a sequence;
  one highlighted word in each headline; a trailing arrow on every link; a big number over a
  small label as the hero.

Borders, numbers, rules and labels are allowed when they carry information. They are dated when
they are decoration.

## 5. Templates

Start a new surface from these and change them; an unchanged template is another default.
They fix structure, tokens and the limits. Hue, typeface, composition and copy are chosen for
the product.

| File | Use |
| --- | --- |
| `../assets/templates/tokens.css` | The token set: five hues in OKLCH, light and dark, type scale, spacing, radius, motion. Change `--hue` and the font first. |
| `../assets/templates/landing.html` | A landing page: headline, product shown, one ask, proof only where it is real. |
| `../assets/templates/app.html` | An app screen: navigation, a data view, and its empty, loading and error states. |

In a project with its own framework and components, take the structure and the tokens, and
build with the project's components. Do not paste raw HTML beside an existing system.

## 6. Before handing off

Look at the result at 375, 768 and 1440px in both themes, then:

- Cover the logo. Could this be any product? Then the direction did not reach the page.
- Squint. One thing should stand out, and it should be the primary action or the product.
- Remove one element. If nothing is lost, leave it out, and try again.
- Check section 4 line by line against the screenshot, not against the intent.
