# Console architecture

The console is the dashboard and the public site (landing page, docs, sign-in),
built with Next.js 16 (App Router), React 19, Tailwind 4 and TypeScript. It is
a client of the server's REST API like any other: it holds a project credential
and calls `/api`. It shares no code with the server. In production the server
runs the standalone build and proxies to it.

This Next.js version differs from older ones (for example, middleware is now
`proxy.ts`). Read the guide in `node_modules/next/dist/docs/` before changing
app-router files, `proxy.ts` or metadata.

The standards every part shares (size limits, documentation, no magic numbers)
are in the root [ARCHITECTURE.md](../ARCHITECTURE.md) and enforced by
`npm run lint`. React components may be 50 lines; their logic lives in hooks,
which, like every other function, get 10.

## Layout

```
src/
  proxy.ts               per-request CSP nonce (Next.js 16's name for middleware); the policy is lib/csp.ts
  app/                   routes (App Router)
    layout.tsx           fonts, site metadata, structured data, theme script, AuthProvider
    page.tsx             landing page; its sections live in _home/
    _home/               hero, providers, control plane, console, developers, header, footer, content.ts
    docs/
      page.tsx           docs shell: header, sidebar (a dialog on mobile), sections
      _docs/             the machinery: nav.ts (active section, navigate, "/"), search-index.ts,
                         use-docs-search.ts, sidebar.tsx, blocks.tsx (SectionHeading, CodeBlock, Table…)
      _sections/         the content, one file per sidebar group (control plane, getting started, …)
    dashboard/
      layout.tsx         the gate: an account or a working console credential, else /login
      error.tsx          the console's error boundary
      page.tsx           the fleet console; renders from useConsole()
      _console/          its state and parts (below)
    live/[browserId]/    one browser's live view; live-session.ts (plain logic), use-live-page.ts (hooks)
    login/, signup/      sign-in and sign-up; each has its hook (use-login.ts, use-signup.ts) and forms
    robots.ts, sitemap.ts
  components/
    auth-provider.tsx    facade: AuthProvider and useAuth (the session logic is lib/auth)
    auth/                shared by sign-in and sign-up: AuthShell, fields, useFormState/runSubmit
    ui/                  dialog.tsx (+ use-dialog.ts), context-menu.tsx (+ use-context-menu.ts), kbd, syntax-code
    copy-example, oya-logo, theme-toggle, workflow-diagram (+ workflow-stages.ts)
    dashboard/           the console's components (below)
  lib/
    api.ts               API URL, auth headers, account endpoints, the tab's console credential
    api-client.ts        api(): every project request; ApiError; errorMessage (re-exports ago, shortId)
    format.ts            ago(), shortId()
    live-stream.ts       subscribeFrames(): ticketed EventSource that reconnects
    shortcuts.ts         useShortcuts() and keyCaps()
    csp.ts               the Content-Security-Policy
    auth/                the session: types, storage, token timing, session steps, use-auth-session
    hooks/               use-patch-state (object state changed by merging patches)
    http-status.ts       Status: HTTP codes by name
    constants.ts         the library's numbers
    site.ts, browser-downloads.ts
tests/
  unit/                  Vitest + Testing Library; mirrors src/ (src/a/b.ts → tests/unit/a/b.test.ts)
    support.ts           fakeFetch, a scripted fetch
  *.spec.ts              Playwright end-to-end, against a running console
```

## Patterns

- **Facades.** Other code imports a module's entry file: `lib/api-client.ts`,
  `components/auth-provider.tsx`, `components/ui/dialog.tsx`, and in the
  dashboard the top-level `<name>.tsx` files. Their exported names are part
  of the contract and stay stable when the insides move.
- **Hooks for logic, components for layout.** A component that needs more
  than a few lines of logic gets a hook beside it (`use-dialog.ts`,
  `use-login.ts`, `use-live-page.ts`). A hook does little itself: it holds
  state and wires plain functions (`live-session.ts`, `feeds.ts`,
  `session.ts`) to React. Those plain functions are where the unit tests
  aim.
- **Command maps.** Keys and modes dispatch through tables guarded with
  `Object.hasOwn`: the console shortcuts (`dashboard/_console/shortcuts.ts`),
  menu keys (`ui/use-context-menu.ts`), diagram tabs, key caps, live-view
  control steps.
- **Private folders.** Route-specific code sits beside its route in
  `_folders` (`_home`, `_docs`, `_sections`, `_console`), which Next.js never
  routes.

## The console page (`app/dashboard/_console/`)

| File | Role |
|:---|:---|
| `use-console.ts` | Composes everything: credential and project, view, feeds, polling, effects, actions, shortcuts |
| `view.ts` | The view state (tab, selection, filter, open dialogs) as one object, changed with `patch` |
| `feeds.ts`, `use-feeds.ts` | Browsers, fleet, personas and config: loaders as plain functions, hooks around them |
| `polling.ts` | Each feed on its interval, paused while the tab is hidden |
| `actions.ts` | Stop, screenshot, a persona's browsers, the Slack return, selection pruning |
| `shortcuts.ts` | The keyboard shortcuts as a command table |
| `rate.ts` | Commands per minute and error share between two fleet polls |
| `console-body.tsx`, `console-dialogs.tsx` | The parts the page renders |

Every feed drops a response that arrives after the credential changed, so a
previous project's data is never painted over the new one. Only a change of
project clears the view; an hourly credential renewal does not.

## The dashboard components (`components/dashboard/`)

Each top-level file is a facade for one piece of the console, with its parts
in the folder of the same name: `browser-panel.tsx` → `browser/`,
`control-tab.tsx` and `durable-control.tsx` → `control/`, `fleet-table.tsx`
and `fleet-strip.tsx` → `fleet/`, `header.tsx` → `header/`, `live-view.tsx` →
`live/`, `onboarding.tsx` → `onboarding/`, `persona-drawer.tsx`,
`persona-form.tsx`, `personas-tab.tsx` and `proxies-dialog.tsx` →
`personas/`, `playbooks-tab.tsx` → `playbooks/`, `project-switcher.tsx` →
`projects/`, `settings-dialog.tsx` (with `slack-section.tsx` and
`webhook-section.tsx`) → `settings/`, `snippets.tsx` → `snippets/`,
`start-browser.tsx` → `start/`, `toast.tsx` → `toast/`. Inside each folder the
same split holds: `use-*.ts` hooks for state, plain modules for API calls and
rules (`persona-api.ts`, `requests.ts`, `model.ts`), `constants.ts` for
numbers, `types.ts` for shapes. `hooks/` has the hooks several pieces share
(`use-busy-action`, `use-outside-click`, `use-desktop-sign-in`); `types.ts` and
`config.ts` hold the API's shapes and the key's configuration.

## Security

- **CSP with a nonce** (`proxy.ts`, `lib/csp.ts`): scripts run only with the
  per-request nonce; the two inline scripts in `app/layout.tsx` read it back
  from the `x-nonce` request header.
- **Credentials live in sessionStorage**, never localStorage: a project
  credential or API key is a fleet administrator credential and should not
  outlive the tab. The session's refresh token is an httpOnly cookie; only the
  cross-origin fallback token is ever stored in page-readable storage.
- **Live view** URLs carry a one-use ticket, never the credential. A shared
  live link carries its scoped token in the URL fragment and strips it once
  read.

## Tests

| Command | Runs |
|:---|:---|
| `npm test` | every Vitest suite in `tests/unit/` |
| `npm run test:coverage` | the same, with a coverage summary |
| `npm run test:ui` | Playwright (`tests/*.spec.ts`) against `OYA_TEST_URL` (a running console) |

Unit tests run in jsdom with Testing Library and `user-event`, fake timers for
anything time-based, and `fakeFetch` (or `vi.mock`) for the network. Each file
calls `cleanup` after each test. Vitest starts its workers with
`--no-experimental-webstorage`: Node 25's own `localStorage` global otherwise
shadows jsdom's. Playwright ignores `tests/unit/`.

To run the end-to-end suite against a production build:
`npm run build`, then `cd .next/standalone && PORT=3979 HOSTNAME=127.0.0.1 node server.js`,
then `OYA_TEST_URL=http://127.0.0.1:3979 npm run test:ui`.

## Exceptions

- **`app/layout.tsx`'s theme script** is a string injected into the page
  before hydration. It stays a literal so it runs before any bundle.
- **`app/live/[browserId]/use-live-page.ts`** sets state in an effect
  (lint-disabled on that line): the share token is in the URL fragment, which
  only exists in the browser, so it can only be read after hydration.
- **`lib/api.ts`'s `account<T = any>`** keeps `any`: the account endpoints'
  answers are untyped JSON passed straight to their callers.
- **Docs sections** are content, split mechanically so each component fits
  the size limit. A long section continues in `<Section>Part2`, `Part3`…
  components, and big tables keep their rows in module constants. Every
  heading id and all text are unchanged; the sidebar, the search index
  (`_docs/search-index.ts`) and links elsewhere depend on the ids.
