/**
 * Docs: get started (the quickstart, API keys, desktop sign-in and connecting),
 * and the SDK and CLI references.
 */
'use client';

import type { ReactNode } from 'react';
import { browserDownloads } from '@/lib/browser-downloads';
import {
  CodeBlock,
  InlineAnchor,
  InlineCode,
  InlineLink,
  NoteBox,
  SectionHeading,
  Step,
  Table,
  WarnBox,
} from '../_docs/blocks';
import { useDocsNav } from '../_docs/nav';

/** Rows of a table in the Desktop Sign-in section. */
const DOWNLOAD_ROWS_1: ReactNode[][] = [
  [
    'macOS (Intel + Apple Silicon)',
    <a
      key="mac"
      data-track="download_clicked"
      data-track-label="macOS"
      data-track-place="docs"
      href={browserDownloads[0].href}
      className="text-accent hover:text-accent-hover transition-colors"
    >
      Oya Browser.dmg
    </a>,
  ],
  [
    'Windows (x64)',
    <a
      key="win"
      data-track="download_clicked"
      data-track-label="Windows"
      data-track-place="docs"
      href={browserDownloads[1].href}
      className="text-accent hover:text-accent-hover transition-colors"
    >
      Oya Browser.exe
    </a>,
  ],
  [
    'Linux (x64)',
    <a
      key="linux"
      data-track="download_clicked"
      data-track-label="Linux"
      data-track-place="docs"
      href={browserDownloads[2].href}
      className="text-accent hover:text-accent-hover transition-colors"
    >
      Oya Browser.AppImage
    </a>,
  ],
];

/** Rows of a table in the Connect section. */
const CONNECT_ROWS_1: ReactNode[][] = [
  ['Server URL', <InlineCode key="url">wss://oyabrowser.com/ws</InlineCode>],
  ['API Key', 'The key you generated in the dashboard'],
  ['Browser Name', 'Optional, how it shows in the dashboard'],
];

/** The first run: do a task once with the model, save it, replay it without. */
const FIRST_RUN = `import { Oya } from "@oya-ai/browser";

const oya = new Oya();                          // reads OYA_API_KEY
const browser = await oya.browser.start();      // a real browser, not headless Chrome

try {
  // 1. Do the task once, in plain language. The model works it out.
  await browser.ask(
    "On https://httpbin.org/forms/post order a medium pizza for {{name}} and submit it.",
    { data: { name: "Ada Lovelace" } },
  );

  // 2. Save that run as a playbook: Playwright steps, with your values as variables.
  const playbook = await browser.toPlaybook("pizza-order");
  console.log(playbook.variables);              // the inputs play() takes

  // 3. Replay it with new values. No model, no tokens.
  console.log(await browser.play("pizza-order", { name: "Grace Hopper" }));
} finally {
  await browser.close();                        // stop paying for it, even on an error
}`;

/** The Quickstart section: three steps from nothing to a replayed playbook. */
function Quickstart() {
  return (
    <>
      <SectionHeading id="quickstart" first>
        Quickstart
      </SectionHeading>
      <p className="mb-6">Three steps, about three minutes. You end with a task your code replays with no model.</p>
      <Step n={1} title="Get an API key">
        <p className="mb-3">
          Open the <InlineLink href="/dashboard">console</InlineLink>, create a key from the API key menu, and export
          it.
        </p>
        <CodeBlock label="Terminal">{'export OYA_API_KEY=oya_...'}</CodeBlock>
      </Step>
      <Step n={2} title="Install the SDK">
        <CodeBlock label="Terminal">{'npm i @oya-ai/browser'}</CodeBlock>
      </Step>
      <Step n={3} title="Run a task, save it, replay it">
        <CodeBlock label="first-run.mjs">{FIRST_RUN}</CodeBlock>
      </Step>
      <QuickstartNext />
    </>
  );
}

/** Where to go after the first run, and the CLI route for people who would rather not write code yet. */
function QuickstartNext() {
  const { navigate } = useDocsNav();
  return (
    <>
      <p className="mb-3">
        That is the loop: <strong>ask once, replay forever</strong>. From here, read{' '}
        <InlineAnchor onClick={() => navigate('playbooks')}>Playbooks</InlineAnchor> for variables, free-text fields and
        healing, or <InlineAnchor onClick={() => navigate('mcp-setup')}>MCP</InlineAnchor> to hand the browser to Claude
        or Cursor.
      </p>
      <h3 className="text-base font-semibold mt-6 mb-2 text-text">From a terminal instead</h3>
      <CodeBlock label="Terminal">{`npm i -g @oya-ai/cli
oya login && oya start
oya ask "find the pricing page on example.com"`}</CodeBlock>
      <NoteBox>
        Everything belongs to the API key: browsers, personas, cookies, playbooks, settings and usage. One key never
        sees another&apos;s. Which provider runs the browser is a setting on the key, chosen during{' '}
        <InlineAnchor onClick={() => navigate('onboarding')}>onboarding</InlineAnchor>, so your code never changes.
      </NoteBox>
    </>
  );
}

/** The SDK section. */
function Sdk() {
  return (
    <>
      {/* ============ SDK ============ */}
      <SectionHeading id="sdk">SDK</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        <InlineCode>@oya-ai/browser</InlineCode> is TypeScript with no runtime dependencies, shipped as ESM, CJS and
        types. Element IDs come from <InlineCode>analyze()</InlineCode> and are only valid until the page changes, after
        a navigation or a click that redraws, analyze again.
      </p>
      <CodeBlock>{`const page = await browser.analyze();      // markdown + numbered elements ({ format: 'toon' } for TOON)
const els  = await browser.elements();     // just the visible ones

await browser.click(13);
await browser.type(9, "hello");
await browser.pressKey("Enter");
await browser.waitFor("[data-testid=results]");
await browser.scroll("bottom");

const png = await browser.screenshot();    // base64
const answer = await browser.ask("find the pricing page");

await browser.solveCaptcha();              // { solved, method }
await browser.completeMfa();               // { completed, method, liveViewUrl }
await browser.close();`}</CodeBlock>

      <h3 className="text-base font-semibold mt-6 mb-2 text-text">Bring your own tools</h3>
      <p className="mb-3 text-[15px] leading-relaxed">
        <InlineCode>browser.cdpUrl</InlineCode> is our gateway URL, not the vendor&apos;s, point Playwright, Puppeteer,
        Stagehand or browser-use at it and you get routing, profile capture and session recording without any of them
        knowing this exists.
      </p>
      <CodeBlock>{`const browser = await oya.browser.start();
const pw = await chromium.connectOverCDP(browser.cdpUrl);`}</CodeBlock>
      <SdkPart2 />
    </>
  );
}

/** The SDK section, continued. */
function SdkPart2() {
  return (
    <>
      <p className="mb-3 text-[15px] leading-relaxed">
        The gateway also answers <InlineCode>/json/version</InlineCode> and <InlineCode>/json/list</InlineCode>, which
        is what lets those clients treat it as an ordinary browser.
      </p>
    </>
  );
}

/** The CLI section. */
function Cli() {
  return (
    <>
      {/* ============ CLI ============ */}
      <SectionHeading id="cli">CLI</SectionHeading>
      <CodeBlock>{`oya login                       Save an API key for this machine
oya init                        Model, browser provider, solver, desktop sign-in
oya start [--persona auto]      Start a browser and print its id
oya goto <url>                  Navigate (defaults to the newest browser)
oya ask "<prompt>"              Drive it in plain language
oya ls                          What is running
oya rm <id> | --all             Stop browsers
oya personas [new|rm <id>]      Identities and their concurrency
oya open                        Watch a browser work
oya playbooks                   Saved playbooks
oya playbooks export <name>     One playbook as JSON [--out <file>]
oya playbooks import <file>     Save an export here [--name <n>] [--replace]
oya config [key=value ...]      This key's settings
oya usage                       What this key has spent
oya stealth-test [--live]       Score this deployment against bot detectors`}</CodeBlock>
      <p className="mb-3 text-[15px] leading-relaxed">
        Flags and <InlineCode>OYA_API_KEY</InlineCode> / <InlineCode>OYA_BASE_URL</InlineCode> beat the saved file, so
        CI never needs <InlineCode>oya login</InlineCode>. The key is stored at{' '}
        <InlineCode>~/.oya/config.json</InlineCode>, mode 600.
      </p>
    </>
  );
}

/** The Create API Key section. */
function CreateKey() {
  return (
    <>
      {/* ============ CREATE KEY ============ */}
      <SectionHeading id="create-key">API keys</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        Go to the <InlineLink href="/dashboard">dashboard</InlineLink>. Open the API key menu to create or select a key
        for your workspace.
      </p>
      <p className="mb-3 text-[15px] leading-relaxed">
        Your key is scoped: you only see browsers connected with your key. Other users&apos; browsers are invisible to
        you.
      </p>
      <WarnBox>
        Save your key somewhere safe. If you lose it, you&apos;ll need to generate a new one. The old key still works
        for any browsers already connected with it.
      </WarnBox>
    </>
  );
}

/** The Desktop Sign-in section. */
function Download() {
  return (
    <>
      {/* ============ DOWNLOAD ============ */}
      <SectionHeading id="download">Desktop sign-in</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        For browsers on Oya infrastructure, the desktop app is a one-time step: log into the sites your agents need, and
        those cookies move to the remote browsers, which run the same fingerprint as that identity. The agent arrives
        already signed in, and the site sees one device returning rather than a fleet sharing an account.
      </p>
      <p className="mb-3 text-[15px] leading-relaxed">
        Onboarding and Settings both have an <strong>Open the desktop browser</strong> button. It builds an{' '}
        <InlineCode>oya://</InlineCode> link carrying a single-use pairing code, never your API key, because a protocol
        URL is reachable by any page you visit and lands in OS logs on the way. The app exchanges that code over HTTPS
        with the server the link names.
      </p>
      <WarnBox>
        The desktop app asks before connecting, naming the destination host, with Cancel as the default. Connecting
        shares that browser&apos;s cookies and logged-in sessions with the control plane it dials, so if a web page
        opened the dialog rather than your own dashboard, cancel it.
      </WarnBox>
      <Table headers={['Platform', 'Download']} rows={DOWNLOAD_ROWS_1} />
      <p className="mb-3 text-[15px] leading-relaxed">
        <strong>macOS:</strong> Open the .dmg and drag the app to Applications. The build is signed and notarized, so it
        opens normally. If an older download is blocked, right-click the app → Open → Open.
      </p>
      <DownloadPart2 />
    </>
  );
}

/** The Desktop Sign-in section, continued. */
function DownloadPart2() {
  return (
    <>
      <p className="mb-3 text-[15px] leading-relaxed">
        <strong>Linux:</strong> <InlineCode>chmod +x</InlineCode> the AppImage and run it.
      </p>

      <h3 className="text-base font-semibold mt-6 mb-2 text-text">Running multiple instances</h3>
      <p className="mb-3 text-[15px] leading-relaxed">
        To open multiple browser windows (e.g. different accounts or different API keys):
      </p>
      <CodeBlock>{`# macOS, open another instance
open -n "/Applications/Oya Browser.app"

# With separate sessions (own cookies, own config)
open -n "/Applications/Oya Browser.app" --args --user-data-dir=/tmp/oya-2
open -n "/Applications/Oya Browser.app" --args --user-data-dir=/tmp/oya-3

# Linux
./Oya-Browser.AppImage --user-data-dir=/tmp/oya-2`}</CodeBlock>
      <p className="mb-3 text-[15px] leading-relaxed">
        Each <InlineCode>--user-data-dir</InlineCode> gets its own cookies, logins, and config, fully isolated sessions.
      </p>
    </>
  );
}

/** The Connect section. */
function Connect() {
  return (
    <>
      {/* ============ CONNECT ============ */}
      <SectionHeading id="connect">Connect a desktop browser</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">Open Oya Browser. The setup screen appears on first launch.</p>
      <Table headers={['Field', 'Value']} rows={CONNECT_ROWS_1} />
      <p className="mb-3 text-[15px] leading-relaxed">
        Click <strong>Connect</strong>. The green dot in the toolbar confirms the connection. Your browser now appears
        in the <InlineLink href="/dashboard">dashboard</InlineLink>.
      </p>
    </>
  );
}

/** Get started: the quickstart, then the key, the desktop app and connecting it. */
export function GetStartedDocs() {
  return (
    <>
      <Quickstart />
      <CreateKey />
      <Download />
      <Connect />
    </>
  );
}

/** The SDK reference. */
export function SdkDocs() {
  return <Sdk />;
}

/** The CLI reference. */
export function CliDocs() {
  return <Cli />;
}
