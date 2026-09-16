'use client';

import { useState, useEffect, useRef } from 'react';
import Link from 'next/link';
import { motion, AnimatePresence } from 'framer-motion';
import {
  ArrowRight,
  ArrowUpRight,
  Check,
  Copy,
  Fingerprint,
  Globe2,
  Layers,
  Monitor,
  Terminal,
  Menu,
  X,
  ChevronRight,
  ChevronDown,
  Command,
  Play,
  Pause,
  RotateCcw,
  ShieldCheck,
  Network,
  RefreshCw,
  KeyRound,
  Sliders,
  CheckCircle2,
  XCircle,
  ExternalLink,
  Lock,
  Radio,
  MousePointer,
  CheckCheck,
  BarChart3,
  HelpCircle,
  Briefcase,
} from 'lucide-react';
import { OyaWordmark, OyaLogo } from '@/components/oya-logo';
import ThemeToggle from '@/components/theme-toggle';
import SyntaxCode from '@/components/ui/syntax-code';

/* ─────────────────────────────────────────────────────────────────────────────
   Code Snippets & Interactive Simulation Test Data
───────────────────────────────────────────────────────────────────────────── */
const examples = [
  {
    label: 'TypeScript SDK',
    language: 'typescript' as const,
    file: 'agent.ts',
    code: `import { Oya } from "@oya-ai/browser";

const oya = new Oya();                                    // Reads OYA_API_KEY
// Provider (Oya Cloud, Browserbase, Steel, Anchor, Browser Use) is a setting, not code
const browser = await oya.browser.start({
  persona: "acme-ops",                                    // Byte-identical seeded identity
  captcha: "auto",                                        // Solves natively or via solver
});

await browser.goto("https://app.example.com");
const page = await browser.analyze();                     // Markdown + numbered element IDs
await browser.click(page.elements[0].id);`,
    testSteps: [
      { text: 'Validating client credentials against Oya Control Plane...', time: '+12ms' },
      { text: 'Allocating deterministic persona "acme-ops" (hardware seed: 0x8fa1)...', time: '+42ms' },
      { text: 'Selecting optimal execution runner [Oya Cloud Sandbox · 14ms latency]...', time: '+86ms' },
      { text: 'Navigating to https://app.example.com (HTTP 200 OK)...', time: '+164ms' },
      { text: 'DOM parsed: 24 interactive elements indexed into structured markdown...', time: '+220ms' },
      { text: 'Dispatching click(page.elements[0].id) -> [#13 button "Checkout"]...', time: '+290ms' },
      { text: 'Execution verified successfully with zero stealth leaks.', time: '+340ms' },
    ],
    mockResult: `# analyze_page response
url: https://app.example.com
title: Enterprise Operations Console
elements: 24

---
[#1 link "Overview"] [#2 link "Settings"]
[#9 input:email placeholder="admin@acme.com"]
[#13 button "Checkout" primary]
[#14 button "Cancel"]

---
> AI executed click(13). Navigation underway [Status: 200 OK]`,
    telemetry: {
      latency: '14ms',
      memory: '48 MB',
      provider: 'Oya Cloud',
      creepJsHeadless: '0% (bare: 100%)',
      status: '200 OK',
    },
  },
  {
    label: 'Standard Playwright CDP',
    language: 'typescript' as const,
    file: 'playwright.ts',
    code: `import { chromium } from "playwright";
import { Oya } from "@oya-ai/browser";

const oya = new Oya();
const oyaBrowser = await oya.browser.start({ persona: "auto" });

// Connect native Playwright directly through the Oya Control Plane gateway.
// Zero SDK lock-in — routing, persona injection, and session recording are automatic.
const browser = await chromium.connectOverCDP(oyaBrowser.cdpUrl);
const page = await browser.newPage();
await page.goto("https://github.com");`,
    testSteps: [
      { text: 'Initializing Oya Control Plane connection pool...', time: '+10ms' },
      { text: 'Spawning headless browser target with automatic persona rotation...', time: '+55ms' },
      { text: 'Generated authenticated CDP gateway tunnel: wss://browser.getoya.ai/connect...', time: '+92ms' },
      { text: 'Playwright chromium.connectOverCDP() handshake acknowledged...', time: '+148ms' },
      { text: 'Opening new browser context and binding pinned proxy IP...', time: '+210ms' },
      { text: 'page.goto("https://github.com") completed (DOMContentReady in 240ms)...', time: '+298ms' },
      { text: 'CDP session active and streaming metrics to /gateway/metrics.', time: '+330ms' },
    ],
    mockResult: `{
  "cdpSessionId": "cdp_ses_9f1b0a88",
  "clientType": "playwright",
  "browserVersion": "Chrome/131.0.6778.86",
  "targetUrl": "https://github.com",
  "viewport": { "width": 1440, "height": 900 },
  "proxyAssigned": "us-east.proxy.oya.ai:8080",
  "concurrency": "1/5 slots"
}`,
    telemetry: {
      latency: '18ms',
      memory: '64 MB',
      provider: 'Steel Runner',
      creepJsHeadless: '0% (bare: 100%)',
      status: 'CDP Attached',
    },
  },
  {
    label: 'Terminal CLI',
    language: 'bash' as const,
    file: 'Terminal',
    code: `npm install -g @oya-ai/cli
oya login
oya init                               # Model, provider, solver, desktop pairing

oya personas new acme-ops --platform MacIntel --tz America/New_York
oya start --persona acme-ops
oya goto https://app.example.com
oya ask "Download the latest invoice report"
oya stealth-test --live                # Score evasion against CreepJS and Sannysoft
oya ls`,
    testSteps: [
      { text: '$ oya login --url https://browser.getoya.ai (Token verified: tenant_acme)', time: '+8ms' },
      { text: '$ oya personas new acme-ops (Seeded byte-identical MacIntel canvas profile)', time: '+40ms' },
      { text: '$ oya start --persona acme-ops (Spawning sandbox on provider priority 0)', time: '+90ms' },
      { text: '$ oya goto https://app.example.com (Page loaded in 190ms, 0 challenge blocks)', time: '+175ms' },
      { text: '$ oya stealth-test --live -> CreepJS headless 0% · 0 lies · Sannysoft 31/31', time: '+265ms' },
      { text: '$ oya ls -> 1 active browser, status: Healthy', time: '+310ms' },
    ],
    mockResult: `ID        PROVIDER    PERSONA    PAGE                    STATUS    UPTIME
7f02a9    Oya Cloud   acme-ops   app.example.com/invoice Ready     42s

Evasion report (oya stealth-test --live):
  CreepJS headless score: 0%   (bare headless Chrome: 100%)
  CreepJS lies detected:  0
  Bot.Sannysoft:          31 / 31 pass
  Oya probe suite:        64 / 64 (29 probes, weighted)`,
    telemetry: {
      latency: '11ms',
      memory: '38 MB',
      provider: 'Oya Cloud',
      creepJsHeadless: '0% (bare: 100%)',
      status: 'CLI 0 OK',
    },
  },
  {
    label: 'MCP',
    language: 'json' as const,
    file: 'mcp.json',
    code: `{
  "mcpServers": {
    "oya-browser": {
      "url": "https://browser.getoya.ai/mcp/pool",
      "headers": {
        "Authorization": "Bearer <your-oya-api-key>"
      }
    }
  }
}`,
    testSteps: [
      { text: 'Validating Model Context Protocol client headers...', time: '+6ms' },
      { text: 'Handshaking with streamable-http gateway at /mcp/pool...', time: '+35ms' },
      { text: 'Registering 15 browser tools (analyze_page, click, type, press_key...)...', time: '+78ms' },
      { text: 'Fleet pool round-robin healthy (1,000 capacity target)...', time: '+120ms' },
      { text: 'MCP ready for Claude Code, Cursor, Windsurf, and LangChain.', time: '+180ms' },
    ],
    mockResult: `{
  "protocolVersion": "2024-11-05",
  "serverInfo": {
    "name": "oya-browser-control-plane",
    "version": "1.0.46"
  },
  "capabilities": {
    "tools": { "listChanged": true },
    "resources": { "subscribe": true }
  },
  "status": "ready"
}`,
    telemetry: {
      latency: '9ms',
      memory: '24 MB',
      provider: 'Multi-Runner Pool',
      creepJsHeadless: '0% (bare: 100%)',
      status: 'MCP Active',
    },
  },
];

/* ─────────────────────────────────────────────────────────────────────────────
   Demo Fleet Browsers (Interactive Preview Console)
───────────────────────────────────────────────────────────────────────────── */
const demoBrowsers = [
  {
    name: 'Production Analyst',
    provider: 'Oya Cloud',
    profile: 'us-east-finance',
    page: 'app.snowflake.com/query',
    action: 'Executing SQL aggregation',
    health: 'Running',
    routing: 'Primary route · 14ms',
    elements: 42,
  },
  {
    name: 'Operations',
    provider: 'Steel',
    profile: 'acme-ops',
    page: 'shop.example',
    action: 'Waiting for your next command',
    health: 'Ready',
    routing: 'Failover route · 28ms',
    elements: 18,
  },
  {
    name: 'Executive Assistant',
    provider: 'Desktop Paired',
    profile: 'exec-gsuite',
    page: 'mail.google.com',
    action: 'Authenticated via 1-click desktop session',
    health: 'Ready',
    routing: 'Direct workstation · 0ms',
    elements: 64,
  },
  {
    name: 'Compliance Auditor',
    provider: 'Anchor',
    profile: 'eu-gdpr',
    page: 'procure.corp.de/orders',
    action: 'Turnstile challenge auto-bypassed',
    health: 'Ready',
    routing: 'Encrypted proxy · 31ms',
    elements: 27,
  },
];

/* ─────────────────────────────────────────────────────────────────────────────
   Interactive Live Product Preview Component (Responsive & Mobile-Refined)
───────────────────────────────────────────────────────────────────────────── */
function ProductPreview() {
  const [selected, setSelected] = useState(0);
  const [simulatedCmd, setSimulatedCmd] = useState('');
  const [isTakeover, setIsTakeover] = useState(false);
  const browser = demoBrowsers[selected];

  useEffect(() => {
    const commands = [
      'analyze_page() → 24 elements indexed',
      'click(13) → [#13 button "Checkout"]',
      'type(9, "finance@acme.corp")',
      'solve_captcha() → Cloudflare Turnstile bypassed [340ms]',
    ];
    let idx = 0;
    const interval = setInterval(() => {
      setSimulatedCmd(commands[idx % commands.length]);
      idx++;
    }, 3200);
    return () => clearInterval(interval);
  }, [selected]);

  return (
    <div className="double-bezel overflow-hidden">
      <div className="double-bezel-inner product-preview bg-bg-card/95 backdrop-blur-xl" aria-label="Interactive browser console preview">
        {/* Top Control Bar */}
        <div className="flex flex-wrap items-center justify-between border-b border-border/80 px-4 py-3 sm:px-5 sm:py-3.5 bg-bg-elevated/40 gap-2">
          <div className="flex items-center gap-2 sm:gap-2.5 min-w-0">
            <OyaLogo size={18} />
            <span className="text-[12.5px] sm:text-[13px] font-semibold tracking-tight">Control Plane</span>
            <ChevronRight size={12} className="text-text-dim shrink-0" />
            <span className="text-[11.5px] sm:text-[12px] text-text-muted truncate max-w-[140px] sm:max-w-none">
              Fleet Console (1,000+ cap)
            </span>
          </div>
          <span className="inline-flex items-center gap-1.5 rounded-full border border-accent/30 bg-accent/10 px-2.5 py-0.5 font-mono text-[9.5px] sm:text-[10px] font-medium text-accent">
            <span className="h-1.5 w-1.5 rounded-full bg-accent animate-pulse" />
            ACTIVE · 99.99%
          </span>
        </div>

        <div className="grid min-w-0 md:grid-cols-[1.35fr_1.15fr]">
          {/* Left: Active Fleet List */}
          <div className="min-w-0 md:border-r border-border/80">
            <div className="flex items-center justify-between px-4 py-3 sm:px-5 sm:py-4 border-b border-border/50">
              <div className="flex items-center">
                <span className="text-[13.5px] sm:text-[14px] font-semibold text-text">Fleet Browsers</span>
                <span className="ml-2 rounded bg-text/10 px-1.5 py-0.2 font-mono text-[10.5px] text-text-dim">
                  04 / 1k
                </span>
              </div>
              <span className="flex items-center gap-1.5 text-[10.5px] sm:text-[11px] text-text-muted font-medium">
                <span className="h-1.5 w-1.5 rounded-full bg-accent shrink-0" />
                Auto-Routing
              </span>
            </div>

            {/* Desktop Table Headers */}
            <div className="preview-columns preview-columns-header border-b border-border/60 bg-bg-sunken/60 py-2 text-[10px] uppercase tracking-widest text-text-dim">
              <span>Browser / Persona</span>
              <span>Provider</span>
              <span>Status</span>
            </div>

            {/* List Rows */}
            <div className="divide-y divide-border/50">
              {demoBrowsers.map((row, index) => (
                <button
                  key={row.name}
                  onClick={() => {
                    setSelected(index);
                    setIsTakeover(false);
                  }}
                  aria-pressed={index === selected}
                  className={`w-full px-4 py-3 sm:py-3.5 text-left transition-all duration-200 ${
                    index === selected
                      ? 'bg-accent/[0.09] border-l-2 border-l-accent'
                      : 'hover:bg-text/[0.04] border-l-2 border-l-transparent'
                  }`}
                >
                  <div className="flex items-center justify-between gap-2">
                    <div className="flex min-w-0 items-center gap-3">
                      <Monitor
                        size={15}
                        className={index === selected ? 'text-accent shrink-0' : 'text-text-dim shrink-0'}
                      />
                      <div className="min-w-0">
                        <span className="block truncate text-[12.5px] font-medium text-text">{row.name}</span>
                        <span className="block truncate font-mono text-[10px] text-text-dim">
                          {row.profile}
                        </span>
                      </div>
                    </div>
                    <div className="flex items-center gap-2 shrink-0">
                      <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-bg-sunken border border-border/60 text-text-muted">
                        {row.provider}
                      </span>
                      <span className="inline-flex items-center gap-1 text-[10px] font-semibold text-accent">
                        <span className="h-1.5 w-1.5 rounded-full bg-accent animate-ping" />
                        <span className="hidden sm:inline">{row.health}</span>
                      </span>
                    </div>
                  </div>
                </button>
              ))}
            </div>

            <div className="flex flex-wrap items-center justify-between border-t border-border/50 px-4 py-2.5 sm:px-5 sm:py-3 text-[10px] sm:text-[10.5px] text-text-dim bg-bg-sunken/40 gap-2">
              <span className="flex items-center gap-1.5 truncate">
                <Command size={11} className="text-accent shrink-0" /> Unified Gateway: One API key
              </span>
              <span className="font-mono text-accent shrink-0">Failover: ACTIVE</span>
            </div>
          </div>

          {/* Right: Live Interactive Viewport */}
          <div className="flex min-w-0 flex-col p-4 sm:p-5 bg-bg-elevated/20 border-t md:border-t-0 border-border/80">
            <div className="flex flex-wrap items-center justify-between gap-2 mb-3">
              <div className="min-w-0">
                <span className="truncate text-[13px] sm:text-[13.5px] font-semibold text-text block">{browser.name}</span>
                <span className="text-[9.5px] sm:text-[10px] text-text-dim font-mono">{browser.routing}</span>
              </div>
              <button
                onClick={() => setIsTakeover(!isTakeover)}
                className={`text-[9.5px] sm:text-[10px] font-mono px-2.5 py-1 rounded-full border transition-all ${
                  isTakeover
                    ? 'border-accent bg-accent text-bg font-bold'
                    : 'border-accent/40 bg-accent/10 text-accent hover:bg-accent/20'
                }`}
              >
                {isTakeover ? 'TAKEN OVER' : 'STEP IN & TAKE OVER'}
              </button>
            </div>

            {/* Simulated Browser Frame */}
            <div className="rounded-xl border border-border/80 bg-bg-sunken overflow-hidden shadow-inner flex flex-col flex-1">
              {/* Address bar */}
              <div className="flex items-center gap-2 border-b border-border/60 bg-bg-elevated/80 px-3 py-1.5 text-[10.5px] sm:text-[11px] font-mono text-text-muted">
                <Lock size={10} className="text-accent shrink-0" />
                <span className="truncate text-text-secondary">{browser.page}</span>
                <span className="ml-auto text-[9px] text-accent/80 font-mono shrink-0">24ms CDP</span>
              </div>

              {/* Viewport Content */}
              <div className="relative flex-1 p-4 sm:p-5 flex flex-col items-center justify-center text-center min-h-[150px] bg-gradient-to-b from-bg-sunken to-bg-card">
                <div className="relative mb-2.5">
                  <Globe2 size={28} strokeWidth={1.2} className="text-accent/60 mx-auto" />
                  <motion.div
                    animate={{ scale: [1, 1.25, 1], opacity: [0.3, 0.7, 0.3] }}
                    transition={{ repeat: Infinity, duration: 2.5 }}
                    className="absolute -inset-2 rounded-full border border-accent/40 pointer-events-none"
                  />
                </div>
                <p className="text-[12.5px] sm:text-[13px] font-medium text-text">Sub-second Interactive Live View</p>
                <p className="mt-1 text-[10.5px] sm:text-[11px] text-text-dim max-w-[260px] leading-relaxed">
                  {isTakeover
                    ? 'Keyboard captured. Keystrokes forwarded into remote session.'
                    : 'Click, scroll, or type. Step in when 2FA hits; hand back to agent.'}
                </p>

                {/* Simulated DOM Highlights */}
                <div className="mt-3 flex flex-wrap items-center justify-center gap-1.5 font-mono text-[9px]">
                  <span className="rounded border border-indigo/40 bg-indigo/10 px-1.5 py-0.5 text-indigo">
                    [#13 button &quot;Checkout&quot;]
                  </span>
                  <span className="rounded border border-accent/40 bg-accent/10 px-1.5 py-0.5 text-accent">
                    [#9 input:email]
                  </span>
                </div>
              </div>

              {/* Status footer */}
              <div className="border-t border-border/60 bg-bg-elevated/60 px-3 py-2 sm:px-4 flex items-center justify-between text-[10.5px] sm:text-[11px] text-text-muted gap-2">
                <p aria-live="polite" className="flex items-center gap-1.5 truncate">
                  <span className="h-1.5 w-1.5 rounded-full bg-accent animate-pulse shrink-0" />
                  <span className="font-mono text-[10px] sm:text-[10.5px] truncate">{browser.action}</span>
                </p>
                {simulatedCmd && (
                  <span className="hidden sm:inline-block font-mono text-[9px] text-text-dim truncate max-w-[170px]">
                    {simulatedCmd}
                  </span>
                )}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ─────────────────────────────────────────────────────────────────────────────
   Benchmarks on Startup Time, Tokens & Evasion
───────────────────────────────────────────────────────────────────────────── */
const benchmarkCategories = [
  {
    id: 'stealth',
    label: 'Bot evasion, measured',
    icon: ShieldCheck,
    headline: '0% CreepJS headless, 0 lies, 31 / 31 Bot.Sannysoft',
    highlight: 'Reproducible',
    summary:
      '"Zero detection" is neither measurable nor achievable — the published leader sits near 77% bypass. So this is a number instead: the same headless Chrome launched twice, once bare and once with a persona applied exactly as production does, both facing the public detectors. Chrome 153 on macOS. Run it yourself with `oya stealth-test --live`.',
    metrics: [
      { name: 'Oya persona — Oya probe suite (29 probes, weighted)', time: '64 / 64', pct: 100, winner: true },
      { name: 'Oya persona — Bot.Sannysoft', time: '31 / 31', pct: 100, winner: false },
      { name: 'Bare headless Chrome — Bot.Sannysoft', time: '27 / 31', pct: 87, winner: false },
      { name: 'Bare headless Chrome — Oya probe suite', time: '55 / 64', pct: 86, winner: false },
    ],
  },
];

function BenchmarksSection() {
  const [activeCat, setActiveCat] = useState(0);
  const cat = benchmarkCategories[activeCat];

  return (
    <section id="benchmarks" className="site-width section-space scroll-mt-16 border-t border-border/80">
      <div className="flex flex-wrap items-end justify-between gap-4 mb-8">
        <div>
          <p className="eyebrow mb-2 sm:mb-3 text-accent flex items-center gap-2">
            <BarChart3 size={13} className="text-accent" />
            Verified Benchmarks
          </p>
          <h2 className="marketing-heading">Hard numbers. Zero marketing fluff.</h2>
          <p className="text-[14px] sm:text-[16px] text-text-muted mt-2 max-w-xl">
            Compare latency, LLM token efficiency, anti-bot evasion scores, and concurrency density against traditional runners.
          </p>
        </div>
        <span className="font-mono text-[11px] text-accent bg-accent/10 border border-accent/30 rounded-full px-3 py-1">
          Open benchmark suite: oya stealth-test --live
        </span>
      </div>

      {/* Category Pills (Swipeable on mobile) */}
      <div className="flex items-center gap-2 overflow-x-auto pb-3 mb-6 scrollbar-none">
        {benchmarkCategories.map((c, idx) => {
          const Icon = c.icon;
          return (
            <button
              key={c.id}
              role="tab"
              aria-selected={activeCat === idx}
              onClick={() => setActiveCat(idx)}
              className={`rounded-xl px-4 py-2.5 text-[12px] sm:text-[13px] font-semibold whitespace-nowrap transition-all duration-200 flex items-center gap-2 shrink-0 ${
                activeCat === idx
                  ? 'bg-accent text-bg shadow-[0_0_20px_rgba(57,237,53,0.3)]'
                  : 'border border-border/80 bg-bg-card/70 text-text-muted hover:text-text hover:bg-bg-elevated'
              }`}
            >
              <Icon size={14} className={activeCat === idx ? 'text-bg' : 'text-accent'} />
              {c.label}
            </button>
          );
        })}
      </div>

      {/* Benchmark Display Card (Double-Bezel) */}
      <div className="double-bezel overflow-hidden">
        <div className="double-bezel-inner bg-bg-card p-5 sm:p-8 rounded-[calc(1.5rem-3px)] border border-border/80 shadow-2xl">
          <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border/60 pb-5 mb-6">
            <div>
              <span className="text-[16px] sm:text-[20px] font-bold text-text block">
                {cat.headline}
              </span>
              <p className="text-[12.5px] sm:text-[13.5px] text-text-muted mt-1 max-w-2xl leading-relaxed">
                {cat.summary}
              </p>
            </div>
            <div className="rounded-2xl border border-accent/40 bg-accent/10 px-4 py-2 text-center shrink-0">
              <span className="block text-[20px] sm:text-[24px] font-bold text-accent font-mono">
                {cat.highlight}
              </span>
              <span className="block text-[10px] uppercase font-mono text-text-dim">
                Architectural Lead
              </span>
            </div>
          </div>

          {/* Comparative Progress Bars */}
          <div className="space-y-4">
            {cat.metrics.map((m, i) => (
              <div key={i} className="space-y-1.5">
                <div className="flex items-center justify-between text-[12px] sm:text-[13px] font-medium">
                  <span className={m.winner ? 'text-text font-semibold flex items-center gap-1.5' : 'text-text-muted'}>
                    {m.winner && <CheckCircle2 size={13} className="text-accent shrink-0" />}
                    {m.name}
                  </span>
                  <span className={`font-mono font-bold ${m.winner ? 'text-accent' : 'text-text-dim'}`}>
                    {m.time}
                  </span>
                </div>
                <div className="h-3 w-full rounded-full bg-bg-sunken border border-border/60 overflow-hidden p-0.5">
                  <motion.div
                    initial={{ width: 0 }}
                    animate={{ width: `${m.pct}%` }}
                    transition={{ duration: 0.8, ease: [0.16, 1, 0.3, 1] }}
                    className={`h-full rounded-full ${
                      m.winner
                        ? 'bg-accent shadow-[0_0_12px_rgba(57,237,53,0.6)]'
                        : 'bg-text/25'
                    }`}
                  />
                </div>
              </div>
            ))}
          </div>

          <div className="mt-6 pt-4 border-t border-border/50 flex flex-wrap items-center justify-between text-[11px] font-mono text-text-dim gap-2">
            <span>Methodology: Benchmarked over 1,000 requests on AWS us-east-1 against live Bot.Sannysoft and CreepJS suite.</span>
            <span className="text-accent">Reproducible in CLI</span>
          </div>
        </div>
      </div>
    </section>
  );
}

/* ─────────────────────────────────────────────────────────────────────────────
   Enterprise & Autonomous Agent Use Cases
───────────────────────────────────────────────────────────────────────────── */
const useCases = [
  {
    id: 'procurement',
    title: 'Autonomous Procurement & Enterprise ERP',
    subtitle: 'SAP · Coupa · NetSuite · Amazon Business',
    badge: 'ENTERPRISE AGENTS',
    icon: Briefcase,
    challenge: 'Enterprise portals enforce corporate Okta SSO, push notifications, and Cloudflare Turnstile that kill conventional headless scrapers.',
    solution: 'Employees sign in once via Oya Desktop; cookies and passkeys sync to cloud agent personas. Two-tier solver automatically clears Turnstile challenges.',
    metrics: '100% task completion · Zero stored plaintext credentials',
  },
  {
    id: 'intelligence',
    title: 'High-Frequency Intelligence & Anti-Ban Scraping',
    subtitle: 'Market Intelligence · Price Monitoring · Public Registries',
    badge: 'DATA EXTRACTION',
    icon: Globe2,
    challenge: 'Target platforms track canvas noise, WebGL hashes, and IP reputation, banning bots after 20 requests.',
    solution: 'Personas bind deterministic hardware profiles to dedicated residential proxy exits with concurrency caps. Returning sessions look like recurring legitimate workstations.',
    metrics: '0% CreepJS headless · 31 / 31 Bot.Sannysoft · one pinned proxy exit per persona',
  },
  {
    id: 'coding-agents',
    title: 'AI Coding & Browser Tools (Claude Code, Cursor, Windsurf)',
    subtitle: 'Automated Web Verification · Browser-Use · LangChain',
    badge: 'DEVELOPER WORKFLOW',
    icon: Terminal,
    challenge: 'Agents hallucinate CSS selectors and waste 18,000 tokens per page reading bloated HTML.',
    solution: 'Universal MCP server returns structured markdown and numbered element IDs. The AI reads [#13 button "Save"] and calls click(13).',
    metrics: '85% LLM token savings · ~450 tokens/page snapshot',
  },
  {
    id: 'qa-testing',
    title: 'Multi-Provider QA & Synthetic Fleet Monitoring',
    subtitle: 'Playwright · Puppeteer · Stagehand · Cypress',
    badge: 'CONTINUOUS TESTING',
    icon: ShieldCheck,
    challenge: 'Single cloud runner outages (Browserbase downtime or Steel rate limits) crash production CI/CD test pipelines.',
    solution: 'Connect existing Playwright tests directly to wss://browser.getoya.ai/connect. If a runner stumbles, Oya auto-routes to healthy backups in 14ms.',
    metrics: 'Zero pipeline downtime · Sub-14ms automatic failover',
  },
];

function UseCasesSection() {
  return (
    <section id="use-cases" className="site-width section-space scroll-mt-16 border-t border-border/80">
      <div className="grid gap-6 lg:grid-cols-[1fr_1.35fr] mb-10">
        <div>
          <p className="eyebrow mb-2 sm:mb-3 text-accent">Production Applications</p>
          <h2 className="marketing-heading max-w-sm">
            Built for agents that cannot fail.
          </h2>
        </div>
        <p className="max-w-xl text-[15px] sm:text-[17px] leading-[1.75] text-text-muted lg:pt-9">
          From high-stakes enterprise procurement to large-scale data intelligence and autonomous QA, see how teams run resilient browser fleets on Oya.
        </p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-5 sm:gap-6">
        {useCases.map((u) => {
          const Icon = u.icon;
          return (
            <div
              key={u.id}
              className="rounded-2xl sm:rounded-3xl border border-border/80 bg-bg-card p-6 sm:p-7 shadow-xl hover:border-accent/40 transition-all flex flex-col justify-between"
            >
              <div>
                <div className="flex items-center justify-between mb-4">
                  <div className="h-10 w-10 rounded-xl bg-accent/10 border border-accent/25 flex items-center justify-center text-accent">
                    <Icon size={19} />
                  </div>
                  <span className="font-mono text-[9.5px] uppercase tracking-wider px-2.5 py-1 rounded-full border border-border/80 bg-bg-sunken text-text-dim font-medium">
                    {u.badge}
                  </span>
                </div>

                <h3 className="text-[17px] sm:text-[19px] font-bold text-text tracking-tight">
                  {u.title}
                </h3>
                <p className="font-mono text-[11px] text-accent mt-0.5 mb-3">
                  {u.subtitle}
                </p>

                <div className="space-y-2.5 text-[12.5px] sm:text-[13.5px] leading-relaxed text-text-muted">
                  <p>
                    <strong className="text-text-secondary font-semibold">Challenge: </strong>
                    {u.challenge}
                  </p>
                  <p>
                    <strong className="text-accent font-semibold">Solution: </strong>
                    {u.solution}
                  </p>
                </div>
              </div>

              <div className="mt-5 pt-4 border-t border-border/60 flex items-center justify-between font-mono text-[11px] sm:text-[11.5px] text-accent font-medium">
                <span>{u.metrics}</span>
                <ChevronRight size={14} className="text-text-dim" />
              </div>
            </div>
          );
        })}
      </div>
    </section>
  );
}

/* ─────────────────────────────────────────────────────────────────────────────
   Frequently Asked Questions (FAQs) Accordion Component
───────────────────────────────────────────────────────────────────────────── */
const faqs = [
  {
    q: 'How does Oya differ from browser runners like Browserbase, Steel, and Anchor?',
    a: 'Browserbase, Steel, Anchor, and Browser Use are execution targets — they spin up and bill for isolated Chromium containers. Oya is the Control Plane sitting above them. Oya unifies your entire fleet behind a single API key, provides deterministic seeded personas (preventing anti-bot flags), synchronizes desktop passkeys, offers two-tier CAPTCHA solving, and automatically fails over between providers if one throttles or experiences an outage.',
  },
  {
    q: 'What is a "Deterministic Persona" and why does it prevent bot bans?',
    a: 'Anti-bot algorithms flag two patterns: one account seen from 50 different device fingerprints (bot farm), or one device fingerprint seen on 1,000 accounts (device farm). Oya personas derive canvas, WebGL, audio, and client rects from a cryptographic seed, guaranteeing byte-identical hardware profiles across restarts. Each persona is permanently bound to a dedicated cookie jar and proxy IP with an enforced concurrency cap. Returning weeks later looks like the exact same legitimate workstation.',
  },
  {
    q: 'How does Oya reduce LLM token consumption by up to 85%?',
    a: 'Traditional browser automation dumps the full raw HTML DOM (15,000 to 25,000 tokens) or takes full-screen screenshots that burn 1,600 vision tokens per frame. Oya injects an in-memory Chromium analyzer that parses the active DOM and returns clean, structured markdown with numbered interactive element IDs (e.g. [#13 button "Submit"]). The agent reads ~450 tokens and simply replies click(13).',
  },
  {
    q: 'How does Sign-In-Once Desktop Pairing work without storing passwords?',
    a: 'You install the native Oya desktop app and log into your services using real passkeys, WebAuthn, Google SSO, or corporate Okta. Oya securely extracts the authenticated session cookies, encrypts them at rest with your profile master secret, and synchronizes them to your cloud personas via ephemeral pairing codes. Your cloud agents wake up already logged in, without brittle login scripts or stored plaintext credentials.',
  },
  {
    q: 'What happens during a provider outage or quota exhaustion?',
    a: 'Oya routes connections dynamically based on configured priority and concurrency limits. If your primary runner (e.g. Browserbase) experiences an outage, 502 error, or rate limit, Oya’s gateway detects the upstream fault in < 4ms and auto-migrates the connection to your secondary provider (e.g. Steel or Oya Cloud) in 14ms. Your Playwright, Puppeteer, or MCP code never crashes.',
  },
  {
    q: 'Can I self-host Oya in my own VPC or air-gapped infrastructure?',
    a: 'Yes. Oya is 100% self-hostable with `docker compose up`. You can bring your own bare-metal Chrome, private Kubernetes cluster, or Oya Cloud runners. All session state, credentials, and settings are encrypted on your local storage volume using your own master key.',
  },
  {
    q: 'How does interactive live stream takeover work during autonomous runs?',
    a: 'Every browser exposes an interactive SSE / WebSocket live stream. When an agent encounters an unexpected step — such as an SMS verification code, push approval, or CAPTCHA — a human operator can open the live view, click or type directly with zero noticeable lag, and hand control right back to the agent.',
  },
];

function FaqSection() {
  const [openIdx, setOpenIdx] = useState<number | null>(0);

  return (
    <section id="faq" className="site-width section-space scroll-mt-16 border-t border-border/80">
      <div className="grid gap-6 lg:grid-cols-[1fr_1.35fr] mb-10">
        <div>
          <p className="eyebrow mb-2 sm:mb-3 text-accent flex items-center gap-2">
            <HelpCircle size={13} className="text-accent" />
            Frequently Asked Questions
          </p>
          <h2 className="marketing-heading max-w-sm">
            Everything you need to know.
          </h2>
        </div>
        <p className="max-w-xl text-[15px] sm:text-[17px] leading-[1.75] text-text-muted lg:pt-9">
          Detailed answers on architecture, deterministic personas, anti-bot mitigation, token economics, and multi-provider failover.
        </p>
      </div>

      <div className="space-y-3 max-w-4xl mx-auto">
        {faqs.map((f, idx) => {
          const isOpen = openIdx === idx;
          return (
            <div
              key={idx}
              className="rounded-2xl border border-border/80 bg-bg-card/90 overflow-hidden shadow-md transition-colors hover:border-accent/30"
            >
              <button
                onClick={() => setOpenIdx(isOpen ? null : idx)}
                aria-expanded={isOpen}
                className="w-full p-5 sm:p-6 text-left flex items-center justify-between gap-4 transition-colors hover:bg-text/[0.02]"
              >
                <span className="text-[14px] sm:text-[16px] font-semibold text-text leading-snug">
                  {f.q}
                </span>
                <span className="rounded-full bg-bg-sunken border border-border/70 p-1.5 text-text-muted shrink-0">
                  {isOpen ? <ChevronDown size={14} className="rotate-180 transition-transform" /> : <ChevronDown size={14} className="transition-transform" />}
                </span>
              </button>

              <AnimatePresence>
                {isOpen && (
                  <motion.div
                    initial={{ height: 0, opacity: 0 }}
                    animate={{ height: 'auto', opacity: 1 }}
                    exit={{ height: 0, opacity: 0 }}
                    transition={{ duration: 0.25, ease: 'easeInOut' }}
                    className="overflow-hidden"
                  >
                    <div className="px-5 pb-5 sm:px-6 sm:pb-6 text-[13px] sm:text-[14.5px] leading-[1.8] text-text-muted border-t border-border/50 pt-4">
                      {f.a}
                    </div>
                  </motion.div>
                )}
              </AnimatePresence>
            </div>
          );
        })}
      </div>
    </section>
  );
}

/* ─────────────────────────────────────────────────────────────────────────────
   Animated Video & Motion Showcase Studio (Fully Responsive)
───────────────────────────────────────────────────────────────────────────── */
function VideoMotionStudio() {
  const [activeTab, setActiveTab] = useState(0);
  const [isPlaying, setIsPlaying] = useState(true);
  const [scrubPercent, setScrubPercent] = useState(35);
  const videoRef = useRef<HTMLVideoElement>(null);

  const scenes = [
    {
      id: 'walkthrough',
      title: '01 / Fleet Walkthrough',
      badge: 'VIDEO',
      desc: 'Watch the Control Plane manage 1,000+ browsers across hybrid cloud and desktop pairing.',
      type: 'video',
    },
    {
      id: 'captcha',
      title: '02 / Autonomous CAPTCHA Bypass',
      badge: 'MOTION',
      desc: 'Cloudflare Turnstile and reCAPTCHA solved in < 380ms with native solver fallback.',
      type: 'simulation-captcha',
    },
    {
      id: 'takeover',
      title: '03 / Sub-Second Live Takeover',
      badge: 'MOTION',
      desc: 'Human-in-the-loop: when unexpected 2FA hits, step in with real clicks and hand back.',
      type: 'simulation-takeover',
    },
    {
      id: 'failover',
      title: '04 / Zero-Downtime Provider Failover',
      badge: 'MOTION',
      desc: 'When an upstream runner 502s, sessions auto-migrate to healthy runners in 14ms.',
      type: 'simulation-failover',
    },
  ];

  // Auto-scrubber for simulation tabs
  useEffect(() => {
    if (!isPlaying || activeTab === 0) return;
    const interval = setInterval(() => {
      setScrubPercent((prev) => (prev >= 100 ? 0 : prev + 1.5));
    }, 100);
    return () => clearInterval(interval);
  }, [isPlaying, activeTab]);

  return (
    <div className="mt-8">
      {/* Horizontally Scrollable Scene Navigation Bar */}
      <div className="flex items-center gap-2 overflow-x-auto pb-3 mb-4 scrollbar-none">
        {scenes.map((s, idx) => (
          <button
            key={s.id}
            onClick={() => {
              setActiveTab(idx);
              setScrubPercent(15);
            }}
            className={`rounded-full px-3.5 py-1.5 sm:px-4 sm:py-2 text-[11.5px] sm:text-[12.5px] font-medium whitespace-nowrap transition-all duration-300 flex items-center gap-2 shrink-0 ${
              activeTab === idx
                ? 'bg-accent text-bg shadow-[0_0_24px_rgba(57,237,53,0.35)]'
                : 'border border-border/80 bg-bg-card/70 text-text-muted hover:text-text hover:bg-bg-elevated'
            }`}
          >
            <span
              className={`text-[9px] font-mono px-1.5 py-0.2 rounded ${
                activeTab === idx ? 'bg-bg/20 text-bg font-bold' : 'bg-text/10 text-accent'
              }`}
            >
              {s.badge}
            </span>
            {s.title}
          </button>
        ))}
      </div>

      {/* Main Showcase Theater (Double-Bezel Architecture) */}
      <div className="double-bezel overflow-hidden">
        <div className="double-bezel-inner rounded-[calc(1.5rem-3px)] bg-bg-card border border-border/70 overflow-hidden shadow-2xl">
          {/* Header Bar */}
          <div className="flex items-center justify-between border-b border-border/70 px-4 py-2.5 sm:px-5 sm:py-3 bg-bg-elevated/40 gap-2">
            <div className="flex items-center gap-2 min-w-0">
              <span className="flex h-2 w-2 rounded-full bg-red animate-pulse shrink-0" />
              <span className="font-mono text-[10.5px] sm:text-[11px] uppercase tracking-wider text-text font-medium truncate">
                {scenes[activeTab].title}
              </span>
            </div>
            <div className="flex items-center gap-2 text-[10px] sm:text-[11px] text-text-dim font-mono shrink-0">
              <span>HD</span>
              <span>·</span>
              <span className="text-accent font-semibold">60 FPS</span>
            </div>
          </div>

          {/* Player Container */}
          <div className="relative aspect-video w-full bg-black/90 flex items-center justify-center overflow-hidden">
            {activeTab === 0 ? (
              /* Tab 0: Real HD Video Walkthrough */
              <video
                ref={videoRef}
                controls
                playsInline
                preload="metadata"
                poster="/oya-browser-poster.jpg"
                aria-label="Oya Browser product walkthrough"
                className="h-full w-full object-contain"
              >
                <source src="/oya-browser.mp4" type="video/mp4" />
              </video>
            ) : activeTab === 1 ? (
              /* Tab 1: CAPTCHA Bypass Video Simulation */
              <div className="w-full h-full p-4 sm:p-6 flex flex-col justify-between bg-gradient-to-br from-[#0c0c0a] via-[#141410] to-[#0a0a08] text-left">
                <div className="flex items-center justify-between border-b border-white/10 pb-2.5 gap-2">
                  <div className="flex items-center gap-2 font-mono text-[11px] sm:text-[12px] text-text-muted truncate">
                    <ShieldCheck size={15} className="text-accent shrink-0" />
                    <span className="truncate">https://protected.acme.internal</span>
                  </div>
                  <span className="font-mono text-[9.5px] sm:text-[11px] text-accent bg-accent/10 px-2 py-0.5 rounded-full shrink-0">
                    TURNSTILE MITIGATION
                  </span>
                </div>

                <div className="max-w-md mx-auto w-full rounded-xl sm:rounded-2xl border border-white/10 bg-bg-card/90 p-4 sm:p-6 shadow-2xl backdrop-blur-xl my-auto">
                  <div className="flex items-center justify-between mb-3">
                    <span className="text-[12.5px] sm:text-[14px] font-semibold">Cloudflare Turnstile Verification</span>
                    <span className="font-mono text-[9.5px] text-text-dim">ID: cf_90f2a</span>
                  </div>

                  <div className="rounded-lg sm:rounded-xl border border-border p-3 sm:p-4 bg-bg-sunken/80 flex items-center justify-between gap-2">
                    <div className="flex items-center gap-2.5 sm:gap-3 min-w-0">
                      {scrubPercent < 60 ? (
                        <div className="h-5 w-5 sm:h-6 sm:w-6 rounded-md border-2 border-accent border-t-transparent animate-spin shrink-0" />
                      ) : (
                        <div className="h-5 w-5 sm:h-6 sm:w-6 rounded-md bg-accent text-bg flex items-center justify-center shrink-0">
                          <Check size={14} strokeWidth={3} />
                        </div>
                      )}
                      <div className="min-w-0">
                        <p className="text-[11.5px] sm:text-[12.5px] font-medium text-text truncate">
                          {scrubPercent < 30
                            ? 'Analyzing challenge...'
                            : scrubPercent < 60
                            ? 'Synthesizing gesture token...'
                            : 'Challenge Verified (340ms)'}
                        </p>
                        <p className="text-[9.5px] sm:text-[10px] text-text-dim font-mono truncate">
                          Seed: 0x39a1fe
                        </p>
                      </div>
                    </div>
                    <span className="font-mono text-[10px] sm:text-[11px] text-accent font-semibold shrink-0">
                      {scrubPercent < 60 ? 'SOLVING' : 'SUCCESS'}
                    </span>
                  </div>
                </div>

                <div className="font-mono text-[10px] sm:text-[11px] text-text-muted flex items-center justify-between gap-2">
                  <span className="truncate">Autonomous solver bypass</span>
                  <span className="text-accent shrink-0">Solver: provider</span>
                </div>
              </div>
            ) : activeTab === 2 ? (
              /* Tab 2: Live Takeover Video Simulation */
              <div className="w-full h-full p-4 sm:p-6 flex flex-col justify-between bg-gradient-to-br from-[#0c0c0a] via-[#15131a] to-[#0c0c0a] text-left">
                <div className="flex items-center justify-between border-b border-white/10 pb-2.5 gap-2">
                  <div className="flex items-center gap-2 font-mono text-[11px] sm:text-[12px] text-indigo truncate">
                    <Radio size={15} className="text-indigo animate-pulse shrink-0" />
                    <span className="truncate">Live Stream Takeover</span>
                  </div>
                  <span className="font-mono text-[9.5px] sm:text-[11px] text-yellow bg-yellow/10 px-2 py-0.5 rounded-full shrink-0">
                    2FA HANDOFF
                  </span>
                </div>

                <div className="max-w-lg mx-auto w-full rounded-xl sm:rounded-2xl border border-white/10 bg-bg-card/90 p-4 sm:p-6 shadow-2xl backdrop-blur-xl my-auto">
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-[12.5px] sm:text-[14px] font-semibold">Okta Verify / Google Passkey Prompt</span>
                    <span className="rounded-full bg-yellow/15 text-yellow px-2 py-0.2 text-[9.5px] font-mono">
                      PENDING
                    </span>
                  </div>
                  <p className="text-[11px] sm:text-[12px] text-text-muted mb-3 leading-relaxed">
                    2FA prompt detected. Sub-second interactive stream activated. Click to approve:
                  </p>

                  <div className="flex items-center justify-center gap-4 py-2">
                    <button
                      className="rounded-lg sm:rounded-xl bg-accent px-4 py-2 sm:px-5 sm:py-2.5 text-[12px] sm:text-[13px] font-semibold text-bg flex items-center gap-2 shadow-lg"
                    >
                      <MousePointer size={14} />
                      Approve with Passkey (Human Click)
                    </button>
                  </div>
                </div>

                <div className="font-mono text-[10px] sm:text-[11px] text-text-muted flex items-center justify-between gap-2">
                  <span className="truncate">Handoff status: Approved by operator</span>
                  <span className="text-accent shrink-0">0 Leaks</span>
                </div>
              </div>
            ) : (
              /* Tab 3: Failover Simulation */
              <div className="w-full h-full p-4 sm:p-6 flex flex-col justify-between bg-gradient-to-br from-[#0c0c0a] via-[#101413] to-[#0c0c0a] text-left">
                <div className="flex items-center justify-between border-b border-white/10 pb-2.5 gap-2">
                  <div className="flex items-center gap-2 font-mono text-[11px] sm:text-[12px] text-accent truncate">
                    <Network size={15} className="text-accent shrink-0" />
                    <span className="truncate">Dynamic Routing Matrix</span>
                  </div>
                  <span className="font-mono text-[9.5px] sm:text-[11px] text-accent bg-accent/10 px-2 py-0.5 rounded-full shrink-0">
                    FAILOVER: 14ms
                  </span>
                </div>

                <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 max-w-xl mx-auto w-full my-auto">
                  <div className="rounded-lg sm:rounded-xl border border-red/40 bg-red/[0.04] p-3 sm:p-4">
                    <div className="flex items-center justify-between text-[10.5px] font-mono mb-1 text-red">
                      <span>RUNNER A (Browserbase)</span>
                      <XCircle size={13} />
                    </div>
                    <p className="text-[12px] sm:text-[13px] font-semibold text-text">HTTP 502 Timeout</p>
                    <p className="text-[10px] sm:text-[11px] text-text-dim">Detected in 4ms.</p>
                  </div>

                  <div className="rounded-lg sm:rounded-xl border border-accent/60 bg-accent/[0.08] p-3 sm:p-4 shadow-[0_0_20px_rgba(57,237,53,0.15)]">
                    <div className="flex items-center justify-between text-[10.5px] font-mono mb-1 text-accent">
                      <span>AUTO-ROUTED: Steel</span>
                      <CheckCircle2 size={13} />
                    </div>
                    <p className="text-[12px] sm:text-[13px] font-semibold text-text">Resumed · 14ms</p>
                    <p className="text-[10px] sm:text-[11px] text-text-dim">Zero lost commands.</p>
                  </div>
                </div>

                <div className="font-mono text-[10px] sm:text-[11px] text-text-muted flex items-center justify-between gap-2">
                  <span className="truncate">Auto-failover active</span>
                  <span className="text-accent shrink-0">Runners: 4 Healthy</span>
                </div>
              </div>
            )}
          </div>

          {/* Bottom Timeline & Controls Bar */}
          <div className="border-t border-border/70 bg-bg-elevated/80 px-4 py-2.5 sm:px-5 sm:py-3 flex flex-wrap items-center justify-between gap-3">
            <div className="flex items-center gap-2 sm:gap-3">
              <button
                onClick={() => setIsPlaying(!isPlaying)}
                className="btn-icon text-text hover:text-accent h-7 w-7"
                aria-label={isPlaying ? 'Pause demonstration' : 'Play demonstration'}
              >
                {isPlaying ? <Pause size={14} /> : <Play size={14} />}
              </button>
              <button
                onClick={() => setScrubPercent(0)}
                className="btn-icon text-text-dim hover:text-text h-7 w-7"
                aria-label="Restart demonstration"
              >
                <RotateCcw size={13} />
              </button>
              <span className="hidden sm:inline-block text-[11px] sm:text-[12px] text-text-muted font-mono truncate max-w-[200px] sm:max-w-md">
                {scenes[activeTab].desc}
              </span>
            </div>

            <div className="flex items-center gap-2 sm:gap-3 w-full sm:w-auto justify-between sm:justify-end">
              {activeTab !== 0 && (
                <div className="w-28 sm:w-44 h-1.5 rounded-full bg-white/10 overflow-hidden">
                  <div
                    className="h-full bg-accent transition-all duration-150"
                    style={{ width: `${scrubPercent}%` }}
                  />
                </div>
              )}
              <Link href="/dashboard" className="btn-primary h-7 sm:h-8 px-3 text-[11px] sm:text-[11.5px]">
                Try in console <ArrowRight size={12} />
              </Link>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ─────────────────────────────────────────────────────────────────────────────
   Interactive Live Code Snippet Tester (Mobile Optimized)
───────────────────────────────────────────────────────────────────────────── */
function CodeSnippetTester() {
  const [example, setExample] = useState(0);
  const [copied, setCopied] = useState(false);
  const [copyError, setCopyError] = useState('');
  const [isTesting, setIsTesting] = useState(false);
  const [testComplete, setTestComplete] = useState(false);
  const [testView, setTestView] = useState<'logs' | 'output' | 'telemetry'>('logs');
  const [activeStep, setActiveStep] = useState(0);

  const snippet = examples[example];

  async function copyCode() {
    try {
      await navigator.clipboard.writeText(snippet.code);
      setCopied(true);
      setTimeout(() => setCopied(false), 1800);
    } catch {
      setCopyError('Select the code below to copy it.');
    }
  }

  function runSnippetTest() {
    setIsTesting(true);
    setTestComplete(false);
    setActiveStep(0);

    let step = 0;
    const interval = setInterval(() => {
      step++;
      setActiveStep(step);
      if (step >= snippet.testSteps.length) {
        clearInterval(interval);
        setIsTesting(false);
        setTestComplete(true);
      }
    }, 280);
  }

  return (
    <div className="min-w-0 overflow-hidden rounded-2xl border border-border/80 bg-bg-card shadow-2xl">
      {/* Code Header with Horizontal Scroll Tabs */}
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between border-b border-border/70 p-2 sm:px-3 sm:py-2 bg-bg-elevated/60 gap-2">
        <div role="tablist" aria-label="Integration language" className="flex gap-1 overflow-x-auto pb-1 sm:pb-0 scrollbar-none">
          {examples.map((item, i) => (
            <button
              key={item.label}
              role="tab"
              aria-selected={example === i}
              aria-controls="integration-code"
              onClick={() => {
                setExample(i);
                setCopied(false);
                setCopyError('');
                setIsTesting(false);
                setTestComplete(false);
                setActiveStep(0);
              }}
              className={`rounded-lg px-2.5 py-1.5 text-[11px] sm:text-[12px] whitespace-nowrap transition-all duration-150 shrink-0 ${
                example === i
                  ? 'bg-text/10 text-text font-semibold border border-border/60'
                  : 'text-text-dim hover:text-text hover:bg-text/5'
              }`}
            >
              {item.label}
            </button>
          ))}
        </div>

        <div className="flex items-center justify-between sm:justify-end gap-2 pt-1 sm:pt-0 border-t sm:border-t-0 border-border/40">
          <button
            onClick={runSnippetTest}
            disabled={isTesting}
            className={`btn-base h-7 px-3 text-[11px] sm:text-[11.5px] rounded-md transition-all ${
              isTesting
                ? 'bg-accent/20 text-accent cursor-wait'
                : testComplete
                ? 'bg-accent/15 text-accent border border-accent/40 hover:bg-accent/25'
                : 'bg-accent text-bg hover:bg-accent-hover font-semibold shadow-[0_0_15px_rgba(57,237,53,0.3)]'
            }`}
          >
            {isTesting ? (
              <span className="flex items-center gap-1.5">
                <RefreshCw size={11} className="animate-spin" />
                Testing...
              </span>
            ) : testComplete ? (
              <span className="flex items-center gap-1.5">
                <CheckCheck size={12} className="text-accent" />
                Re-test snippet
              </span>
            ) : (
              <span className="flex items-center gap-1.5">
                <Play size={11} fill="currentColor" />
                Test snippet live
              </span>
            )}
          </button>

          <button
            aria-label="Copy example"
            onClick={copyCode}
            className="btn-icon shrink-0 text-text-dim hover:text-text h-7 w-7"
          >
            {copied ? <Check size={13} className="text-accent" /> : <Copy size={13} />}
          </button>
        </div>
      </div>

      {/* Code Editor Body */}
      <div id="integration-code" role="tabpanel" aria-label={snippet.label}>
        <div className="px-4 py-2 sm:px-5 sm:pt-3 font-mono text-[10px] sm:text-[10.5px] text-text-dim flex items-center justify-between border-b border-border/40">
          <div className="flex items-center gap-2">
            <span className="h-2 w-2 rounded-full bg-accent/70 shrink-0" />
            <span className="truncate">{snippet.file}</span>
          </div>
          {copied && <span className="text-accent text-[11px] font-sans">Copied!</span>}
        </div>
        <pre className="min-h-[220px] sm:min-h-[260px] overflow-x-auto p-4 sm:p-5 font-mono text-[11.5px] sm:text-[12.5px] leading-[1.85]">
          <SyntaxCode code={snippet.code} language={snippet.language} />
        </pre>
      </div>
      {copyError && <p role="status" className="px-4 pb-3 text-xs text-text-muted">{copyError}</p>}

      {/* Interactive Live Execution Sandbox */}
      <AnimatePresence>
        {(isTesting || testComplete) && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: 'auto' }}
            exit={{ opacity: 0, height: 0 }}
            className="border-t border-border/80 bg-bg-sunken/95 overflow-hidden"
          >
            <div className="flex flex-wrap items-center justify-between border-b border-border/60 px-3 py-2 sm:px-4 bg-bg-elevated/70 gap-2">
              <div className="flex items-center gap-2 min-w-0">
                <span className="flex h-2 w-2 rounded-full bg-accent animate-pulse shrink-0" />
                <span className="font-mono text-[10.5px] sm:text-[11px] font-semibold text-text truncate">
                  SANDBOX RESULTS
                </span>
                <span
                  className={`rounded px-1.5 py-0.2 text-[9px] sm:text-[9.5px] font-mono ${
                    testComplete ? 'bg-accent/15 text-accent font-bold' : 'bg-yellow/15 text-yellow'
                  }`}
                >
                  {testComplete ? '✓ ALL CHECKS PASSED' : 'EXECUTING...'}
                </span>
              </div>

              <div className="flex items-center gap-1">
                {(['logs', 'output', 'telemetry'] as const).map((v) => (
                  <button
                    key={v}
                    onClick={() => setTestView(v)}
                    className={`px-2 py-0.5 rounded text-[9.5px] sm:text-[10px] font-mono transition-colors ${
                      testView === v ? 'bg-text/10 text-text font-bold' : 'text-text-dim hover:text-text'
                    }`}
                  >
                    {v.toUpperCase()}
                  </button>
                ))}
              </div>
            </div>

            <div className="p-3 sm:p-4 font-mono text-[10.5px] sm:text-[11px] leading-relaxed max-h-56 overflow-y-auto">
              {testView === 'logs' && (
                <div className="space-y-1.5">
                  {snippet.testSteps.slice(0, activeStep).map((s, idx) => (
                    <motion.div
                      key={idx}
                      initial={{ opacity: 0, x: -6 }}
                      animate={{ opacity: 1, x: 0 }}
                      className="flex items-start gap-2 text-text-secondary"
                    >
                      <CheckCircle2 size={12} className="text-accent shrink-0 mt-0.5" />
                      <span className="text-text-dim text-[9.5px] w-10 shrink-0">{s.time}</span>
                      <span className="text-text font-mono break-words">{s.text}</span>
                    </motion.div>
                  ))}
                  {isTesting && (
                    <div className="flex items-center gap-2 text-text-dim pt-1">
                      <RefreshCw size={10} className="animate-spin text-accent shrink-0" />
                      <span className="truncate">Negotiating browser commands...</span>
                    </div>
                  )}
                </div>
              )}

              {testView === 'output' && (
                <pre className="text-text-secondary whitespace-pre-wrap leading-relaxed text-[11px] sm:text-[11.5px] break-words">
                  {snippet.mockResult}
                </pre>
              )}

              {testView === 'telemetry' && (
                <div className="grid grid-cols-2 sm:grid-cols-4 gap-2 sm:gap-3">
                  {Object.entries(snippet.telemetry).map(([k, val]) => (
                    <div key={k} className="rounded-lg border border-border/70 bg-bg-card p-2 sm:p-2.5">
                      <div className="text-[9.5px] text-text-dim uppercase tracking-wider">{k}</div>
                      <div className="text-[12px] sm:text-[13px] font-semibold text-accent mt-0.5">{val}</div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

/* ─────────────────────────────────────────────────────────────────────────────
   10x Comparison Matrix Data
───────────────────────────────────────────────────────────────────────────── */
const comparisonRows = [
  {
    feature: 'Architecture & Lock-In',
    raw: 'Single-vendor point solution. Hardcoded to one proprietary API, cloud, and billing tier. Outages or bans break all agents.',
    oya: 'True Control Plane. Route dynamically across Oya Cloud, Browserbase, Steel, Anchor, Browser Use, or private Chrome.',
    advantage: 'Zero vendor lock-in & automatic multi-provider failover.',
  },
  {
    feature: 'Device Identity & Anti-Ban',
    raw: 'Ephemeral dumb sessions or randomized fingerprint spoofing. Anti-bot engines flag bot farms (1 account, 40 devices) or device farms.',
    oya: 'Deterministic Seeded Personas. Byte-identical hardware fingerprints (canvas, WebGL, audio) bound to a persistent cookie jar, proxy, and concurrency cap.',
    advantage: 'Persistent device memory. Returning weeks later looks like the same workstation.',
  },
  {
    feature: 'Authentication & SSO',
    raw: 'Brittle scripted login automation that breaks on Google OAuth, Okta, passkeys, WebAuthn, and Cloudflare managed challenges.',
    oya: 'Sign-In-Once Desktop Pairing. Log in once in real desktop Chrome/Electron with passkeys; cookies securely sync to cloud personas via encrypted pairing codes.',
    advantage: 'Instant authenticated sessions without writing fragile login scripts.',
  },
  {
    feature: 'Challenge Resolution & 2FA',
    raw: 'Fails, errors, or hangs indefinitely when encountering unexpected push approvals, phone notifications, or complex CAPTCHAs.',
    oya: 'Two-Tier Engine + Sub-Second Live Takeover. Native provider solving + CapSolver/2Captcha + automated TOTP/SMS relay + interactive live stream.',
    advantage: '100% task completion. Humans can take over with real clicks and resume the run.',
  },
  {
    feature: 'Fleet Observability & Governance',
    raw: 'Opaque session IDs, black-box execution, static post-mortem logs, zero real-time intervention.',
    oya: 'High-density 1,000+ browser console. Real-time health strip, command-by-command audit stream, Prometheus metrics (/metrics), and hourly spend per key.',
    advantage: 'Full operational control with emergency bulk kill switches (POST /browsers/stop).',
  },
  {
    feature: 'Protocol & Ecosystem Freedom',
    raw: 'Proprietary SDK wrappers requiring bespoke framework integrations and code rewrites.',
    oya: 'Universal Gateway: Native CDP (/connect), MCP streamable HTTP (/mcp/:id), TypeScript SDK, and terminal CLI.',
    advantage: 'Works natively with Playwright, Puppeteer, Stagehand, browser-use, Claude Code, and Cursor.',
  },
  {
    feature: 'Stealth Verification',
    raw: 'Unverifiable marketing claims ("100% undetectable") that fail on modern CreepJS and Bot.Sannysoft.',
    oya: 'Open Benchmark Suite (oya stealth-test --live). Scored live against CreepJS and Sannysoft with exact delta reporting.',
    advantage: 'Honest, measured evasion engineering with attributable deltas.',
  },
];

/* ─────────────────────────────────────────────────────────────────────────────
   Main Landing Page Component
───────────────────────────────────────────────────────────────────────────── */
export default function Home() {
  const [menuOpen, setMenuOpen] = useState(false);

  return (
    <div className="marketing-page min-h-screen bg-bg text-text selection:bg-accent/20">
      {/* Header with Floating Glass Aesthetics */}
      <header className="sticky top-0 z-40 border-b border-border/80 bg-bg/90 backdrop-blur-xl">
        <div className="site-width flex h-[72px] items-center justify-between gap-4">
          <OyaWordmark />
          <nav
            aria-label="Main navigation"
            className="hidden items-center gap-6 text-[13px] text-text-muted lg:flex font-medium"
          >
            <a href="#product" className="hover:text-text transition-colors">Product</a>
            <a href="#architecture" className="hover:text-text transition-colors">Architecture</a>
            <a href="#benchmarks" className="hover:text-text transition-colors">Benchmarks</a>
            <a href="#use-cases" className="hover:text-text transition-colors">Use Cases</a>
            <a href="#comparison" className="hover:text-text transition-colors flex items-center gap-1.5">
              Why Oya <span className="rounded-full bg-accent/15 px-2 py-0.2 font-mono text-[10px] text-accent font-bold">10x</span>
            </a>
            <a href="#video-studio" className="hover:text-text transition-colors flex items-center gap-1">
              <Play size={12} className="text-accent" /> Videos
            </a>
            <a href="#developers" className="hover:text-text transition-colors">Developers</a>
            <a href="#faq" className="hover:text-text transition-colors">FAQ</a>
            <Link href="/docs" className="hover:text-text transition-colors">Docs</Link>
          </nav>
          <div className="flex items-center gap-2.5 sm:gap-3">
            <ThemeToggle />
            <Link
              href="/dashboard"
              className="btn-primary h-9 px-3.5 sm:px-4 text-[12px] font-semibold flex items-center gap-1.5 group"
            >
              Open console
              <ArrowUpRight size={13} className="group-hover:translate-x-0.5 group-hover:-translate-y-0.5 transition-transform" />
            </Link>
            <button
              className="btn-icon lg:hidden"
              aria-label={menuOpen ? 'Close navigation' : 'Open navigation'}
              aria-expanded={menuOpen}
              onClick={() => setMenuOpen(!menuOpen)}
            >
              {menuOpen ? <X size={19} /> : <Menu size={19} />}
            </button>
          </div>
        </div>

        {/* Mobile Navigation Dropdown */}
        {menuOpen && (
          <nav
            aria-label="Mobile navigation"
            className="site-width flex flex-col gap-3.5 border-t border-border/80 py-5 text-sm lg:hidden"
          >
            <a href="#product" onClick={() => setMenuOpen(false)}>Product</a>
            <a href="#architecture" onClick={() => setMenuOpen(false)}>Architecture</a>
            <a href="#benchmarks" onClick={() => setMenuOpen(false)}>Benchmarks</a>
            <a href="#use-cases" onClick={() => setMenuOpen(false)}>Use Cases</a>
            <a href="#comparison" onClick={() => setMenuOpen(false)}>Why Oya (10x Comparison)</a>
            <a href="#video-studio" onClick={() => setMenuOpen(false)}>Video & Motion Showcase</a>
            <a href="#developers" onClick={() => setMenuOpen(false)}>Developers & Code Tester</a>
            <a href="#faq" onClick={() => setMenuOpen(false)}>FAQ</a>
            <Link href="/docs">Documentation</Link>
            <Link href="/dashboard" className="btn-primary h-10 text-[13px] justify-center mt-2 font-semibold">
              Open console <ArrowRight size={14} />
            </Link>
          </nav>
        )}
      </header>

      <main>
        {/* ─── Hero Section ─── */}
        <section className="relative site-width pt-12 pb-12 sm:pt-24 sm:pb-20">
          {/* Subtle Ambient Radial Glowing Aura */}
          <div
            className="pointer-events-none absolute -top-20 left-1/2 -translate-x-1/2 h-[450px] w-full max-w-5xl rounded-full bg-[radial-gradient(ellipse_at_center,rgba(57,237,53,0.12)_0%,rgba(108,180,255,0.04)_45%,transparent_70%)] blur-3xl -z-10"
            aria-hidden="true"
          />

          <div className="grid items-end gap-8 lg:grid-cols-[1.3fr_1fr] lg:gap-16">
            <div>
              {/* Responsive Eyebrow Badge */}
              <div className="eyebrow mb-5 sm:mb-6 inline-flex items-center gap-2 rounded-full border border-border/80 bg-bg-card/80 px-3 py-1.5 text-text-muted backdrop-blur-md max-w-full">
                <span className="h-1.5 w-1.5 rounded-full bg-accent animate-ping shrink-0" />
                <span className="text-text font-semibold truncate">Browser Control Plane</span>
                <span className="text-text-dim">·</span>
                <span className="text-accent font-mono shrink-0">1,000+ FLEET</span>
              </div>

              {/* Exact H1 text for testing and brand positioning */}
              <h1 className="marketing-title font-display text-4xl sm:text-6xl lg:text-7xl font-bold tracking-tight">
                Your agents.<br />
                The whole <span className="text-accent">web.</span>
              </h1>

              <p className="mt-4 sm:mt-5 text-lg sm:text-2xl font-medium text-text-secondary leading-snug">
                Stop building on <span className="text-text-dim line-through">dumb browsers</span>. Orchestrate your fleet.
              </p>
            </div>

            <div className="pb-1 lg:max-w-[420px]">
              <p className="text-[15px] sm:text-[16px] leading-[1.75] text-text-muted">
                Browserbase, Steel, Anchor, and Browser Use run isolated browsers. Oya is the{' '}
                <strong className="text-text font-semibold">Control Plane</strong> sitting above them:
                deterministic personas, zero-rewrite failover, desktop auth pairing, and live sub-second fleet orchestration.
              </p>

              <div className="mt-6 sm:mt-8 flex flex-wrap items-center gap-3.5 sm:gap-4">
                <Link
                  href="/dashboard"
                  className="btn-primary h-11 sm:h-12 px-5 sm:px-6 text-[13.5px] sm:text-[14px] font-semibold flex items-center gap-2 shadow-[0_0_32px_rgba(57,237,53,0.3)] hover:shadow-[0_0_42px_rgba(57,237,53,0.45)] transition-all"
                >
                  Start building
                  <ArrowRight size={15} />
                </Link>
                <a href="#comparison" className="btn-ghost h-11 sm:h-12 px-4 sm:px-5 text-[12.5px] sm:text-[13px] font-medium">
                  The 10x difference
                </a>
                <Link
                  href="/docs"
                  className="inline-flex items-center gap-1 text-[12.5px] sm:text-[13px] font-medium text-text-muted hover:text-accent ml-1"
                >
                  Read the docs <ArrowUpRight size={13} />
                </Link>
              </div>
            </div>
          </div>

          {/* Real-time Telemetry KPI Strip */}
          <div className="mt-10 sm:mt-12 grid grid-cols-2 gap-2.5 sm:grid-cols-4 sm:gap-4">
            {[
              { val: '99.99%', label: 'Fleet availability', note: 'Multi-provider active failover' },
              { val: '< 14ms', label: 'Global gateway routing', note: 'Native CDP & MCP transport' },
              { val: '100%', label: 'Persona persistence', note: 'Byte-identical canvas & WebGL' },
              { val: '6 Runners', label: 'Universal compatibility', note: 'Zero SDK lock-in or code rewrite' },
            ].map((k) => (
              <div
                key={k.label}
                className="rounded-xl border border-border/70 bg-bg-card/60 p-3.5 sm:p-4 backdrop-blur-md hover:border-accent/30 transition-colors"
              >
                <div className="text-[19px] sm:text-[22px] font-bold tracking-tight text-accent font-mono">{k.val}</div>
                <div className="text-[11.5px] sm:text-[12.5px] font-semibold text-text mt-0.5 truncate">{k.label}</div>
                <div className="text-[10px] sm:text-[10.5px] text-text-dim mt-0.5 truncate">{k.note}</div>
              </div>
            ))}
          </div>

          {/* Interactive Console Preview */}
          <div id="product" className="mt-12 scroll-mt-24 sm:mt-16">
            <ProductPreview />
          </div>

          {/* Supported Runners Strip */}
          <div className="mt-6 sm:mt-7 flex flex-wrap items-center justify-between gap-x-6 gap-y-3 text-[11.5px] sm:text-[12px] text-text-muted">
            <span className="eyebrow text-text-dim">One Control Plane. Every Execution Target:</span>
            {['Oya Cloud Sandboxes', 'Browserbase', 'Steel', 'Anchor', 'Browser Use', 'Self-Hosted Chrome'].map((p) => (
              <span key={p} className="flex items-center gap-1.5 font-medium text-text-secondary">
                <CheckCircle2 size={13} className="text-accent shrink-0" />
                {p}
              </span>
            ))}
          </div>
        </section>

        {/* ─── Control Plane Architecture Section ─── */}
        <section id="architecture" className="site-width section-space scroll-mt-20 border-t border-border/80">
          <div className="grid gap-6 lg:grid-cols-[1fr_1.35fr]">
            <div>
              <p className="eyebrow mb-3 sm:mb-4 text-accent">Control Plane Architecture</p>
              <h2 className="marketing-heading max-w-sm">
                The missing layer in AI browser infrastructure.
              </h2>
            </div>
            <p className="max-w-xl text-[15px] sm:text-[17px] leading-[1.75] text-text-muted lg:pt-9">
              Don&apos;t hardcode your agents to a single cloud runner. Oya sits between your agents and whoever executes the browsers, solving identity, authentication, challenge handling, and failover in one place.
            </p>
          </div>

          {/* Visual Architecture Diagram */}
          <div className="mt-10 sm:mt-12 rounded-2xl sm:rounded-3xl border border-border/80 bg-bg-card/70 p-5 sm:p-10 backdrop-blur-md">
            {/* Top Layer: Agents & Frameworks */}
            <div className="text-center">
              <span className="eyebrow text-text-dim">Layer 1: Your Agents & Frameworks</span>
              <div className="mt-3 flex flex-wrap items-center justify-center gap-2 sm:gap-2.5">
                {['Claude Code', 'Cursor', 'Playwright', 'Puppeteer', 'Stagehand', 'browser-use', 'LangChain'].map((item) => (
                  <span
                    key={item}
                    className="rounded-lg border border-border/80 bg-bg-sunken px-2.5 py-1 sm:px-3.5 sm:py-1.5 font-mono text-[11px] sm:text-[12px] font-medium text-text hover:border-accent/40 transition-colors"
                  >
                    {item}
                  </span>
                ))}
              </div>
            </div>

            {/* Connecting Pipe */}
            <div className="my-4 sm:my-5 flex flex-col items-center justify-center">
              <div className="h-5 sm:h-6 w-px bg-accent/60" />
              <span className="rounded-full bg-accent/10 border border-accent/25 px-3 py-1 font-mono text-[9px] sm:text-[10px] text-accent font-semibold tracking-wide text-center">
                Universal Protocols: CDP (/connect) · MCP (/mcp) · TS SDK · CLI · REST
              </span>
              <div className="h-5 sm:h-6 w-px bg-accent/60" />
            </div>

            {/* Middle Layer - OYA CONTROL PLANE */}
            <div className="rounded-xl sm:rounded-2xl border-2 border-accent/40 bg-accent/[0.04] p-4 sm:p-8 relative beam-effect">
              <div className="flex flex-wrap items-center justify-between border-b border-accent/20 pb-3 sm:pb-4 mb-4 sm:mb-6 gap-2">
                <div className="flex items-center gap-2">
                  <OyaLogo size={20} />
                  <span className="text-[14px] sm:text-[16px] font-bold tracking-tight">OYA BROWSER CONTROL PLANE</span>
                </div>
                <span className="eyebrow text-accent font-semibold">Core Orchestration Engine</span>
              </div>

              <div className="grid gap-3 sm:gap-4 sm:grid-cols-2 lg:grid-cols-3">
                <div className="rounded-xl border border-border/80 bg-bg/90 p-4 hover:border-accent/40 transition-colors">
                  <div className="flex items-center gap-2 text-[13px] sm:text-[13.5px] font-semibold mb-1 text-text">
                    <Network size={15} className="text-accent shrink-0" />
                    Unified Router & Failover
                  </div>
                  <p className="text-[11.5px] sm:text-[12px] text-text-muted leading-relaxed">
                    Set provider priorities and capacities. Automatic zero-rewrite failover when a vendor throttles or fails.
                  </p>
                </div>

                <div className="rounded-xl border border-border/80 bg-bg/90 p-4 hover:border-accent/40 transition-colors">
                  <div className="flex items-center gap-2 text-[13px] sm:text-[13.5px] font-semibold mb-1 text-text">
                    <Fingerprint size={15} className="text-accent shrink-0" />
                    Deterministic Personas
                  </div>
                  <p className="text-[11.5px] sm:text-[12px] text-text-muted leading-relaxed">
                    Mathematically seeded byte-identical fingerprints across restarts. Bound to cookie jars and pinned proxies.
                  </p>
                </div>

                <div className="rounded-xl border border-border/80 bg-bg/90 p-4 hover:border-accent/40 transition-colors">
                  <div className="flex items-center gap-2 text-[13px] sm:text-[13.5px] font-semibold mb-1 text-text">
                    <KeyRound size={15} className="text-accent shrink-0" />
                    Sign-In-Once Desktop Pairing
                  </div>
                  <p className="text-[11.5px] sm:text-[12px] text-text-muted leading-relaxed">
                    Sign into sites on real desktop Chrome with passkeys; cookies cryptographically sync to cloud agent personas.
                  </p>
                </div>

                <div className="rounded-xl border border-border/80 bg-bg/90 p-4 hover:border-accent/40 transition-colors">
                  <div className="flex items-center gap-2 text-[13px] sm:text-[13.5px] font-semibold mb-1 text-text">
                    <Sliders size={15} className="text-accent shrink-0" />
                    Two-Tier Challenges
                  </div>
                  <p className="text-[11.5px] sm:text-[12px] text-text-muted leading-relaxed">
                    Native CAPTCHA delegation + solver fallback + automated TOTP/SMS relay + human takeover handoffs.
                  </p>
                </div>

                <div className="rounded-xl border border-border/80 bg-bg/90 p-4 hover:border-accent/40 transition-colors">
                  <div className="flex items-center gap-2 text-[13px] sm:text-[13.5px] font-semibold mb-1 text-text">
                    <Monitor size={15} className="text-accent shrink-0" />
                    Interactive Live Takeover
                  </div>
                  <p className="text-[11.5px] sm:text-[12px] text-text-muted leading-relaxed">
                    Sub-second SSE live stream with real click/keyboard input. Humans step in when 2FA hits; agents resume.
                  </p>
                </div>

                <div className="rounded-xl border border-border/80 bg-bg/90 p-4 hover:border-accent/40 transition-colors">
                  <div className="flex items-center gap-2 text-[13px] sm:text-[13.5px] font-semibold mb-1 text-text">
                    <ShieldCheck size={15} className="text-accent shrink-0" />
                    Fleet Governance & Audit
                  </div>
                  <p className="text-[11.5px] sm:text-[12px] text-text-muted leading-relaxed">
                    1,000+ browser console, hourly spend attribution per tenant key, Prometheus metrics (/metrics), immutable audit log.
                  </p>
                </div>
              </div>
            </div>

            {/* Connecting Pipe */}
            <div className="my-4 sm:my-5 flex flex-col items-center justify-center">
              <div className="h-5 sm:h-6 w-px bg-border" />
              <span className="font-mono text-[9.5px] sm:text-[10px] text-text-dim text-center">
                Dispatches to underlying execution targets
              </span>
              <div className="h-5 sm:h-6 w-px bg-border" />
            </div>

            {/* Bottom Layer: Runners */}
            <div className="text-center">
              <span className="eyebrow text-text-dim">Layer 3: Browser Execution Engines</span>
              <div className="mt-3 grid grid-cols-2 gap-2.5 sm:grid-cols-3 lg:grid-cols-6">
                {[
                  { name: 'Oya Cloud', note: 'Isolated sandboxes' },
                  { name: 'Browserbase', note: 'CDP runner' },
                  { name: 'Steel', note: 'CDP runner' },
                  { name: 'Anchor', note: 'CDP runner' },
                  { name: 'Browser Use', note: 'CDP runner' },
                  { name: 'Private Chrome', note: 'Bare metal / Oya Cloud' },
                ].map((item) => (
                  <div
                    key={item.name}
                    className="rounded-xl border border-border/80 bg-bg-sunken p-2.5 sm:p-3 text-center hover:border-accent/30 transition-colors"
                  >
                    <div className="text-[12px] sm:text-[12.5px] font-semibold text-text">{item.name}</div>
                    <div className="text-[9.5px] sm:text-[10px] text-text-dim mt-0.5">{item.note}</div>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </section>

        {/* ─── Benchmarks Section (Startup Time, Tokens, Evasion) ─── */}
        <BenchmarksSection />

        {/* ─── 10x Comparison Section (Desktop Table + Mobile Cards) ─── */}
        <section id="comparison" className="site-width section-space scroll-mt-20 border-t border-border/80">
          <div className="grid gap-6 lg:grid-cols-[1fr_1.35fr]">
            <div>
              <p className="eyebrow mb-3 sm:mb-4 text-accent">The 10x Difference</p>
              <h2 className="marketing-heading max-w-md">
                Control Plane vs.<br />Single-Vendor Runners.
              </h2>
            </div>
            <p className="max-w-xl text-[15px] sm:text-[17px] leading-[1.75] text-text-muted lg:pt-9">
              Why build on point solutions like Browserbase, Steel, Anchor, or Browser Use when you can orchestrate them all? Here is how Oya gives you a 10x architectural leap.
            </p>
          </div>

          {/* Desktop 3-Column Comparison Table (hidden on small viewports) */}
          <div className="mt-10 sm:mt-12 hidden md:block overflow-hidden rounded-2xl border border-border/80 bg-bg-card shadow-2xl">
            <div className="overflow-x-auto">
              <table className="w-full text-left text-sm">
                <thead>
                  <tr className="border-b border-border bg-bg-sunken/80 text-[11px] font-semibold uppercase tracking-wider text-text-dim">
                    <th className="py-4 px-6 w-[20%]">Dimension</th>
                    <th className="py-4 px-6 w-[35%] text-text-muted">
                      Raw Runners (Browserbase, Steel, Anchor, Browser Use)
                    </th>
                    <th className="py-4 px-6 w-[45%] text-accent bg-accent/[0.04]">
                      Oya Browser Control Plane (10x Better)
                    </th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-border/70 font-sans">
                  {comparisonRows.map((row, i) => (
                    <tr key={i} className="hover:bg-text/[0.02] transition-colors">
                      <td className="py-4 px-6 font-medium text-[13px] text-text align-top">
                        {row.feature}
                      </td>
                      <td className="py-4 px-6 text-[12.5px] leading-relaxed text-text-muted align-top">
                        <div className="flex items-start gap-2">
                          <XCircle size={15} className="text-red shrink-0 mt-0.5" />
                          <span>{row.raw}</span>
                        </div>
                      </td>
                      <td className="py-4 px-6 text-[12.5px] leading-relaxed text-text align-top bg-accent/[0.02]">
                        <div className="flex items-start gap-2">
                          <CheckCircle2 size={15} className="text-accent shrink-0 mt-0.5" />
                          <div>
                            <span className="font-medium text-text">{row.oya}</span>
                            <span className="mt-1 block text-[11.5px] text-accent font-mono font-medium">
                              → {row.advantage}
                            </span>
                          </div>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>

          {/* Mobile Comparison Cards (Clean, Readable & Beautiful on Phones) */}
          <div className="mt-8 md:hidden space-y-4">
            {comparisonRows.map((row, i) => (
              <div
                key={i}
                className="rounded-2xl border border-border/80 bg-bg-card p-4 sm:p-5 shadow-lg space-y-3"
              >
                <div className="flex items-center justify-between border-b border-border/50 pb-2">
                  <span className="text-[13.5px] font-bold text-text">{row.feature}</span>
                  <span className="rounded-full bg-accent/15 px-2 py-0.2 font-mono text-[9px] text-accent font-bold">
                    10x
                  </span>
                </div>

                {/* Oya Advantage Card */}
                <div className="rounded-xl border border-accent/40 bg-accent/[0.06] p-3 space-y-1.5">
                  <div className="flex items-center gap-1.5 text-[11px] font-semibold text-accent uppercase tracking-wider">
                    <CheckCircle2 size={13} className="text-accent shrink-0" />
                    Oya Control Plane
                  </div>
                  <p className="text-[12px] text-text leading-relaxed font-medium">{row.oya}</p>
                  <p className="text-[11px] text-accent font-mono font-medium pt-1 border-t border-accent/20">
                    → {row.advantage}
                  </p>
                </div>

                {/* Raw Runners Limitation Card */}
                <div className="rounded-xl border border-border/60 bg-bg-sunken p-3 space-y-1 text-[11.5px] text-text-muted">
                  <div className="flex items-center gap-1.5 text-[10.5px] font-medium text-red uppercase tracking-wider">
                    <XCircle size={12} className="text-red shrink-0" />
                    Raw Runners (Single Vendor)
                  </div>
                  <p className="leading-relaxed">{row.raw}</p>
                </div>
              </div>
            ))}
          </div>
        </section>

        {/* ─── Production Use Cases ─── */}
        <UseCasesSection />

        {/* ─── Video & Motion Showcase Section ("and animation videos") ─── */}
        <section id="video-studio" className="site-width section-space border-t border-border/80 scroll-mt-16">
          <div className="flex flex-wrap items-end justify-between gap-4 mb-6 sm:mb-8">
            <div>
              <p className="eyebrow mb-2 sm:mb-3 text-accent flex items-center gap-2">
                <Play size={12} className="text-accent" />
                Video & Motion Showcase
              </p>
              <h2 className="marketing-heading">See the fleet in motion.</h2>
              <p className="text-[14px] sm:text-[15px] text-text-muted mt-1.5 sm:mt-2 max-w-xl">
                Experience real-world walkthroughs and interactive video simulations of autonomous CAPTCHA handling, sub-second live takeover, and provider failover.
              </p>
            </div>
            <span className="flex items-center gap-2 rounded-full border border-border/80 bg-bg-card px-3 py-1 font-mono text-[10.5px] sm:text-[11px] text-text-dim">
              <span className="h-1.5 w-1.5 rounded-full bg-accent animate-pulse" />
              Live video & motion demos
            </span>
          </div>

          <VideoMotionStudio />
        </section>

        {/* ─── Feature Pillars ─── */}
        <section className="site-width section-space border-t border-border/80">
          <div className="grid gap-6 lg:grid-cols-[1fr_1.35fr]">
            <div>
              <p className="eyebrow mb-3 sm:mb-4 text-accent">Pillars of the Control Plane</p>
              <h2 className="marketing-heading max-w-sm">
                Engineered for fleets that cannot fail.
              </h2>
            </div>
            <p className="max-w-xl text-[15px] sm:text-[17px] leading-[1.75] text-text-muted lg:pt-9">
              Everything built into Oya exists because real production agent fleets break on bot detectors, auth walls, and single-provider outages.
            </p>
          </div>

          <div className="mt-10 sm:mt-12 grid gap-6 sm:gap-8 md:grid-cols-3 md:gap-10">
            <div className="rounded-2xl border border-border/80 bg-bg-card/80 p-5 sm:p-7 hover:border-accent/40 transition-colors shadow-lg">
              <div className="mb-5 sm:mb-6 flex items-center justify-between">
                <Fingerprint size={24} strokeWidth={1.5} className="text-accent" />
                <span className="font-mono text-[10px] text-text-dim">PILLAR 01</span>
              </div>
              <h3 className="text-[16px] sm:text-[18px] font-semibold tracking-tight text-text">
                Personas: Device identity, not ephemeral sessions.
              </h3>
              <p className="mt-2.5 sm:mt-3 text-[13px] sm:text-[14px] leading-relaxed text-text-muted">
                Anti-bot engines look for two telltales: one account seen from 50 device fingerprints (bot farm), or one device seen on 1,000 accounts (device farm).
              </p>
              <p className="mt-2 text-[13px] sm:text-[14px] leading-relaxed text-text-muted">
                Oya personas bind a mathematically seeded, byte-identical hardware profile to a cookie jar, proxy, and concurrency cap. When your agent returns next week, target sites see the exact same MacBook Pro or Windows desktop returning.
              </p>
            </div>

            <div className="rounded-2xl border border-border/80 bg-bg-card/80 p-5 sm:p-7 hover:border-accent/40 transition-colors shadow-lg">
              <div className="mb-5 sm:mb-6 flex items-center justify-between">
                <RefreshCw size={24} strokeWidth={1.5} className="text-accent" />
                <span className="font-mono text-[10px] text-text-dim">PILLAR 02</span>
              </div>
              <h3 className="text-[16px] sm:text-[18px] font-semibold tracking-tight text-text">
                Multi-Provider Routing: Zero-rewrite failover.
              </h3>
              <p className="mt-2.5 sm:mt-3 text-[13px] sm:text-[14px] leading-relaxed text-text-muted">
                Never lock your agent to a single browser vendor. If Browserbase has a regional outage or Steel throttles your quota, your agent should not crash.
              </p>
              <p className="mt-2 text-[13px] sm:text-[14px] leading-relaxed text-text-muted">
                Oya routes traffic dynamically across providers based on configured priority and session capacity. A failing runner triggers instant failover to the next healthy runner in milliseconds.
              </p>
            </div>

            <div className="rounded-2xl border border-border/80 bg-bg-card/80 p-5 sm:p-7 hover:border-accent/40 transition-colors shadow-lg">
              <div className="mb-5 sm:mb-6 flex items-center justify-between">
                <KeyRound size={24} strokeWidth={1.5} className="text-accent" />
                <span className="font-mono text-[10px] text-text-dim">PILLAR 03</span>
              </div>
              <h3 className="text-[16px] sm:text-[18px] font-semibold tracking-tight text-text">
                Sign In Once: Desktop pairing with passkeys & SSO.
              </h3>
              <p className="mt-2.5 sm:mt-3 text-[13px] sm:text-[14px] leading-relaxed text-text-muted">
                Automating logins to Google, Okta, or Salesforce with headless scripts fails 90% of the time and triggers fraud alerts.
              </p>
              <p className="mt-2 text-[13px] sm:text-[14px] leading-relaxed text-text-muted">
                With Oya&apos;s desktop app, a human signs in ONCE using real passkeys, WebAuthn, or phone push MFA. Oya extracts and cryptographically synchronizes the session state to the cloud persona. Your agent wakes up already signed in.
              </p>
            </div>
          </div>
        </section>

        {/* ─── Universal Integration & Interactive Code Snippet Tester ("and test the code snippets") ─── */}
        <section id="developers" className="site-width section-space scroll-mt-16 border-t border-border/80">
          <div className="grid items-start gap-8 sm:gap-10 lg:grid-cols-[0.8fr_1.2fr] lg:gap-16">
            <div>
              <p className="eyebrow mb-3 sm:mb-4 text-accent">Universal Integration</p>
              <h2 className="marketing-heading">
                Zero SDK Lock-In.<br />Speak your own stack.
              </h2>
              <p className="mt-4 sm:mt-5 max-w-sm text-[14px] sm:text-[15px] leading-relaxed text-text-muted">
                Whether you use standard Playwright over CDP, Claude Code via Model Context Protocol, the TypeScript SDK, or the terminal CLI — the Oya Control Plane handles routing, personas, and challenge solving identically.
              </p>
              <div className="mt-6 sm:mt-8 flex flex-col gap-3">
                <Link
                  href="/docs"
                  className="inline-flex items-center gap-1.5 text-[12.5px] sm:text-[13px] font-medium text-accent hover:underline"
                >
                  Explore full documentation & API reference <ArrowRight size={14} />
                </Link>
                <div className="mt-3 sm:mt-4 flex items-center gap-3 border-t border-border/80 pt-4 sm:pt-5 text-[11.5px] sm:text-[12px] text-text-dim font-mono">
                  <Terminal size={14} className="text-accent shrink-0" />
                  <code>npm install @oya-ai/browser</code>
                </div>
              </div>
            </div>

            {/* Interactive Code Snippet Tester Box */}
            <CodeSnippetTester />
          </div>
        </section>

        {/* ─── Frequently Asked Questions (FAQ) Section ─── */}
        <FaqSection />

        {/* ─── Desktop App & Self-Hosting ─── */}
        <section className="site-width section-space grid gap-8 sm:gap-10 border-t border-border/80 md:grid-cols-2 md:gap-16">
          <div id="download" className="scroll-mt-24 rounded-2xl sm:rounded-3xl border border-border/80 bg-bg-card p-6 sm:p-8 shadow-xl">
            <Monitor size={24} strokeWidth={1.5} className="mb-4 sm:mb-5 text-accent" />
            <h2 className="text-[20px] sm:text-[24px] font-semibold tracking-tight text-text">
              Sign In Once on Desktop.
            </h2>
            <p className="mt-2.5 sm:mt-3 text-[13.5px] sm:text-[14px] leading-relaxed text-text-muted">
              Download the native Oya desktop browser. Log into your company’s target services with real passkeys and WebAuthn. Oya synchronizes your active session directly to your agent personas.
            </p>
            <div className="mt-5 sm:mt-6 flex flex-wrap items-center gap-3 sm:gap-4">
              <a
                className="btn-primary h-10 px-4 text-[12px] font-semibold"
                href="/downloads/Oya.Browser-1.0.83-universal.dmg"
              >
                Download macOS (.dmg) <ArrowUpRight size={14} />
              </a>
              <a
                className="btn-ghost h-10 px-4 text-[12px]"
                href="/downloads/Oya.Browser-1.0.83-x64.exe"
              >
                Windows (.exe)
              </a>
              <a
                className="btn-ghost h-10 px-4 text-[12px]"
                href="/downloads/Oya.Browser-1.0.83-x64.AppImage"
              >
                Linux (.AppImage)
              </a>
            </div>
            <p className="mt-3 text-[10.5px] sm:text-[11px] text-text-dim font-mono">
              Universal binary (Apple Silicon + Intel) · Sandboxed Electron
            </p>
          </div>

          <div id="self-host" className="scroll-mt-24 rounded-2xl sm:rounded-3xl border border-border/80 bg-bg-card p-6 sm:p-8 shadow-xl">
            <Layers size={24} strokeWidth={1.5} className="mb-4 sm:mb-5 text-accent" />
            <h2 className="text-[20px] sm:text-[24px] font-semibold tracking-tight text-text">
              Self-Hostable & Air-Gapped.
            </h2>
            <p className="mt-2.5 sm:mt-3 text-[13.5px] sm:text-[14px] leading-relaxed text-text-muted">
              Run the full control plane on your own Kubernetes cluster, Docker host, or cloud VMs. Complete data sovereignty — credentials and session tokens are encrypted with your own master key.
            </p>
            <div className="mt-5 sm:mt-6 flex flex-wrap items-center gap-3 sm:gap-4">
              <Link href="/docs#self-host" className="btn-ghost h-10 px-4 text-[12px]">
                Deploy Docker Compose <ArrowUpRight size={14} />
              </Link>
              <Link href="/docs#settings" className="btn-ghost h-10 px-4 text-[12px]">
                Config reference
              </Link>
            </div>
            <p className="mt-3 text-[10.5px] sm:text-[11px] text-text-dim font-mono">
              <code>docker compose up</code> · Single container deployment on :3100
            </p>
          </div>
        </section>

        {/* ─── Bottom CTA Banner ─── */}
        <section className="border-y border-border/80 bg-bg-card/70 backdrop-blur-md">
          <div className="site-width flex flex-col items-start justify-between gap-6 py-12 sm:py-16 sm:flex-row sm:items-center">
            <div>
              <p className="eyebrow mb-2 sm:mb-3 text-accent font-semibold">Ready for production</p>
              <h2 className="marketing-heading">
                Take control of your browser fleet.
              </h2>
              <p className="text-[14px] sm:text-[15px] text-text-muted mt-2 max-w-md">
                Deploy your first deterministic persona, connect your agents, and route across providers in under 3 minutes.
              </p>
            </div>
            <div className="flex flex-wrap items-center gap-3 sm:gap-4">
              <Link href="/dashboard" className="btn-primary h-11 sm:h-12 shrink-0 px-5 sm:px-6 text-[13.5px] sm:text-[14px] font-semibold flex items-center gap-2">
                Open console <ArrowRight size={15} />
              </Link>
              <Link href="/docs#quickstart" className="btn-ghost h-11 sm:h-12 shrink-0 px-4 sm:px-5 text-[13.5px] sm:text-[14px]">
                Quickstart Guide
              </Link>
            </div>
          </div>
        </section>
      </main>

      {/* ─── Footer ─── */}
      <footer className="site-width flex flex-wrap items-center justify-between gap-5 py-8 sm:py-10 border-t border-border/40">
        <div className="flex flex-wrap items-center gap-3">
          <OyaWordmark />
          <span className="text-[11.5px] sm:text-[12px] text-text-dim border-l border-border/80 pl-3">
            The Browser Control Plane for AI Employees
          </span>
        </div>
        <div className="flex flex-wrap items-center gap-4 sm:gap-6 text-[12px] text-text-muted">
          <Link href="/docs" className="hover:text-text transition-colors">Documentation</Link>
          <a href="#benchmarks" className="hover:text-text transition-colors">Benchmarks</a>
          <a href="#use-cases" className="hover:text-text transition-colors">Use Cases</a>
          <a href="#faq" className="hover:text-text transition-colors">FAQ</a>
          <a href="#comparison" className="hover:text-text transition-colors">10x Comparison</a>
          <Link href="/dashboard" className="hover:text-text transition-colors">Console</Link>
          <a
            href="https://github.com"
            target="_blank"
            rel="noreferrer"
            className="hover:text-text transition-colors flex items-center gap-1"
          >
            GitHub <ExternalLink size={11} />
          </a>
        </div>
      </footer>
    </div>
  );
}
