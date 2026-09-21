/**
 * Docs: the control plane: architecture, why it beats raw runners, routing and failover, stealth benchmarks.
 */
'use client';

import type { ReactNode } from 'react';
import { CodeBlock, InlineCode, NoteBox, SectionHeading, Table } from '../_docs/blocks';

/** Rows of a table in the Why Oya: The 10x Advantage section. */
const COMPARISON_ROWS_1: ReactNode[][] = [
  [
    'Architecture',
    'Single-vendor lock-in. Outages or regional IP blocks halt all agents.',
    'Unified control plane. Dynamic routing across multiple providers with automatic failover.',
  ],
  [
    'Device Identity',
    'Ephemeral dumb sessions or random fingerprints that trigger bot-farm heuristics.',
    'Deterministic Personas. Cryptographically seeded hardware fingerprints byte-identical across restarts.',
  ],
  [
    'Authentication',
    'Fragile scripted headless logins that fail on Google SSO, passkeys, and Cloudflare.',
    'Sign-In-Once Desktop Pairing. Log in once on desktop; cookies sync securely to cloud personas.',
  ],
  [
    'Challenges & 2FA',
    'Fails or hangs on push approvals or unexpected verification prompts.',
    'Two-Tier Engine + Live Takeover. Automated TOTP/SMS relay + sub-second interactive takeover.',
  ],
  [
    'Observability',
    'Opaque session IDs, black-box execution, post-mortem static videos.',
    '1,000+ browser console, real-time activity log, Prometheus metrics, hourly spend attribution.',
  ],
  [
    'Protocol Freedom',
    'Proprietary SDKs and bespoke API wrappers.',
    'Universal Gateway: Native CDP (/connect), MCP streamable HTTP, TS SDK, and CLI.',
  ],
  [
    'Stealth Testing',
    'Unverifiable marketing claims of "undetectable" scrapers.',
    'Open benchmark suite (oya stealth-test --live) scored against CreepJS and Bot.Sannysoft.',
  ],
];

/** The Control Plane Architecture section. */
function ControlPlane() {
  return (
    <>
      {/* ============ CONTROL PLANE ARCHITECTURE ============ */}
      <SectionHeading id="control-plane" first>
        Control Plane Architecture
      </SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        Raw browser runners like <strong>Browserbase</strong>, <strong>Steel</strong>, <strong>Anchor</strong>, and{' '}
        <strong>Browser Use</strong> are execution targets: they spin up headless Chromium instances inside isolated
        containers or VMs.
      </p>
      <p className="mb-3 text-[15px] leading-relaxed">
        <strong>Oya is the Control Plane.</strong> It sits above the execution targets and manages the state, identity,
        authentication, challenge resolution, and orchestration that production agent fleets require:
      </p>
      <ul className="list-disc list-inside space-y-1.5 mb-5 text-[15px] leading-relaxed">
        <li>
          <strong>Universal Router:</strong> Exposes unified CDP (<InlineCode>/connect</InlineCode>), MCP, and REST
          interfaces. Route requests across providers with priority order and automatic failover.
        </li>
        <li>
          <strong>Deterministic Personas:</strong> Mathematically seeded device profiles. Canvas, WebGL, audio, and
          client rects stay byte-identical across restarts, bound to a dedicated cookie jar and proxy.
        </li>
        <li>
          <strong>Sign-In-Once Desktop Pairing:</strong> Transfer authenticated sessions from real desktop Chrome (with
          WebAuthn, passkeys, and Google SSO) to remote personas via single-use encrypted codes.
        </li>
        <li>
          <strong>Two-Tier Challenges:</strong> Automatic native delegation to CAPTCHA solvers, automated TOTP and
          SMS/email relays, and sub-second interactive live stream handoffs for human intervention.
        </li>
        <li>
          <strong>Fleet Governance:</strong> High-density console for 1,000+ browsers, real-time command activity logs,
          Prometheus metrics (<InlineCode>/metrics</InlineCode>), and hourly spend attribution per tenant key.
        </li>
      </ul>

      <NoteBox>
        By decoupling the <em>control plane</em> from the <em>execution engine</em>, your agent codebase never has to
        know or care which cloud provider or bare-metal machine runs a session.
      </NoteBox>
    </>
  );
}

/** The Why Oya: The 10x Advantage section. */
function Comparison() {
  return (
    <>
      {/* ============ WHY OYA: 10X COMPARISON ============ */}
      <SectionHeading id="comparison">Why Oya: The 10x Advantage</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        Directly coding agents to single-vendor browser runners creates brittle architectures. Here is why an
        orchestrating control plane is 10x better than relying on raw point solutions:
      </p>
      <Table
        headers={['Dimension', 'Raw Runners (Browserbase, Steel, Anchor, Browser Use)', 'Oya Control Plane']}
        rows={COMPARISON_ROWS_1}
      />
    </>
  );
}

/** The Multi-Provider Routing & Failover section. */
function RoutingFailover() {
  return (
    <>
      {/* ============ ROUTING & FAILOVER ============ */}
      <SectionHeading id="routing-failover">Multi-Provider Routing & Failover</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        Configure providers in the dashboard under <strong>Control → Providers</strong> or via the API. Each provider
        has a unique route name, vendor type, priority (0 goes first), and session capacity.
      </p>
      <CodeBlock>{`// Point any CDP client at the Oya Control Plane gateway:
const browser = await chromium.connectOverCDP(
  "wss://browser.getoya.ai/connect?token=YOUR_OYA_KEY"
);

// Oya selects the highest-priority available provider.
// If Steel errors or hits rate limits, Oya instantly fails over to Browserbase or Oya Cloud.`}</CodeBlock>
      <p className="mb-3 text-[15px] leading-relaxed">
        When a connection attempt to an upstream vendor fails, the control plane immediately catches the error, puts the
        failing route into a cooldown period, and dispatches the connection to the next healthy provider in priority
        order. Your client application never observes a disconnect.
      </p>
    </>
  );
}

/** The Stealth & Live Benchmarks section. */
function StealthBenchmarks() {
  return (
    <>
      {/* ============ STEALTH BENCHMARKS ============ */}
      <SectionHeading id="stealth-benchmarks">Stealth & Live Benchmarks</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        Rather than making unsubstantiated marketing claims about detection resistance, Oya includes an open testing
        suite that benchmarks browser evasion against real detectors:
      </p>
      <CodeBlock>{`oya stealth-test            # Score local probe suite
oya stealth-test --live     # Benchmark live against Bot.Sannysoft and CreepJS`}</CodeBlock>
      <p className="mb-3 text-[15px] leading-relaxed">
        The suite tests canvas noise, WebGL renderer and vendor strings, AudioContext noise, client rects, plugins,{' '}
        <InlineCode>navigator.webdriver</InlineCode>, <InlineCode>userAgentData</InlineCode>, media devices, and{' '}
        <InlineCode>Function.prototype.toString</InlineCode> masking.
      </p>
      <NoteBox>
        Oya deliberately does <strong>not</strong> double-layer custom stealth over providers that already ship tuned
        anti-bot stealth (Anchor, Browserbase, Steel, Browser Use). Double-masking causes internal contradictions that
        anti-bot heuristics detect. On those providers, Oya manages the persona identity, cookie jar, residential proxy,
        and concurrency limits.
      </NoteBox>
    </>
  );
}

/** The control plane: architecture, why it beats raw runners, routing and failover, stealth benchmarks. */
export function ControlPlaneDocs() {
  return (
    <>
      <ControlPlane />
      <Comparison />
      <RoutingFailover />
      <StealthBenchmarks />
    </>
  );
}
