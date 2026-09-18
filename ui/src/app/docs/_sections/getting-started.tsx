/**
 * Docs: getting started: quickstart, SDK, CLI, API keys, desktop sign-in and connecting.
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
  Table,
  WarnBox,
} from '../_docs/blocks';
import { useDocsNav } from '../_docs/nav';

/** Rows of a table in the Desktop Sign-in section. */
const DOWNLOAD_ROWS_1: ReactNode[][] = [
  [
    'macOS (Intel + Apple Silicon)',
    <a key="mac" href={browserDownloads[0].href} className="text-accent hover:text-accent-hover transition-colors">
      Oya Browser.dmg
    </a>,
  ],
  [
    'Windows (x64)',
    <a key="win" href={browserDownloads[1].href} className="text-accent hover:text-accent-hover transition-colors">
      Oya Browser.exe
    </a>,
  ],
  [
    'Linux (x64)',
    <a key="linux" href={browserDownloads[2].href} className="text-accent hover:text-accent-hover transition-colors">
      Oya Browser.AppImage
    </a>,
  ],
];

/** Rows of a table in the Connect section. */
const CONNECT_ROWS_1: ReactNode[][] = [
  ['Server URL', <InlineCode key="url">wss://browser.getoya.ai/ws</InlineCode>],
  ['API Key', 'The key you generated in the dashboard'],
  ['Browser Name', 'Optional — how it shows in the dashboard'],
];

/** The Quickstart section. */
function Quickstart() {
  const { navigate } = useDocsNav();
  return (
    <>
      {/* ============ QUICKSTART ============ */}
      <SectionHeading id="quickstart">Quickstart</SectionHeading>
      <CodeBlock>{`npm i @oya-ai/browser
npm i -g @oya-ai/cli && oya login && oya init`}</CodeBlock>
      <CodeBlock>{`import { Oya } from "@oya-ai/browser";

const oya = new Oya();                                    // OYA_API_KEY
const browser = await oya.browser.start({ persona: "auto", captcha: "auto" });
await browser.goto("https://example.com");`}</CodeBlock>
      <p className="mb-3 text-[15px] leading-relaxed">
        That is the whole surface. Which provider actually runs the browser — Oya Cloud, your own machines, Browser Use,
        Browserbase, Steel, Anchor, or a CDP URL you hand us — is a setting on your API key, chosen once during{' '}
        <InlineAnchor onClick={() => navigate('onboarding')}>onboarding</InlineAnchor>. Your code never branches on it.
      </p>
      <NoteBox>
        The API key is the identity for everything: browsers, personas, cookies, settings, usage and audit history are
        all scoped to it, and one key can never see another&apos;s.
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
        types. Element IDs come from <InlineCode>analyze()</InlineCode> and are only valid until the page changes —
        after a navigation or a click that redraws, analyze again.
      </p>
      <CodeBlock>{`const page = await browser.analyze();      // markdown + numbered elements
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
        <InlineCode>browser.cdpUrl</InlineCode> is our gateway URL, not the vendor&apos;s — point Playwright, Puppeteer,
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
      <SectionHeading id="create-key">Create API Key</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        Go to the <InlineLink href="/dashboard">dashboard</InlineLink>. Open the API key menu to create or select a key
        for your workspace.
      </p>
      <p className="mb-3 text-[15px] leading-relaxed">
        Your key is scoped — you only see browsers connected with your key. Other users&apos; browsers are invisible to
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
      <SectionHeading id="download">Desktop Sign-in</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        For browsers on Oya infrastructure, the desktop app is a one-time step: log into the sites your agents need, and
        those cookies move to the remote browsers, which run the same fingerprint as that identity. The agent arrives
        already signed in, and the site sees one device returning rather than a fleet sharing an account.
      </p>
      <p className="mb-3 text-[15px] leading-relaxed">
        Onboarding and Settings both have an <strong>Open the desktop browser</strong> button. It builds an{' '}
        <InlineCode>oya://</InlineCode> link carrying a single-use pairing code — never your API key, because a protocol
        URL is reachable by any page you visit and lands in OS logs on the way. The app exchanges that code over HTTPS
        with the server the link names.
      </p>
      <WarnBox>
        The desktop app asks before connecting, naming the destination host, with Cancel as the default. Connecting
        shares that browser&apos;s cookies and logged-in sessions with the control plane it dials — so if a web page
        opened the dialog rather than your own dashboard, cancel it.
      </WarnBox>
      <Table headers={['Platform', 'Download']} rows={DOWNLOAD_ROWS_1} />
      <p className="mb-3 text-[15px] leading-relaxed">
        <strong>macOS:</strong> Open the .dmg, drag to Applications. On first launch, macOS may block the app because
        it&apos;s not notarized. Fix:
      </p>
      <CodeBlock>{`xattr -cr /Applications/Oya\\ Browser.app`}</CodeBlock>
      <p className="mb-3 text-[15px] leading-relaxed">
        Or: right-click the app → Open → Open (bypasses Gatekeeper once).
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
      <CodeBlock>{`# macOS — open another instance
open -n "/Applications/Oya Browser.app"

# With separate sessions (own cookies, own config)
open -n "/Applications/Oya Browser.app" --args --user-data-dir=/tmp/oya-2
open -n "/Applications/Oya Browser.app" --args --user-data-dir=/tmp/oya-3

# Linux
./Oya-Browser.AppImage --user-data-dir=/tmp/oya-2`}</CodeBlock>
      <p className="mb-3 text-[15px] leading-relaxed">
        Each <InlineCode>--user-data-dir</InlineCode> gets its own cookies, logins, and config — fully isolated
        sessions.
      </p>
    </>
  );
}

/** The Connect section. */
function Connect() {
  return (
    <>
      {/* ============ CONNECT ============ */}
      <SectionHeading id="connect">Connect</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">Open Oya Browser. The setup screen appears on first launch.</p>
      <Table headers={['Field', 'Value']} rows={CONNECT_ROWS_1} />
      <p className="mb-3 text-[15px] leading-relaxed">
        Click <strong>Connect</strong>. The green dot in the toolbar confirms the connection. Your browser now appears
        in the <InlineLink href="/dashboard">dashboard</InlineLink>.
      </p>
    </>
  );
}

/** Getting started: quickstart, SDK, CLI, API keys, desktop sign-in and connecting. */
export function GettingStartedDocs() {
  return (
    <>
      <Quickstart />
      <Sdk />
      <Cli />
      <CreateKey />
      <Download />
      <Connect />
    </>
  );
}
