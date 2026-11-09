/**
 * Docs: the dashboard: overview, onboarding, live view and settings.
 */
'use client';

import type { ReactNode } from 'react';
import { CodeBlock, InlineCode, InlineLink, NoteBox, SectionHeading, Table } from '../_docs/blocks';

/** Rows of a table in the Dashboard section. */
const DASHBOARD_OVERVIEW_ROWS_1: ReactNode[][] = [
  ['⌘/Ctrl 1 · 2 · 3 · 4 · 5', 'Browsers · Personas · Control · Playbooks · Routines'],
  ['n', 'Start a browser'],
  ['/', 'Filter the fleet'],
  ['↑ ↓ or j k', 'Move the selection'],
  ['x', 'Stop the selected browser(s)'],
  ['l · r · s', 'URL bar · reload · screenshot'],
  ['Esc', 'Close the panel, or release the keyboard from the live view'],
  ['?', 'The full list'],
];

/** The Dashboard section. */
function DashboardOverview() {
  return (
    <>
      {/* ============ DASHBOARD ============ */}
      <SectionHeading id="dashboard-overview">Dashboard</SectionHeading>
      <NoteBox>
        <strong>Browsers, commands, and CDP sessions</strong>
        <br />A browser is a running desktop or cloud instance. REST commands, including curl requests to{' '}
        <InlineCode>/api/browsers/:id/command</InlineCode>, appear in that browser’s Activity history and count toward
        Usage. A CDP session is a persistent client connection through <InlineCode>/connect</InlineCode>, typically from
        Playwright or Puppeteer. Find these under Control → CDP sessions.
      </NoteBox>
      <p className="mb-3 text-[15px] leading-relaxed">
        The <InlineLink href="/dashboard">dashboard</InlineLink> at <InlineCode>/dashboard</InlineCode> is the control
        panel. It shows your connected browsers and lets you interact with them.
      </p>
      <p className="mb-3 text-[15px] leading-relaxed">
        Built to hold a thousand browsers and let you act on any one of them:
      </p>
      <DashboardTabs />
      <DashboardOverviewPart2 />
      <DashboardOverviewPart3 />
      <DashboardOverviewPart4 />
      <DashboardOverviewPart5 />
      <DashboardOverviewPart6 />
      <DashboardOverviewPart7 />
      <DashboardOverviewPart8 />
      <DashboardOverviewPart9 />
    </>
  );
}

/** The console's tabs, one line each. */
function DashboardTabs() {
  return (
    <ul className="list-disc list-inside space-y-1 mb-4 text-[15px] leading-relaxed">
      <li>
        <strong>Browsers</strong>: a health strip (every number is a filter) over a dense table: health, persona,
        provider, current page, commands · errors, seen, uptime. Select a row to open the panel: URL bar, a bounded{' '}
        <em>interactive</em> live view, screenshot, elements, stats, and the activity log, what that browser has been
        doing, newest first.
      </li>
      <li>
        <strong>Personas</strong>: one identity each. Create with a chosen device and a live fingerprint preview; edit
        name, cap, proxy pin and MFA; the device itself is locked, with <em>Clone</em> for when you want a different
        one.
      </li>
      <li>
        <strong>Control</strong>: health, gateway sessions, providers and routing, per-key usage, the audit trail,
        recordings.
      </li>
      <li>
        <strong>Playbooks</strong>: every saved playbook on the key, its variables, and a Run button.
      </li>
      <li>
        <strong>Routines</strong>: playbooks on a schedule, with the last run and the next one.
      </li>
    </ul>
  );
}

/** The Dashboard section, continued. */
function DashboardOverviewPart2() {
  return (
    <>
      <h3 className="text-base font-semibold mt-6 mb-2 text-text">Adding a provider</h3>
      <p className="mb-3 text-[15px] leading-relaxed">
        Open <strong>Control → Providers → Add provider</strong>. Give the route a unique name, choose a vendor, and
        enter its API key. A credential already saved in Settings can be reused. For your own Chrome, supply its CDP
        WebSocket URL instead.
      </p>
      <p className="mb-3 text-[15px] leading-relaxed">
        Set the session capacity and routing priority (0 goes first). Providers and your routing strategy are saved for
        your Oya key across restarts; credentials and connection URLs are encrypted. Saving a provider does not launch a
        browser or verify its credentials. Its first connection does that. End active sessions before removing a route.
      </p>
      <p className="mb-3 text-[15px] leading-relaxed">
        These routes serve new CDP connections to <InlineCode>/connect</InlineCode>, with your key in an{' '}
        <InlineCode>Authorization: Bearer</InlineCode> header or a one-use ticket from the API, never in the URL. The{' '}
        <strong>Start browser</strong> action uses your provider selection in <strong>Settings → Browsers</strong>.
        Attaching with <InlineCode>?browser=ID</InlineCode> connects to that existing browser.
      </p>
      <h3 className="text-base font-semibold mt-6 mb-2 text-text">Driving a browser from the live view</h3>
      <p className="mb-3 text-[15px] leading-relaxed">
        Choose <strong>Stream</strong> in a browser panel, or <strong>Open live stream in a tab</strong> from its menu,
        to open an interactive viewer in a separate tab. Your dashboard key authorizes the viewer. The{' '}
        <InlineCode>/api/live/:id</InlineCode> endpoint is the raw event stream for integrations.
      </p>
      <p className="mb-3 text-[15px] leading-relaxed">
        Click to control. Clicks land at the page pixel under the cursor, a drag is a drag, the wheel scrolls, typing is
        batched into <InlineCode>keyboard_type</InlineCode> and the named keys go as <InlineCode>press_key</InlineCode>.{' '}
        <InlineCode>Esc</InlineCode> hands the keyboard back. What was typed is never written to the activity log, it
        records <em>2 chars</em>, not the text.
      </p>
    </>
  );
}

/** The Dashboard section, continued. */
function DashboardOverviewPart3() {
  return (
    <>
      <h3 className="text-base font-semibold mt-6 mb-2 text-text">Connect to a browser that is already running</h3>
      <p className="mb-3 text-[15px] leading-relaxed">
        Right-click any row (or press <strong>Connect</strong> in the panel) for code that targets that exact browser:
        SDK, CLI, an MCP config, curl, and for CDP-backed browsers a Playwright <InlineCode>connectOverCDP</InlineCode>{' '}
        attach with the key in a header. Snippets are written for this deployment and your key; the key is masked until
        you ask, and copy always copies the real one. <strong>Copy CDP attach URL</strong> copies a URL with a one-use
        ticket that expires in 60 seconds, the same <InlineCode>cdpUrl</InlineCode> that{' '}
        <InlineCode>GET /api/browsers/:id</InlineCode> returns, for tools that take only a URL.
      </p>
      <CodeBlock>{`// Attach through the gateway to one browser in the fleet. Closing your
// client leaves the browser running.
const browser = await chromium.connectOverCDP("wss://<host>/connect?browser=<browser-id>", {
  headers: { Authorization: "Bearer <api-key>" },
});`}</CodeBlock>
    </>
  );
}

/** The Dashboard section, continued. */
function DashboardOverviewPart4() {
  return (
    <>
      <p className="mb-3 text-[15px] leading-relaxed">
        Any browser that exposes a CDP endpoint has a <InlineCode>cdpUrl</InlineCode>: Browserbase, Steel, Anchor, your
        own Chrome, and Oya browsers through their CDP front door. Playwright and Puppeteer attach the same way to each;
        the SDK, CLI and MCP work too.
      </p>

      <h3 className="text-base font-semibold mt-6 mb-2 text-text">Stop means stop</h3>
      <p className="mb-3 text-[15px] leading-relaxed">
        One button, one endpoint (<InlineCode>POST /browsers/:id/stop</InlineCode>). A cloud browser&apos;s sandbox is
        destroyed so billing ends; a CDP browser is handed back to its provider; a desktop browser disconnects. The
        confirm says which. Bulk stop takes <InlineCode>{`{ids: [...]}`}</InlineCode> or{' '}
        <InlineCode>{`{all: true}`}</InlineCode>.
      </p>
    </>
  );
}

/** The Dashboard section, continued. */
function DashboardOverviewPart5() {
  return (
    <>
      <h3 className="text-base font-semibold mt-6 mb-2 text-text">Keyboard</h3>
      <Table headers={['Key', 'Does']} rows={DASHBOARD_OVERVIEW_ROWS_1} />
      <p className="mb-3 text-[15px] leading-relaxed">
        You can sign in with an account, or by pasting an API key: a self-hosted deployment with{' '}
        <InlineCode>API_KEYS</InlineCode> and no database has no accounts, and still needs its own UI.
      </p>

      <h3 id="onboarding" className="text-base font-semibold mt-6 mb-2 text-text">
        Onboarding
      </h3>
      <p className="mb-3 text-[15px] leading-relaxed">
        A key that has not been set up gets a two-step page. Everything it asks is stored against that key, nothing
        lands in an environment variable, and nothing is inherited from an account.
      </p>
    </>
  );
}

/** The Dashboard section, continued. */
function DashboardOverviewPart6() {
  return (
    <>
      <ol className="list-decimal list-inside space-y-1.5 mb-4 text-[15px] leading-relaxed">
        <li>
          <strong>Desktop</strong>: one click opens the desktop browser signed in to the project
        </li>
        <li>
          <strong>AI model</strong>: Claude, OpenAI or Gemini and your key, which Ask in the desktop needs. Ask also
          asks for it inline if it is missing
        </li>
      </ol>
      <p className="mb-3 text-[15px] leading-relaxed">
        Where browsers run, CAPTCHA solving and the model itself are all in Settings, and{' '}
        <InlineCode>oya init</InlineCode> sets them up in a terminal.
      </p>
    </>
  );
}

/** The Dashboard section, continued. */
function DashboardOverviewPart7() {
  return (
    <>
      <h3 className="text-base font-semibold mt-6 mb-2 text-text">Agent panel (desktop app)</h3>
      <p className="mb-3 text-[15px] leading-relaxed">
        The desktop app&apos;s agent panel (the <strong>Open Assistant</strong> button in the toolbar, ⌘/Ctrl Shift D)
        has five tabs, and Inspect holds three more:
      </p>
      <ul className="list-disc list-inside space-y-1 mb-4 text-[15px] leading-relaxed">
        <li>
          <strong>Ask</strong>: type a task, watch it run, with tool badges
        </li>
        <li>
          <strong>Record</strong>: the studio; do a task once and save it as a playbook
        </li>
        <li>
          <strong>Playbooks</strong>: the playbooks on this key, replay with new values
        </li>
        <li>
          <strong>Routines</strong>: scheduled playbooks and their runs
        </li>
        <li>
          <strong>Inspect → Actions</strong>: a button for every command: analyze, screenshot, navigate, click by id,
          type, keys, hover, scroll, wait, tabs
        </li>
        <li>
          <strong>Inspect → Activity</strong>: live traffic, in and out, with payloads and a direction filter
        </li>
        <li>
          <strong>Inspect → Source</strong>: the page as the agent sees it, TOON or markdown, beside the HTML
        </li>
      </ul>
    </>
  );
}

/** The Dashboard section, continued. */
function DashboardOverviewPart8() {
  return (
    <>
      <h3 id="live-view" className="text-base font-semibold mt-6 mb-2 text-text">
        Live View
      </h3>
      <p className="mb-3 text-[15px] leading-relaxed">
        Select a browser on the Browsers tab to watch it work. Frames stream as JPEG over SSE at ~2fps.{' '}
        <InlineCode>browser.liveViewUrl()</InlineCode> is the console deep link for a person to open;{' '}
        <InlineCode>await browser.liveStreamUrl()</InlineCode> gives you the same frames to embed, with a single-use
        ticket that expires in 60 seconds, EventSource cannot set headers, and a URL that ends up in browser history
        should not be a permanent credential.
      </p>

      <h3 id="settings" className="text-base font-semibold mt-6 mb-2 text-text">
        Settings
      </h3>
    </>
  );
}

/** The Dashboard section, continued. */
function DashboardOverviewPart9() {
  return (
    <>
      <p className="mb-3 text-[15px] leading-relaxed">
        The gear icon next to the API key bar. Everything here belongs to that key: model provider and credential,
        default model, browser provider and its credential, CAPTCHA solver, and the one-click desktop sign-in.
      </p>
      <p className="mb-3 text-[15px] leading-relaxed">
        Credentials are sealed at rest with AES-256-GCM and always read back masked. Saving the masked placeholder never
        overwrites the real value.
      </p>
      <NoteBox>
        A key that has set nothing falls back to the deployment-wide defaults. Changing <em>those</em> affects every key
        that has not set its own, so it needs <InlineCode>OYA_OPERATOR_TOKEN</InlineCode> via{' '}
        <InlineCode>POST /config/host</InlineCode> rather than any API key.
      </NoteBox>
    </>
  );
}

/** The dashboard: overview, onboarding, live view and settings. */
export function DashboardDocs() {
  return (
    <>
      <DashboardOverview />
    </>
  );
}
