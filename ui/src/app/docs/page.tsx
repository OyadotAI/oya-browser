'use client';
import SyntaxCode from '@/components/ui/syntax-code';
import { OyaWordmark } from '@/components/oya-logo';
import ThemeToggle from '@/components/theme-toggle';
import Dialog from '@/components/ui/dialog';

import { useState, useEffect, useRef, useCallback, useMemo, createContext, useContext } from 'react';
import Link from 'next/link';
import {
  Search, Copy, Check, ArrowUpRight,
  X,
  ArrowLeft,
  ExternalLink,
  ChevronRight,
  BookOpen,
  Terminal,
  Code,
  Globe,
  Zap,
  Menu,
  Shield,
  Users,
  Layers,
} from 'lucide-react';

/* ------------------------------------------------------------------ */
/*  Types                                                              */
/* ------------------------------------------------------------------ */

interface SearchIndexEntry {
  text: string;
  id: string;
  tag: string;
}

interface SearchHit {
  id: string;
  snippet: string;
  tag: string;
}

/* ------------------------------------------------------------------ */
/*  Docs Page                                                          */
/* ------------------------------------------------------------------ */

const DocsNav = createContext({ active: 'control-plane', navigate: (() => {}) as (id: string) => void });

export default function DocsPage() {
  const [activeSection, setActiveSection] = useState('control-plane');
  useEffect(() => {
    let frame = 0;
    const update = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const headings = [...document.querySelectorAll<HTMLElement>('main h2[id], main h3[id]')];
        const current = headings.filter(el => el.getBoundingClientRect().top <= 140).at(-1);
        if (current) setActiveSection(current.id);
      });
    };
    window.addEventListener('scroll', update, { passive: true });
    return () => { window.removeEventListener('scroll', update); cancelAnimationFrame(frame); };
  }, []);
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<SearchHit[]>([]);
  const [showResults, setShowResults] = useState(false);
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  /* ---- Search index ---- */
  const searchIndex = useMemo<SearchIndexEntry[]>(() => {
    const items: [string, string, string][] = [
      ['Documentation', '', 'H1'],
      ['The browser control plane sitting between AI agents and execution runners.', '', 'P'],
      ['Control Plane Architecture', 'control-plane', 'H2'],
      ['The control plane model vs raw browser runners like Browserbase, Steel, Anchor, Browser Use.', 'control-plane', 'P'],
      ['Why Oya — 10x Comparison', 'comparison', 'H2'],
      ['Detailed comparison vs single-vendor runners: Browserbase, Steel, Anchor, Browser Use.', 'comparison', 'P'],
      ['Multi-Provider Routing & Failover', 'routing-failover', 'H2'],
      ['Configuring provider priorities, session capacities, and zero-rewrite automatic failover.', 'routing-failover', 'P'],
      ['Stealth Benchmarks & Verification', 'stealth-benchmarks', 'H2'],
      ['Benchmarking anti-detection evasion live against CreepJS and Bot.Sannysoft.', 'stealth-benchmarks', 'P'],
      ['Quickstart', 'quickstart', 'H2'],
      ['Open the API key menu in the dashboard to create an API key', 'quickstart', 'LI'],
      ['Download Oya Browser for your OS', 'quickstart', 'LI'],
      ['Open the app, enter wss://browser.getoya.ai/ws as server URL and paste your API key', 'quickstart', 'LI'],
      ['Your browser appears in the dashboard — you can now send commands or connect AI tools', 'quickstart', 'LI'],
      ['Create API Key', 'create-key', 'H2'],
      ['Open the API key menu in the dashboard to create or choose your key.', 'create-key', 'P'],
      ['Your key is scoped — you only see browsers connected with your key.', 'create-key', 'P'],
      ['Save your key somewhere safe. If you lose it, you\'ll need to generate a new one.', 'create-key', 'P'],
      ['Download Browser', 'download', 'H2'],
      ['macOS (Intel + Apple Silicon)', 'download', 'TD'],
      ['Linux (arm64)', 'download', 'TD'],
      ['Running multiple instances', 'download', 'H3'],
      ['Connect', 'connect', 'H2'],
      ['Open Oya Browser. The setup screen appears on first launch.', 'connect', 'P'],
      ['MCP Setup', 'mcp-setup', 'H2'],
      ['Oya Browser exposes each connected browser as an MCP server', 'mcp-setup', 'P'],
      ['Cursor', 'cursor', 'H3'],
      ['Claude Desktop', 'claude-desktop', 'H3'],
      ['Claude Code', 'claude-code', 'H3'],
      ['analyze_page', 'analyze_page', 'H2'],
      ['Analyzes the current page. Returns the full page as structured markdown with every interactive element numbered.', 'analyze_page', 'P'],
      ['navigate', 'navigate', 'H2'],
      ['Navigate the browser to a URL.', 'navigate', 'P'],
      ['click', 'click', 'H2'],
      ['Click an interactive element by its ID number from analyze_page.', 'click', 'P'],
      ['type', 'type', 'H2'],
      ['Type text into an input element.', 'type', 'P'],
      ['press_key', 'press_key', 'H2'],
      ['Press a keyboard key.', 'press_key', 'P'],
      ['screenshot', 'screenshot', 'H2'],
      ['Capture the visible tab as a base64 PNG image.', 'screenshot', 'P'],
      ['scroll', 'scroll', 'H2'],
      ['Scroll the page up or down.', 'scroll', 'P'],
      ['Tab Management', 'tabs', 'H2'],
      ['list_tabs', 'tabs', 'H3'],
      ['open_tab', 'tabs', 'H3'],
      ['switch_tab', 'tabs', 'H3'],
      ['close_tab', 'tabs', 'H3'],
      ['wait', 'wait', 'H2'],
      ['Wait for an element matching a CSS selector to appear on the page.', 'wait', 'P'],
      ['Anonymity', 'anonymity', 'H2'],
      ['Manage browser profiles with unique fingerprints, proxy routing, and isolated cookie stores', 'anonymity', 'P'],
      ['Fingerprint Spoofing', 'fingerprint', 'H3'],
      ['Canvas, WebGL, AudioContext, font, and ClientRects noise per profile', 'fingerprint', 'P'],
      ['Proxy Support', 'proxy-support', 'H3'],
      ['SOCKS5 and HTTP proxy per profile with DNS leak prevention', 'proxy-support', 'P'],
      ['Anti-Detection Stealth', 'stealth', 'H3'],
      ['Removes Electron markers, fixes window.chrome, navigator.webdriver, plugins', 'stealth', 'P'],
      ['list_profiles', 'list_profiles', 'H2'],
      ['List all available anonymity profiles with their platform, timezone, and proxy status', 'list_profiles', 'P'],
      ['create_profile', 'create_profile', 'H2'],
      ['Create a new anonymity profile with randomized fingerprint and optional proxy', 'create_profile', 'P'],
      ['set_profile', 'set_profile', 'H2'],
      ['Switch to a different profile — reloads all tabs with new fingerprint, proxy, and cookies', 'set_profile', 'P'],
      ['Dashboard', 'dashboard-overview', 'H2'],
      ['The dashboard at /dashboard is the control panel.', 'dashboard-overview', 'P'],
      ['Chat', 'chat', 'H3'],
      ['Natural language browser control with formatted responses and tool badges', 'chat', 'P'],
      ['Dev Panel', 'chat', 'H3'],
      ['Chat, Actions, Network, and Source tabs in the desktop app dev panel', 'chat', 'P'],
      ['Live View', 'live-view', 'H3'],
      ['Settings', 'settings', 'H3'],
      ['REST API', 'rest-api', 'H2'],
      ['All endpoints require Authorization: Bearer YOUR_API_KEY header', 'rest-api', 'P'],
      ['Command API Reference', 'command-api', 'H3'],
      ['Navigation Actions', 'command-api', 'H3'],
      ['Page Analysis Actions', 'command-api', 'H3'],
      ['Interaction Actions', 'command-api', 'H3'],
      ['WebSocket Protocol', 'websocket', 'H2'],
      ['Browsers connect via WebSocket at wss://browser.getoya.ai/ws.', 'websocket', 'P'],
    ];
    return items
      .filter(([text]) => text.length >= 3)
      .map(([text, id, tag]) => ({ text, id, tag }));
  }, []);

  /* ---- Search logic ---- */
  const doSearch = useCallback(
    (q: string) => {
      const query = q.trim().toLowerCase();
      if (!query || query.length < 2) {
        setSearchResults([]);
        setShowResults(false);
        return;
      }
      const hits: SearchHit[] = [];
      const seen = new Set<string>();
      for (const entry of searchIndex) {
        const lower = entry.text.toLowerCase();
        const pos = lower.indexOf(query);
        if (pos === -1) continue;
        const key = entry.id + '|' + entry.text.slice(0, 40);
        if (seen.has(key)) continue;
        seen.add(key);
        const start = Math.max(0, pos - 30);
        const end = Math.min(entry.text.length, pos + query.length + 30);
        const snippet =
          (start > 0 ? '...' : '') +
          entry.text.slice(start, end) +
          (end < entry.text.length ? '...' : '');
        hits.push({ id: entry.id, snippet, tag: entry.tag });
        if (hits.length >= 12) break;
      }
      setSearchResults(hits);
      setShowResults(true);
    },
    [searchIndex],
  );

  const handleSearchInput = useCallback(
    (value: string) => {
      setSearchQuery(value);
      if (debounceRef.current) clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(() => doSearch(value), 150);
    },
    [doSearch],
  );

  const handleSearchKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Escape') {
        setSearchQuery('');
        doSearch('');
        searchInputRef.current?.blur();
      }
    },
    [doSearch],
  );

  const handleResultClick = useCallback(
    (id: string) => {
      setSearchQuery('');
      setShowResults(false);
      setSearchResults([]);
      setMobileMenuOpen(false);
      const el = document.getElementById(id);
      if (el) {
        el.scrollIntoView({ behavior: 'smooth', block: 'start' });
      }
    },
    [],
  );

  /* ---- "/" shortcut ---- */
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === '/' && (document.activeElement as HTMLElement)?.tagName !== 'INPUT') {
        e.preventDefault();
        [...document.querySelectorAll<HTMLInputElement>('[aria-label="Search documentation"]')].find(el => el.getClientRects().length)?.focus();
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, []);

  /* ---- Sidebar nav click (close mobile menu + scroll) ---- */
  const navClick = useCallback((id: string) => {
    setMobileMenuOpen(false);
    setActiveSection(id);
    window.history.replaceState(null, '', '#' + id);
    setTimeout(() => {
      const el = document.getElementById(id);
      if (el) el.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }, 100);
  }, []);

  /* ---------------------------------------------------------------- */
  /*  Sidebar contents (shared between desktop + mobile)               */
  /* ---------------------------------------------------------------- */
  const sidebarContent = (
    <>
      <div className="mb-6 flex items-center justify-between"><span className="eyebrow text-text-dim">Documentation</span><span className="rounded border border-border px-1.5 py-0.5 font-mono text-[10px] text-text-dim">v1</span></div>

      {/* Search */}
      <div className="relative mb-5">
        <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-text-dim pointer-events-none" />
        <input
          ref={searchInputRef}
          type="text"
          placeholder="Search docs…"
          aria-label="Search documentation"
          className="w-full bg-bg-sunken border border-border rounded-lg py-2.5 pl-8 pr-8 text-text text-sm outline-none placeholder:text-text-dim focus:border-indigo transition-colors"
          value={searchQuery}
          onChange={(e) => handleSearchInput(e.target.value)}
          onKeyDown={handleSearchKeyDown}
        />
        {searchQuery && (
          <button
            aria-label="Clear search" onClick={() => { setSearchQuery(''); doSearch(''); }}
            className="absolute right-2 top-1/2 -translate-y-1/2 text-text-dim hover:text-text transition-colors"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        )}
      </div>

      {/* Search results dropdown */}
      {showResults && (
        <div className="mb-4 bg-bg-card border border-border rounded-xl overflow-hidden shadow-lg shadow-black/30">
          {searchResults.length === 0 ? (
            <div className="text-xs text-text-dim px-3 py-2">No results</div>
          ) : (
            searchResults.map((hit, i) => (
              <button
                key={i}
                onClick={() => handleResultClick(hit.id)}
                className="w-full text-left block text-xs px-3 py-2 text-text-muted hover:bg-bg-elevated hover:text-text transition-colors cursor-pointer border-b border-border last:border-b-0"
              >
                <span className="text-text-dim text-[11px] leading-snug">{hit.snippet}</span>
              </button>
            ))
          )}
        </div>
      )}

      {/* Nav sections */}
      <NavSection icon={<Layers className="w-3 h-3" />} label="Control Plane">
        <NavLink id="control-plane">Architecture</NavLink>
        <NavLink id="comparison">Why Oya (10x Leap)</NavLink>
        <NavLink id="routing-failover">Routing & Failover</NavLink>
        <NavLink id="stealth-benchmarks">Stealth Benchmarks</NavLink>
      </NavSection>

      <NavSection icon={<Zap className="w-3 h-3" />} label="Getting Started">
        <NavLink id="quickstart">Quickstart</NavLink>
        <NavLink id="sdk">SDK</NavLink>
        <NavLink id="cli">CLI</NavLink>
        <NavLink id="create-key">Create API Key</NavLink>
        <NavLink id="download">Desktop Sign-in</NavLink>
        <NavLink id="connect">Connect</NavLink>
      </NavSection>

      <NavSection icon={<Users className="w-3 h-3" />} label="Identity">
        <NavLink id="personas">Personas</NavLink>
        <NavLink id="rotation">Rotation</NavLink>
        <NavLink id="captcha">CAPTCHA</NavLink>
        <NavLink id="mfa">MFA</NavLink>
      </NavSection>

      <NavSection icon={<BookOpen className="w-3 h-3" />} label="AI Integration">
        <NavLink id="mcp-setup">MCP Setup</NavLink>
        <NavLink id="cursor">Cursor</NavLink>
        <NavLink id="claude-desktop">Claude Desktop</NavLink>
        <NavLink id="claude-code">Claude Code</NavLink>
      </NavSection>

      <NavSection icon={<Terminal className="w-3 h-3" />} label="MCP Tools">
        <NavLink id="analyze_page">analyze_page</NavLink>
        <NavLink id="navigate">navigate</NavLink>
        <NavLink id="click">click</NavLink>
        <NavLink id="type">type</NavLink>
        <NavLink id="press_key">press_key</NavLink>
        <NavLink id="screenshot">screenshot</NavLink>
        <NavLink id="scroll">scroll</NavLink>
        <NavLink id="tabs">Tab management</NavLink>
        <NavLink id="wait">wait</NavLink>
      </NavSection>

      <NavSection icon={<Shield className="w-3 h-3" />} label="Anonymity">
        <NavLink id="anonymity">Overview</NavLink>
        <NavLink id="fingerprint">Fingerprint Spoofing</NavLink>
        <NavLink id="proxy-support">Proxy Support</NavLink>
        <NavLink id="stealth">Anti-Detection</NavLink>
        <NavLink id="list_profiles">list_profiles</NavLink>
        <NavLink id="create_profile">create_profile</NavLink>
        <NavLink id="set_profile">set_profile</NavLink>
      </NavSection>

      <NavSection icon={<Globe className="w-3 h-3" />} label="Dashboard">
        <NavLink id="dashboard-overview">Overview</NavLink>
        <NavLink id="onboarding">Onboarding</NavLink>
        <NavLink id="live-view">Live View</NavLink>
        <NavLink id="settings">Settings</NavLink>
      </NavSection>

      <NavSection icon={<Code className="w-3 h-3" />} label="API">
        <NavLink id="rest-api">REST API</NavLink>
        <NavLink id="command-api">Command Reference</NavLink>
        <a
          href="/swagger"
          className="flex items-center gap-1 text-[13px] text-text-muted hover:text-text py-1 transition-colors"
        >
          Swagger UI <ExternalLink className="w-3 h-3" />
        </a>
        <NavLink id="websocket">WebSocket Protocol</NavLink>
      </NavSection>

      {/* Bottom links */}
      <div className="mt-auto pt-5 border-t border-border space-y-1.5">
        <Link
          href="/"
          className="flex items-center gap-1.5 text-xs text-text-dim hover:text-text transition-colors"
        >
          <ArrowLeft className="w-3 h-3" /> Back to home
        </Link>
        <Link
          href="/dashboard"
          className="flex items-center gap-1.5 text-xs text-text-dim hover:text-text transition-colors"
        >
          <ChevronRight className="w-3 h-3" /> Open Dashboard
        </Link>
        <a
          href="/llms.txt"
          className="block text-[11px] text-text-dim/60 hover:text-text-dim transition-colors mt-2"
        >
          llms.txt (plain text for AI)
        </a>
        <div className="text-[10px] text-text-dim/40 mt-2">
          Press{' '}
          <kbd className="bg-bg-elevated border border-border rounded px-1 py-0.5 font-mono text-[10px]">
            /
          </kbd>{' '}
          to search
        </div>
      </div>
    </>
  );

  /* ---------------------------------------------------------------- */
  /*  Render                                                           */
  /* ---------------------------------------------------------------- */
  return (
    <DocsNav.Provider value={{ active: activeSection, navigate: navClick }}>
    <div className="docs-page min-h-screen bg-bg">
      <header className="fixed inset-x-0 top-0 z-40 flex h-[72px] items-center justify-between gap-4 border-b border-border bg-bg/95 px-4 backdrop-blur-md lg:px-8">
        <div className="flex items-center gap-5"><OyaWordmark /><span className="hidden border-l border-border pl-5 text-[13px] text-text-muted sm:block">Documentation</span></div>
        <div className="flex items-center gap-3"><ThemeToggle /><Link href="/dashboard" className="btn-ghost h-9 text-[12px]">Open console <ArrowUpRight size={14}/></Link><button className="btn-icon lg:hidden" onClick={() => setMobileMenuOpen(true)} aria-label="Open documentation menu"><Menu size={19}/></button></div>
      </header>
      {mobileMenuOpen && <Dialog open onClose={() => setMobileMenuOpen(false)} title="Documentation" size="sm"><nav aria-label="Documentation sections">{sidebarContent}</nav></Dialog>}
      <aside aria-label="Documentation sections" className="hidden lg:block fixed bottom-0 left-0 top-[72px] z-30 w-[260px] border-r border-border bg-bg-card/25 flex-col px-6 py-7 overflow-y-auto">{sidebarContent}</aside>

      {/* ---- Main content ---- */}
      <main className="min-w-0 lg:ml-[260px] px-5 pt-28 pb-24 sm:px-10 lg:px-14 lg:pt-32">
        <div className="max-w-[800px] mx-auto">

          <p className="eyebrow mb-5 text-accent">Browser Infrastructure</p>
          <h1 className="text-[40px] sm:text-[52px] leading-[1.1] font-medium tracking-[-.05em] text-text mb-5">Your browser, the Control Plane.</h1>
          <p className="max-w-xl text-text-muted mb-8 text-[16px] leading-7">Orchestrate Oya Cloud, Browserbase, Steel, Anchor, Browser Use, and private Chrome behind one API. Deterministic personas, zero-rewrite failover, and sub-second live takeover.</p>
          <div className="mb-12 grid gap-3 sm:grid-cols-2 lg:grid-cols-4">{[
            ['control-plane','Control Plane','Architecture & model'],
            ['comparison','Why Oya (10x)','Comparison vs raw runners'],
            ['routing-failover','Routing & Failover','Zero-rewrite resilience'],
            ['sdk','TypeScript SDK','Build from code'],
          ].map(([id,title,description])=><a key={id} href={'#'+id} onClick={e=>{e.preventDefault();navClick(id);}} className="group rounded-xl border border-border bg-bg-card/40 p-4 hover:border-accent/40"><span className="flex items-center justify-between text-[13px] font-medium">{title}<ArrowUpRight size={13} className="text-text-dim group-hover:text-accent"/></span><span className="mt-2 block text-[12px] text-text-dim">{description}</span></a>)}</div>

          {/* ============ CONTROL PLANE ARCHITECTURE ============ */}
          <SectionHeading id="control-plane" first>Control Plane Architecture</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">
            Raw browser runners like <strong>Browserbase</strong>, <strong>Steel</strong>, <strong>Anchor</strong>, and <strong>Browser Use</strong> are execution targets — they spin up headless Chromium instances inside isolated containers or VMs.
          </p>
          <p className="mb-3 text-[15px] leading-relaxed">
            <strong>Oya is the Control Plane.</strong> It sits above the execution targets and manages the state, identity, authentication, challenge resolution, and orchestration that production agent fleets require:
          </p>
          <ul className="list-disc list-inside space-y-1.5 mb-5 text-[15px] leading-relaxed">
            <li><strong>Universal Router:</strong> Exposes unified CDP (<InlineCode>/connect</InlineCode>), MCP, and REST interfaces. Route requests across providers with priority order and automatic failover.</li>
            <li><strong>Deterministic Personas:</strong> Mathematically seeded device profiles. Canvas, WebGL, audio, and client rects stay byte-identical across restarts, bound to a dedicated cookie jar and proxy.</li>
            <li><strong>Sign-In-Once Desktop Pairing:</strong> Transfer authenticated sessions from real desktop Chrome (with WebAuthn, passkeys, and Google SSO) to remote personas via single-use encrypted codes.</li>
            <li><strong>Two-Tier Challenges:</strong> Automatic native delegation to CAPTCHA solvers, automated TOTP and SMS/email relays, and sub-second interactive live stream handoffs for human intervention.</li>
            <li><strong>Fleet Governance:</strong> High-density console for 1,000+ browsers, real-time command activity logs, Prometheus metrics (<InlineCode>/metrics</InlineCode>), and hourly spend attribution per tenant key.</li>
          </ul>

          <NoteBox>
            By decoupling the <em>control plane</em> from the <em>execution engine</em>, your agent codebase never has to know or care which cloud provider or bare-metal machine runs a session.
          </NoteBox>

          {/* ============ WHY OYA: 10X COMPARISON ============ */}
          <SectionHeading id="comparison">Why Oya: The 10x Advantage</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">
            Directly coding agents to single-vendor browser runners creates brittle architectures. Here is why an orchestrating control plane is 10x better than relying on raw point solutions:
          </p>
          <Table
            headers={['Dimension', 'Raw Runners (Browserbase, Steel, Anchor, Browser Use)', 'Oya Control Plane']}
            rows={[
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
            ]}
          />

          {/* ============ ROUTING & FAILOVER ============ */}
          <SectionHeading id="routing-failover">Multi-Provider Routing & Failover</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">
            Configure providers in the dashboard under <strong>Control → Providers</strong> or via the API. Each provider has a unique route name, vendor type, priority (0 goes first), and session capacity.
          </p>
          <CodeBlock>{`// Point any CDP client at the Oya Control Plane gateway:
const browser = await chromium.connectOverCDP(
  "wss://browser.getoya.ai/connect?token=YOUR_OYA_KEY"
);

// Oya selects the highest-priority available provider.
// If Steel errors or hits rate limits, Oya instantly fails over to Browserbase or Oya Cloud.`}</CodeBlock>
          <p className="mb-3 text-[15px] leading-relaxed">
            When a connection attempt to an upstream vendor fails, the control plane immediately catches the error, puts the failing route into a cooldown period, and dispatches the connection to the next healthy provider in priority order. Your client application never observes a disconnect.
          </p>

          {/* ============ STEALTH BENCHMARKS ============ */}
          <SectionHeading id="stealth-benchmarks">Stealth & Live Benchmarks</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">
            Rather than making unsubstantiated marketing claims about detection resistance, Oya includes an open testing suite that benchmarks browser evasion against real detectors:
          </p>
          <CodeBlock>{`oya stealth-test            # Score local probe suite
oya stealth-test --live     # Benchmark live against Bot.Sannysoft and CreepJS`}</CodeBlock>
          <p className="mb-3 text-[15px] leading-relaxed">
            The suite tests canvas noise, WebGL renderer and vendor strings, AudioContext noise, client rects, plugins, <InlineCode>navigator.webdriver</InlineCode>, <InlineCode>userAgentData</InlineCode>, media devices, and <InlineCode>Function.prototype.toString</InlineCode> masking.
          </p>
          <NoteBox>
            Oya deliberately does <strong>not</strong> double-layer custom stealth over providers that already ship tuned anti-bot stealth (Anchor, Browserbase, Steel, Browser Use). Double-masking causes internal contradictions that anti-bot heuristics detect. On those providers, Oya manages the persona identity, cookie jar, residential proxy, and concurrency limits.
          </NoteBox>

          {/* ============ QUICKSTART ============ */}
          <SectionHeading id="quickstart">Quickstart</SectionHeading>
          <CodeBlock>{`npm i @oya-ai/browser
npm i -g @oya-ai/cli && oya login && oya init`}</CodeBlock>
          <CodeBlock>{`import { Oya } from "@oya-ai/browser";

const oya = new Oya();                                    // OYA_API_KEY
const browser = await oya.browser.start({ persona: "auto", captcha: "auto" });
await browser.goto("https://example.com");`}</CodeBlock>
          <p className="mb-3 text-[15px] leading-relaxed">
            That is the whole surface. Which provider actually runs the browser — Oya Cloud, your own
            machines, Browser Use, Browserbase, Steel, Anchor, or a CDP URL you hand us — is a setting
            on your API key, chosen once during <InlineAnchor onClick={() => navClick('onboarding')}>onboarding</InlineAnchor>.
            Your code never branches on it.
          </p>
          <NoteBox>
            The API key is the identity for everything: browsers, personas, cookies, settings, usage
            and audit history are all scoped to it, and one key can never see another&apos;s.
          </NoteBox>

          {/* ============ SDK ============ */}
          <SectionHeading id="sdk">SDK</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">
            <InlineCode>@oya-ai/browser</InlineCode> is TypeScript with no runtime dependencies, shipped as
            ESM, CJS and types. Element IDs come from <InlineCode>analyze()</InlineCode> and are only valid
            until the page changes — after a navigation or a click that redraws, analyze again.
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
            <InlineCode>browser.cdpUrl</InlineCode> is our gateway URL, not the vendor&apos;s — point
            Playwright, Puppeteer, Stagehand or browser-use at it and you get routing, profile capture
            and session recording without any of them knowing this exists.
          </p>
          <CodeBlock>{`const browser = await oya.browser.start();
const pw = await chromium.connectOverCDP(browser.cdpUrl);`}</CodeBlock>
          <p className="mb-3 text-[15px] leading-relaxed">
            The gateway also answers <InlineCode>/json/version</InlineCode> and <InlineCode>/json/list</InlineCode>,
            which is what lets those clients treat it as an ordinary browser.
          </p>

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
            Flags and <InlineCode>OYA_API_KEY</InlineCode> / <InlineCode>OYA_BASE_URL</InlineCode> beat the
            saved file, so CI never needs <InlineCode>oya login</InlineCode>. The key is stored at{' '}
            <InlineCode>~/.oya/config.json</InlineCode>, mode 600.
          </p>

          {/* ============ CREATE KEY ============ */}
          <SectionHeading id="create-key">Create API Key</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">
            Go to the <InlineLink href="/dashboard">dashboard</InlineLink>. Open the API key menu to create or select a key for your workspace.
          </p>
          <p className="mb-3 text-[15px] leading-relaxed">
            Your key is scoped — you only see browsers connected with your key. Other users&apos; browsers are invisible to you.
          </p>
          <WarnBox>
            Save your key somewhere safe. If you lose it, you&apos;ll need to generate a new one. The old key still works for any browsers already connected with it.
          </WarnBox>

          {/* ============ DOWNLOAD ============ */}
          <SectionHeading id="download">Desktop Sign-in</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">
            For browsers on Oya infrastructure, the desktop app is a one-time step: log into the sites
            your agents need, and those cookies move to the remote browsers, which run the same
            fingerprint as that identity. The agent arrives already signed in, and the site sees one
            device returning rather than a fleet sharing an account.
          </p>
          <p className="mb-3 text-[15px] leading-relaxed">
            Onboarding and Settings both have an <strong>Open the desktop browser</strong> button. It
            builds an <InlineCode>oya://</InlineCode> link carrying a single-use pairing code — never
            your API key, because a protocol URL is reachable by any page you visit and lands in OS logs
            on the way. The app exchanges that code over HTTPS with the server the link names.
          </p>
          <WarnBox>
            The desktop app asks before connecting, naming the destination host, with Cancel as the
            default. Connecting shares that browser&apos;s cookies and logged-in sessions with the
            control plane it dials — so if a web page opened the dialog rather than your own dashboard,
            cancel it.
          </WarnBox>
          <Table
            headers={['Platform', 'Download']}
            rows={[
              [
                'macOS (Intel + Apple Silicon)',
                <a key="mac" href="/downloads/Oya.Browser-1.0.82-universal.dmg" className="text-accent hover:text-accent-hover transition-colors">Oya Browser.dmg</a>,
              ],
              [
                'Windows (x64)',
                <a key="win" href="/downloads/Oya.Browser-1.0.82-x64.exe" className="text-accent hover:text-accent-hover transition-colors">Oya Browser.exe</a>,
              ],
              [
                'Linux (x64)',
                <a key="linux" href="/downloads/Oya.Browser-1.0.82-x64.AppImage" className="text-accent hover:text-accent-hover transition-colors">Oya Browser.AppImage</a>,
              ],
            ]}
          />
          <p className="mb-3 text-[15px] leading-relaxed">
            <strong>macOS:</strong> Open the .dmg, drag to Applications. On first launch, macOS may block the app because it&apos;s not notarized. Fix:
          </p>
          <CodeBlock>{`xattr -cr /Applications/Oya\\ Browser.app`}</CodeBlock>
          <p className="mb-3 text-[15px] leading-relaxed">
            Or: right-click the app → Open → Open (bypasses Gatekeeper once).
          </p>
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
            Each <InlineCode>--user-data-dir</InlineCode> gets its own cookies, logins, and config — fully isolated sessions.
          </p>

          {/* ============ CONNECT ============ */}
          <SectionHeading id="connect">Connect</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">Open Oya Browser. The setup screen appears on first launch.</p>
          <Table
            headers={['Field', 'Value']}
            rows={[
              ['Server URL', <InlineCode key="url">wss://browser.getoya.ai/ws</InlineCode>],
              ['API Key', 'The key you generated in the dashboard'],
              ['Browser Name', 'Optional — how it shows in the dashboard'],
            ]}
          />
          <p className="mb-3 text-[15px] leading-relaxed">
            Click <strong>Connect</strong>. The green dot in the toolbar confirms the connection. Your browser now appears in the <InlineLink href="/dashboard">dashboard</InlineLink>.
          </p>

          {/* ============ MCP SETUP ============ */}
          <SectionHeading id="mcp-setup">MCP Setup</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">
            Oya Browser exposes each connected browser as an MCP server at:
          </p>
          <CodeBlock>{'https://browser.getoya.ai/mcp/{BROWSER_ID}'}</CodeBlock>
          <p className="mb-3 text-[15px] leading-relaxed">
            Get your browser&apos;s ID from the <InlineLink href="/dashboard">dashboard</InlineLink> (shown under each browser name, or in the MCP Tools tab).
          </p>

          <h3 id="cursor" className="text-base font-semibold mt-6 mb-2 text-text">Cursor</h3>
          <p className="mb-3 text-[15px] leading-relaxed">
            Add to <InlineCode>.cursor/mcp.json</InlineCode> in your project:
          </p>
          <CodeBlock>{`{
  "mcpServers": {
    "oya-browser": {
      "url": "https://browser.getoya.ai/mcp/YOUR_BROWSER_ID",
      "transport": "streamable-http",
      "headers": {
        "Authorization": "Bearer YOUR_API_KEY"
      }
    }
  }
}`}</CodeBlock>

          <h3 id="claude-desktop" className="text-base font-semibold mt-6 mb-2 text-text">Claude Desktop</h3>
          <p className="mb-3 text-[15px] leading-relaxed">
            Add to Claude Desktop&apos;s MCP config (Settings → Developer → Edit Config):
          </p>
          <CodeBlock>{`{
  "mcpServers": {
    "oya-browser": {
      "url": "https://browser.getoya.ai/mcp/YOUR_BROWSER_ID",
      "transport": "streamable-http",
      "headers": {
        "Authorization": "Bearer YOUR_API_KEY"
      }
    }
  }
}`}</CodeBlock>

          <h3 id="claude-code" className="text-base font-semibold mt-6 mb-2 text-text">Claude Code</h3>
          <p className="mb-3 text-[15px] leading-relaxed">
            Same config — add to your project&apos;s <InlineCode>.claude/mcp.json</InlineCode> or use the <InlineCode>/browse</InlineCode> skill command included in the repo.
          </p>

          {/* ============ TOOLS ============ */}
          <SectionHeading id="analyze_page">analyze_page</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">
            Analyzes the current page. Returns the full page as structured markdown with every interactive element numbered.
          </p>
          <CodeBlock>{`// No parameters
analyze_page()`}</CodeBlock>
          <p className="mb-3 text-[15px] leading-relaxed">Returns:</p>
          <ul className="list-disc list-inside space-y-1 mb-4 text-[15px] leading-relaxed">
            <li>Page metadata — URL, title, viewport size, scroll position</li>
            <li>Full page content as markdown with inline element annotations like <InlineCode>{`[#5 button "Submit"]`}</InlineCode></li>
            <li>Element index — all elements listed with IDs, types, labels, visibility flags</li>
          </ul>
          <NoteBox>
            Always call <InlineCode>analyze_page</InlineCode> before using <InlineCode>click</InlineCode> or <InlineCode>type</InlineCode>. Element IDs only exist after analysis and reset on every call.
          </NoteBox>

          <SectionHeading id="navigate">navigate</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">Navigate the browser to a URL.</p>
          <CodeBlock>{'navigate({ url: "https://example.com" })'}</CodeBlock>
          <NoteBox>
            After navigating, call <InlineCode>analyze_page</InlineCode> again — old element IDs are invalid on the new page.
          </NoteBox>

          <SectionHeading id="click">click</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">
            Click an interactive element by its ID number from <InlineCode>analyze_page</InlineCode>.
          </p>
          <CodeBlock>{'click({ element_id: 13 })'}</CodeBlock>
          <p className="mb-3 text-[15px] leading-relaxed">
            The element was tagged with <InlineCode>data-ac-id=&quot;13&quot;</InlineCode> during analysis — the click resolves via a single <InlineCode>querySelector</InlineCode>.
          </p>

          <SectionHeading id="type">type</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">
            Type text into an input element. Clears existing content first, then types character by character with realistic key events.
          </p>
          <CodeBlock>{'type({ element_id: 9, text: "hello world" })'}</CodeBlock>

          <SectionHeading id="press_key">press_key</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">
            Press a keyboard key. Useful for submitting forms (Enter), dismissing dialogs (Escape), or navigating (Tab, arrows).
          </p>
          <CodeBlock>{'press_key({ key: "Enter" })'}</CodeBlock>
          <p className="mb-3 text-[15px] leading-relaxed">
            Supported keys: <InlineCode>Enter</InlineCode>, <InlineCode>Escape</InlineCode>, <InlineCode>Tab</InlineCode>, <InlineCode>Backspace</InlineCode>, <InlineCode>ArrowDown</InlineCode>, <InlineCode>ArrowUp</InlineCode>, or any character.
          </p>

          <SectionHeading id="screenshot">screenshot</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">Capture the visible tab as a base64 PNG image.</p>
          <CodeBlock>screenshot()</CodeBlock>

          <SectionHeading id="scroll">scroll</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">Scroll the page up or down.</p>
          <CodeBlock>{'scroll({ direction: "down", amount: 500 })'}</CodeBlock>
          <Table
            headers={['Param', 'Type', 'Description']}
            rows={[
              [
                <InlineCode key="dir">direction</InlineCode>,
                <><InlineCode>&quot;up&quot;</InlineCode> | <InlineCode>&quot;down&quot;</InlineCode></>,
                'Scroll direction',
              ],
              [
                <InlineCode key="amt">amount</InlineCode>,
                'number (optional)',
                'Pixels to scroll, default 500',
              ],
            ]}
          />

          <SectionHeading id="tabs">Tab Management</SectionHeading>

          <h3 className="text-base font-semibold mt-6 mb-2 text-text">list_tabs</h3>
          <p className="mb-3 text-[15px] leading-relaxed">List all open tabs with ID, title, URL, and which is active.</p>
          <CodeBlock>list_tabs()</CodeBlock>

          <h3 className="text-base font-semibold mt-6 mb-2 text-text">open_tab</h3>
          <p className="mb-3 text-[15px] leading-relaxed">Open a new tab, optionally at a URL.</p>
          <CodeBlock>{'open_tab({ url: "https://gmail.com" })'}</CodeBlock>

          <h3 className="text-base font-semibold mt-6 mb-2 text-text">switch_tab</h3>
          <p className="mb-3 text-[15px] leading-relaxed">Switch to a tab by ID (from <InlineCode>list_tabs</InlineCode>).</p>
          <CodeBlock>{'switch_tab({ tab_id: 2 })'}</CodeBlock>

          <h3 className="text-base font-semibold mt-6 mb-2 text-text">close_tab</h3>
          <p className="mb-3 text-[15px] leading-relaxed">Close a tab. Closes the active tab if no ID specified.</p>
          <CodeBlock>{'close_tab({ tab_id: 3 })'}</CodeBlock>

          <SectionHeading id="wait">wait</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">
            Wait for an element matching a CSS selector to appear on the page.
          </p>
          <CodeBlock>{'wait({ selector: ".results", timeout: 10000 })'}</CodeBlock>

          {/* ============ ANONYMITY ============ */}
          {/* ============ PERSONAS ============ */}
          <SectionHeading id="personas">Personas</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">
            A <strong>persona</strong> is one identity: a fingerprint, a cookie jar and a proxy, bound
            together and stable for its life. One persona is one device.
          </p>
          <p className="mb-3 text-[15px] leading-relaxed">
            There are two ways to get caught, and they are mirror images of each other:
          </p>
          <Table
            headers={['Shape', 'Signal']}
            rows={[
              ['One account seen from many device fingerprints', 'Textbook bot farm'],
              ['One device fingerprint across many accounts — or 1,000 concurrent sessions', 'Device farm'],
            ]}
          />
          <p className="mb-3 text-[15px] leading-relaxed">
            Binding the fingerprint to your API key avoids the first and walks straight into the second.
            Binding it to each browser avoids the second and walks into the first. So the binding sits at
            the level that actually corresponds to a device:
          </p>
          <CodeBlock>{`persona = fingerprint + cookie jar + proxy       # one identity, one device
API key = a group of personas                    # your fleet`}</CodeBlock>
          <p className="mb-3 text-[15px] leading-relaxed">
            A persona&apos;s fingerprint is derived from a stored seed, so it is byte-identical across
            restarts — a returning session looks like a returning device, not a new one.
          </p>
          <CodeBlock>{`const p = await oya.personas.create({ name: "acme-ops" });
const browser = await oya.browser.start({ persona: p.id });

await oya.personas.list();     // includes activeBrowsers and maxConcurrent
await oya.personas.remove(p.id);`}</CodeBlock>
          <NoteBox>
            Every API key has a <strong>default</strong> persona whose seed reproduces the fingerprint
            that key had before personas existed. If you run a single account, nothing changed for you.
          </NoteBox>

          <h3 id="rotation" className="text-base font-semibold mt-6 mb-2 text-text">Rotation and concurrency</h3>
          <p className="mb-3 text-[15px] leading-relaxed">
            Rotation means picking a <em>different</em> persona — never giving one persona a new
            fingerprint. <InlineCode>persona: &apos;auto&apos;</InlineCode> selects the least recently used
            persona that is still under its concurrency cap.
          </p>
          <p className="mb-3 text-[15px] leading-relaxed">
            Concurrency is capped per persona, because one laptop cannot be in a thousand places at once.
            Named personas default to 2 (a phone and a laptop is plausible); the default persona is
            uncapped so an existing fleet does not break on upgrade. Past the cap you get a clear 429
            rather than a silent breach, and <InlineCode>activeBrowsers</InlineCode> is visible in the
            dashboard and as a Prometheus metric.
          </p>

          {/* ============ CHALLENGES ============ */}
          <SectionHeading id="captcha">CAPTCHA</SectionHeading>
          <CodeBlock>{`await browser.solveCaptcha();                    // explicit
oya.browser.start({ captcha: 'auto' });          // solve as they appear`}</CodeBlock>
          <p className="mb-3 text-[15px] leading-relaxed">
            Detects reCAPTCHA v2/v3, hCaptcha and Turnstile. Providers that solve natively — Anchor,
            Browserbase, Steel, Browser Use — are left to do it rather than paying twice and racing
            their attempt. Everything else goes to your configured solver (CapSolver or 2Captcha).
          </p>
          <p className="mb-3 text-[15px] leading-relaxed">
            Returns <InlineCode>{`{ solved, method: 'provider' | 'solver' | 'none' }`}</InlineCode>. A
            failure returns <InlineCode>solved: false</InlineCode> — a silent no-op that leaves an agent
            stuck is worse than a clear answer.
          </p>
          <WarnBox>
            Automated solving conflicts with some sites&apos; terms of service. Sessions that used it are
            recorded in the audit trail so you can see which.
          </WarnBox>

          <SectionHeading id="mfa">MFA</SectionHeading>
          <CodeBlock>{`await oya.personas.setMfa(id, { type: 'totp', secret: 'JBSWY3DPEHPK3PXP' });
await oya.personas.setMfa(id, { type: 'email', url: 'https://mail.example/api/latest' });

const r = await browser.completeMfa();
if (!r.completed) open(r.liveViewUrl);   // finish it by hand`}</CodeBlock>
          <p className="mb-3 text-[15px] leading-relaxed">
            TOTP is generated locally (RFC 6238). Email and SMS one-time codes are polled from a relay
            endpoint you supply, within a bounded window — the code does not exist yet when the prompt
            appears. When nothing automated can answer, <InlineCode>liveViewUrl</InlineCode> is where a
            person finishes; that is also the only workable answer for push-approval MFA.
          </p>
          <NoteBox>
            TOTP seeds are credential material of the same weight as a password: sealed at rest with
            AES-256-GCM, audited on use, and never returned by the API. The relay URL is checked against
            private and link-local ranges when you store it <em>and</em> on every poll, because a public
            name says nothing about where it resolves later.
          </NoteBox>

          <SectionHeading id="anonymity">Anonymity</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">
            Create and manage browser profiles with unique fingerprints, proxy routing, and isolated cookie stores. Each profile is a complete identity — different canvas hash, WebGL renderer, navigator properties, and session storage. Switch identities with a single MCP call.
          </p>
          <NoteBox>
            Every browser runs as a persona: a fingerprint, cookie jar and proxy bound together and stable for its life. Rotation means choosing a different persona, never re-rolling one.
          </NoteBox>

          <h3 id="fingerprint" className="text-base font-semibold mt-6 mb-2 text-text">Fingerprint Spoofing</h3>
          <p className="mb-3 text-[15px] leading-relaxed">
            Each profile generates a coherent set of browser fingerprints that are internally consistent per platform. A Win32 profile gets Windows GPU strings, Windows fonts, and matching screen resolutions.
          </p>
          <ul className="list-disc list-inside space-y-1 mb-4 text-[15px] leading-relaxed">
            <li><strong>Canvas</strong> — deterministic pixel noise on <InlineCode>toDataURL</InlineCode> and <InlineCode>toBlob</InlineCode></li>
            <li><strong>WebGL</strong> — spoofed vendor/renderer strings from real GPU database</li>
            <li><strong>AudioContext</strong> — noise on <InlineCode>OfflineAudioContext.startRendering</InlineCode></li>
            <li><strong>ClientRects</strong> — sub-pixel noise on <InlineCode>getBoundingClientRect</InlineCode> (bypassed internally for click accuracy)</li>
            <li><strong>Navigator</strong> — platform, hardwareConcurrency, deviceMemory, languages, vendor</li>
            <li><strong>Screen</strong> — width, height, colorDepth, devicePixelRatio</li>
            <li><strong>WebRTC</strong> — ICE candidates stripped to prevent local IP leak</li>
            <li><strong>Fonts</strong> — platform-consistent font sets</li>
          </ul>

          <h3 id="proxy-support" className="text-base font-semibold mt-6 mb-2 text-text">Proxy Support</h3>
          <p className="mb-3 text-[15px] leading-relaxed">
            Each profile can include a SOCKS5 or HTTP/HTTPS proxy. The proxy is applied at the Electron session level — all traffic routes through it, including DNS (for SOCKS5). Timezone and locale auto-match the proxy&apos;s geographic location via CDP Emulation.
          </p>
          <CodeBlock>{'create_profile({\n  platform: "Win32",\n  timezone: "America/New_York",\n  proxy_type: "socks5",\n  proxy_host: "1.2.3.4",\n  proxy_port: 1080,\n  proxy_username: "user",\n  proxy_password: "pass"\n})'}</CodeBlock>

          <h3 id="stealth" className="text-base font-semibold mt-6 mb-2 text-text">Anti-Detection Stealth</h3>
          <p className="mb-3 text-[15px] leading-relaxed">
            Always active — no configuration needed. The stealth layer removes automation indicators that anti-bot systems check for:
          </p>
          <ul className="list-disc list-inside space-y-1 mb-4 text-[15px] leading-relaxed">
            <li><InlineCode>navigator.webdriver</InlineCode> removed</li>
            <li>Electron globals (<InlineCode>window.process</InlineCode>, <InlineCode>window.require</InlineCode>) deleted</li>
            <li><InlineCode>window.chrome</InlineCode> fixed to match real Chrome (app, runtime, csi, loadTimes)</li>
            <li><InlineCode>navigator.plugins</InlineCode> populated with PDF viewers</li>
            <li><InlineCode>navigator.permissions.query</InlineCode> patched</li>
            <li>Sec-CH-UA headers rewritten to hide Electron</li>
            <li>Google telemetry domains blocked at the network level</li>
          </ul>

          <SectionHeading id="list_profiles">list_profiles</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">
            List all available anonymity profiles on the connected browser. Shows which profile is active.
          </p>
          <CodeBlock>{'list_profiles()'}</CodeBlock>
          <p className="mb-3 text-[15px] leading-relaxed">Returns each profile&apos;s ID, platform, timezone, and whether it has a proxy configured.</p>

          <SectionHeading id="create_profile">create_profile</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">
            Create a new anonymity profile with a randomized browser fingerprint. All values are generated to be internally consistent for the chosen platform.
          </p>
          <CodeBlock>{'create_profile({\n  platform: "Win32",\n  timezone: "Europe/London",\n  locale: "en-GB"\n})'}</CodeBlock>
          <Table
            headers={['Param', 'Type', 'Description']}
            rows={[
              [<InlineCode key="p">platform</InlineCode>, 'string', 'Win32, MacIntel, or Linux x86_64'],
              [<InlineCode key="tz">timezone</InlineCode>, 'string', 'IANA timezone (e.g. America/New_York)'],
              [<InlineCode key="lo">locale</InlineCode>, 'string', 'Locale (e.g. en-US, en-GB)'],
              [<InlineCode key="pt">proxy_type</InlineCode>, 'string', 'http or socks5'],
              [<InlineCode key="ph">proxy_host</InlineCode>, 'string', 'Proxy server hostname or IP'],
              [<InlineCode key="pp">proxy_port</InlineCode>, 'number', 'Proxy server port'],
              [<InlineCode key="pu">proxy_username</InlineCode>, 'string', 'Proxy auth username'],
              [<InlineCode key="pw">proxy_password</InlineCode>, 'string', 'Proxy auth password'],
            ]}
          />

          <SectionHeading id="set_profile">set_profile</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">
            Switch to a different anonymity profile. This closes all open tabs and reopens the browser with the new profile&apos;s fingerprint, proxy, timezone, and isolated cookie store.
          </p>
          <CodeBlock>{'set_profile({ profile_id: "profile-a1b2c3" })'}</CodeBlock>
          <Table
            headers={['Param', 'Type', 'Description']}
            rows={[
              [<InlineCode key="pid">profile_id</InlineCode>, 'string (required)', 'ID of the profile to activate'],
            ]}
          />
          <WarnBox>
            Switching profiles closes all open tabs. The browser reopens on google.com with the new identity.
          </WarnBox>

          {/* ============ DASHBOARD ============ */}
          <SectionHeading id="dashboard-overview">Dashboard</SectionHeading>
          <NoteBox><strong>Browsers, commands, and CDP sessions</strong><br/>A browser is a running desktop or cloud instance. REST commands, including curl requests to <InlineCode>/api/browsers/:id/command</InlineCode>, appear in that browser’s Activity history and count toward Usage. A CDP session is a persistent client connection through <InlineCode>/connect</InlineCode>, typically from Playwright or Puppeteer. Find these under Control → CDP sessions.</NoteBox>
          <p className="mb-3 text-[15px] leading-relaxed">
            The <InlineLink href="/dashboard">dashboard</InlineLink> at <InlineCode>/dashboard</InlineCode> is the control panel. It shows your connected browsers and lets you interact with them.
          </p>
          <p className="mb-3 text-[15px] leading-relaxed">Built to hold a thousand browsers and let you act on any one of them:</p>
          <ul className="list-disc list-inside space-y-1 mb-4 text-[15px] leading-relaxed">
            <li><strong>Browsers</strong> — a health strip (every number is a filter) over a dense table: health, persona, provider, current page, commands · errors, seen, uptime. Select a row to open the panel: URL bar, a bounded <em>interactive</em> live view, screenshot, elements, stats, and the activity log — what that browser has been doing, newest first.</li>
            <li><strong>Personas</strong> — one identity each. Create with a chosen device and a live fingerprint preview; edit name, cap, proxy pin and MFA; the device itself is locked, with <em>Clone</em> for when you want a different one.</li>
            <li><strong>Control</strong> — health, gateway sessions, providers and routing, per-key usage, the audit trail, recordings.</li>
          </ul>
          <h3 className="text-base font-semibold mt-6 mb-2 text-text">Adding a provider</h3>
          <p className="mb-3 text-[15px] leading-relaxed">Open <strong>Control → Providers → Add provider</strong>. Give the route a unique name, choose a vendor, and enter its API key. A credential already saved in Settings can be reused. For your own Chrome, supply its CDP WebSocket URL instead.</p>
          <p className="mb-3 text-[15px] leading-relaxed">Set the session capacity and routing priority (0 goes first). Providers and your routing strategy are saved for your Oya key across restarts; credentials and connection URLs are encrypted. Saving a provider does not launch a browser or verify its credentials. Its first connection does that. End active sessions before removing a route.</p>
          <p className="mb-3 text-[15px] leading-relaxed">These routes serve new CDP connections to <InlineCode>/connect?token=YOUR_OYA_KEY</InlineCode>. The <strong>Start browser</strong> action uses your provider selection in <strong>Settings → Browsers</strong>. Attaching with <InlineCode>?browser=ID</InlineCode> connects to that existing browser.</p>
          <h3 className="text-base font-semibold mt-6 mb-2 text-text">Driving a browser from the live view</h3>
          <p className="mb-3 text-[15px] leading-relaxed">Choose <strong>Stream</strong> in a browser panel, or <strong>Open live stream in a tab</strong> from its menu, to open an interactive viewer in a separate tab. Your dashboard key authorizes the viewer. The <InlineCode>/api/live/:id</InlineCode> endpoint is the raw event stream for integrations.</p>
          <p className="mb-3 text-[15px] leading-relaxed">
            Click to control. Clicks land at the page pixel under the cursor, a drag is a drag, the wheel
            scrolls, typing is batched into <InlineCode>keyboard_type</InlineCode> and the named keys go
            as <InlineCode>press_key</InlineCode>. <InlineCode>Esc</InlineCode> hands the keyboard back. What
            was typed is never written to the activity log — it records <em>2 chars</em>, not the text.
          </p>
          <h3 className="text-base font-semibold mt-6 mb-2 text-text">Connect to a browser that is already running</h3>
          <p className="mb-3 text-[15px] leading-relaxed">
            Right-click any row (or press <strong>Connect</strong> in the panel) for code that targets
            that exact browser: SDK, CLI, an MCP config, curl — and for CDP-backed browsers, a
            Playwright <InlineCode>connectOverCDP</InlineCode> URL. Snippets are written for this
            deployment and your key; the key is masked until you ask, and copy always copies the real one.
          </p>
          <CodeBlock>{`// Attach through the gateway to one browser in the fleet. Closing your
// client leaves the browser running.
const browser = await chromium.connectOverCDP(
  "wss://<host>/connect?token=<api-key>&browser=<browser-id>",
);`}</CodeBlock>
          <p className="mb-3 text-[15px] leading-relaxed">
            Only CDP-backed browsers (Browserbase, Steel, Anchor, your own Chrome) have an endpoint
            to attach to; an Oya client is driven over its own socket, so use the SDK, CLI or MCP for those.
          </p>

          <h3 className="text-base font-semibold mt-6 mb-2 text-text">Stop means stop</h3>
          <p className="mb-3 text-[15px] leading-relaxed">
            One button, one endpoint (<InlineCode>POST /browsers/:id/stop</InlineCode>). A cloud browser&apos;s
            sandbox is destroyed so billing ends; a CDP browser is handed back to its provider; a desktop
            browser disconnects. The confirm says which. Bulk stop takes <InlineCode>{`{ids: [...]}`}</InlineCode> or <InlineCode>{`{all: true}`}</InlineCode>.
          </p>
          <h3 className="text-base font-semibold mt-6 mb-2 text-text">Keyboard</h3>
          <Table headers={['Key', 'Does']} rows={[
            ['⌘/Ctrl 1 · 2 · 3', 'Browsers · Personas · Control'],
            ['n', 'Start a browser'],
            ['/', 'Filter the fleet'],
            ['↑ ↓ or j k', 'Move the selection'],
            ['x', 'Stop the selected browser(s)'],
            ['l · r · s', 'URL bar · reload · screenshot'],
            ['Esc', 'Close the panel, or release the keyboard from the live view'],
            ['?', 'The full list'],
          ]} />
          <p className="mb-3 text-[15px] leading-relaxed">
            You can sign in with an account, or by pasting an API key — a self-hosted deployment with{' '}
            <InlineCode>API_KEYS</InlineCode> and no database has no accounts, and still needs its own UI.
          </p>

          <h3 id="onboarding" className="text-base font-semibold mt-6 mb-2 text-text">Onboarding</h3>
          <p className="mb-3 text-[15px] leading-relaxed">
            A key that has not been set up gets a four-step wizard. Everything it asks is stored against
            that key — nothing lands in an environment variable, and nothing is inherited from an account.
          </p>
          <ol className="list-decimal list-inside space-y-1.5 mb-4 text-[15px] leading-relaxed">
            <li><strong>Model</strong> — Claude or OpenAI, your key, your default model</li>
            <li><strong>Browsers</strong> — Oya Cloud, Oya self-hosted, Browser Use, Browserbase, Steel, Anchor, or your own CDP URL</li>
            <li><strong>Challenges</strong> — a CAPTCHA solver, or none</li>
            <li><strong>Sign in</strong> — one click into the desktop browser, for Oya providers only</li>
          </ol>
          <p className="mb-3 text-[15px] leading-relaxed">
            The same choices are available any time from Settings, and <InlineCode>oya init</InlineCode>{' '}
            walks the identical flow in a terminal.
          </p>

          <h3 className="text-base font-semibold mt-6 mb-2 text-text">Dev Panel (Desktop App)</h3>
          <p className="mb-3 text-[15px] leading-relaxed">
            The desktop app&apos;s dev panel (<InlineCode>{'{}'}</InlineCode> button in the toolbar) has four tabs:
          </p>
          <ul className="list-disc list-inside space-y-1 mb-4 text-[15px] leading-relaxed">
            <li><strong>Chat</strong> — natural language browser control with formatted responses and tool badges</li>
            <li><strong>Actions</strong> — quick-fire buttons and input fields for every command: analyze, screenshot, navigate, click by element #, type, press keys, hover, scroll, wait, tab management</li>
            <li><strong>Network</strong> — live WebSocket traffic with IN/OUT badges, expandable payloads, filter by direction or type (All, In, Out, Commands, Results)</li>
            <li><strong>Source</strong> — view the page as AI sees it: toggle between Markdown (analyzePage output) and HTML source, refresh on demand</li>
          </ul>

          <h3 id="live-view" className="text-base font-semibold mt-6 mb-2 text-text">Live View</h3>
          <p className="mb-3 text-[15px] leading-relaxed">
            Select a browser on the Browsers tab to watch it work. Frames stream as JPEG over SSE at
            ~2fps. <InlineCode>browser.liveViewUrl()</InlineCode> is the console deep link for a person to
            open; <InlineCode>await browser.liveStreamUrl()</InlineCode> gives you the same frames to embed,
            with a single-use ticket that expires in 60 seconds — EventSource cannot set headers, and a URL
            that ends up in browser history should not be a permanent credential.
          </p>

          <h3 id="settings" className="text-base font-semibold mt-6 mb-2 text-text">Settings</h3>
          <p className="mb-3 text-[15px] leading-relaxed">
            The gear icon next to the API key bar. Everything here belongs to that key: model provider
            and credential, default model, browser provider and its credential, CAPTCHA solver, and the
            one-click desktop sign-in.
          </p>
          <p className="mb-3 text-[15px] leading-relaxed">
            Credentials are sealed at rest with AES-256-GCM and always read back masked. Saving the
            masked placeholder never overwrites the real value.
          </p>
          <NoteBox>
            A key that has set nothing falls back to the deployment-wide defaults. Changing <em>those</em>{' '}
            affects every key that has not set its own, so it needs <InlineCode>OYA_OPERATOR_TOKEN</InlineCode>{' '}
            via <InlineCode>POST /config/host</InlineCode> rather than any API key.
          </NoteBox>

          {/* ============ REST API ============ */}
          <SectionHeading id="rest-api">REST API</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">
            All endpoints require <InlineCode>Authorization: Bearer YOUR_API_KEY</InlineCode> header (except health and register). Interactive API testing available at <a href="/swagger" className="text-accent hover:text-accent-hover transition-colors">/swagger</a>.
          </p>
          <Table
            headers={['Method', 'Endpoint', 'Description']}
            rows={[
              [<InlineCode key="m1">GET</InlineCode>, <InlineCode key="e1">/health</InlineCode>, 'Server status + browser count'],
              [<InlineCode key="m2">POST</InlineCode>, <InlineCode key="e2">/register-key</InlineCode>, <span key="d2">Register a new API key (<InlineCode>{`{ "key": "..." }`}</InlineCode>)</span>],
              [<InlineCode key="m3">GET</InlineCode>, <InlineCode key="e3">/browsers</InlineCode>, 'List your connected browsers'],
              [<InlineCode key="ms">POST</InlineCode>, <InlineCode key="es">/browsers/start</InlineCode>, <span key="ds">Start one (<InlineCode>{`{ "persona": "auto" }`}</InlineCode>) — provider comes from your key</span>],
              [<InlineCode key="mg">GET</InlineCode>, <InlineCode key="eg">/browsers/:id</InlineCode>, 'One browser with counters, health and its recent activity'],
              [<InlineCode key="mst">POST</InlineCode>, <InlineCode key="est">/browsers/:id/stop</InlineCode>, 'Stop it — destroys a cloud sandbox, releases a CDP session'],
              [<InlineCode key="msb">POST</InlineCode>, <InlineCode key="esb">/browsers/stop</InlineCode>, <span key="dsb">Bulk: <InlineCode>{`{ "ids": [...] }`}</InlineCode> or <InlineCode>{`{ "all": true }`}</InlineCode></span>],
              [<InlineCode key="mf">GET</InlineCode>, <InlineCode key="ef">/fleet</InlineCode>, 'Totals by health, provider and persona; usage and limits'],
              [<InlineCode key="m4">POST</InlineCode>, <InlineCode key="e4">/browsers/:id/command</InlineCode>, <span key="d4">Send command (<InlineCode>{`{ "action": "...", "params": {} }`}</InlineCode>)</span>],
              [<InlineCode key="m5">POST</InlineCode>, <InlineCode key="e5">/browsers/:id/chat</InlineCode>, <span key="d5">Chat (<InlineCode>{`{ "messages": [...] }`}</InlineCode>)</span>],
              [<InlineCode key="m6">GET</InlineCode>, <InlineCode key="e6">/live/:id?ticket=...</InlineCode>, 'SSE live view frame stream (single-use ticket)'],
              [<InlineCode key="m7">GET/POST</InlineCode>, <InlineCode key="e7">/mcp/:id</InlineCode>, 'MCP Streamable HTTP endpoint'],
              [<InlineCode key="mp1">GET/POST</InlineCode>, <InlineCode key="ep1">/personas</InlineCode>, 'List or create personas'],
              [<InlineCode key="mp2">DELETE</InlineCode>, <InlineCode key="ep2">/personas/:id</InlineCode>, 'Delete a persona (409 while in use)'],
              [<InlineCode key="mp4">PUT</InlineCode>, <InlineCode key="ep4">/personas/:id</InlineCode>, 'Rename, set the cap or the proxy hint — never the device'],
              [<InlineCode key="mp5">POST</InlineCode>, <InlineCode key="ep5">/personas/:id/clone</InlineCode>, 'A new persona of the same kind of device'],
              [<InlineCode key="mp6">POST</InlineCode>, <InlineCode key="ep6">/personas/preview</InlineCode>, 'The fingerprint a set of choices would produce'],
              [<InlineCode key="mp7">GET</InlineCode>, <InlineCode key="ep7">/personas/options</InlineCode>, 'Platforms and their coherent timezones and locales'],
              [<InlineCode key="mp3">PUT</InlineCode>, <InlineCode key="ep3">/personas/:id/mfa</InlineCode>, 'Store a second factor'],
              [<InlineCode key="mc1">POST</InlineCode>, <InlineCode key="ec1">/browsers/:id/captcha</InlineCode>, 'Detect and clear a CAPTCHA'],
              [<InlineCode key="mc2">POST</InlineCode>, <InlineCode key="ec2">/browsers/:id/mfa</InlineCode>, 'Answer an MFA prompt'],
              [<InlineCode key="mu">GET</InlineCode>, <InlineCode key="eu">/usage</InlineCode>, "This key's usage, bucketed by hour"],
              [<InlineCode key="ma">GET</InlineCode>, <InlineCode key="ea">/audit</InlineCode>, "This key's audit history"],
              [<InlineCode key="m8">GET</InlineCode>, <InlineCode key="e8">/config</InlineCode>, "This key's settings, credentials masked"],
              [<InlineCode key="m9">POST</InlineCode>, <InlineCode key="e9">/config</InlineCode>, "Update this key's settings"],
            ]}
          />

          <h3 id="command-api" className="text-base font-semibold mt-6 mb-2 text-text">Command API Reference</h3>
          <p className="mb-3 text-[15px] leading-relaxed">
            Send commands via <InlineCode>POST /browsers/:id/command</InlineCode>. Each action uses only specific params — the rest are ignored.
          </p>

          <h3 className="text-base font-semibold mt-6 mb-2 text-text">Navigation Actions</h3>
          <Table
            headers={['Action', 'Params', 'Description']}
            rows={[
              [<InlineCode key="a1">navigate</InlineCode>, <span key="p1"><InlineCode>url</InlineCode> (required)</span>, 'Navigate to a URL'],
              [<InlineCode key="a2">open_tab</InlineCode>, <span key="p2"><InlineCode>url</InlineCode> (optional)</span>, 'Open a new tab'],
              [<InlineCode key="a3">switch_tab</InlineCode>, <span key="p3"><InlineCode>tab_id</InlineCode> (required)</span>, 'Activate a tab by ID'],
              [<InlineCode key="a4">close_tab</InlineCode>, <span key="p4"><InlineCode>tab_id</InlineCode> (optional, defaults to active)</span>, 'Close a tab'],
              [<InlineCode key="a5">list_tabs</InlineCode>, <em key="params">none</em>, 'List all open tabs'],
            ]}
          />

          <h3 className="text-base font-semibold mt-6 mb-2 text-text">Page Analysis Actions</h3>
          <Table
            headers={['Action', 'Params', 'Description']}
            rows={[
              [<InlineCode key="a1">analyze</InlineCode>, <em key="params">none</em>, 'Full page as markdown + numbered elements'],
              [<InlineCode key="a2">read_page</InlineCode>, <span key="p2"><InlineCode>selector</InlineCode> (optional), <InlineCode>limit</InlineCode> (default 50)</span>, 'Lightweight element listing'],
              [<InlineCode key="a3">screenshot</InlineCode>, <em key="params">none</em>, 'Capture page as PNG'],
            ]}
          />

          <h3 className="text-base font-semibold mt-6 mb-2 text-text">Interaction Actions</h3>
          <Table
            headers={['Action', 'Params', 'Description']}
            rows={[
              [<InlineCode key="a1">click</InlineCode>, <span key="p1"><InlineCode>selector</InlineCode> (e.g. <InlineCode>[data-ac-id=&quot;3&quot;]</InlineCode>)</span>, 'Click an element'],
              [<InlineCode key="a2">type</InlineCode>, <span key="p2"><InlineCode>selector</InlineCode> + <InlineCode>text</InlineCode></span>, 'Type into an input'],
              [<InlineCode key="a3">press_key</InlineCode>, <span key="p3"><InlineCode>key</InlineCode> (e.g. Enter, Tab, Escape)</span>, 'Press a keyboard key'],
              [<InlineCode key="a4">scroll</InlineCode>, <span key="p4"><InlineCode>direction</InlineCode> (up/down), <InlineCode>amount</InlineCode> (px, default 500)</span>, 'Scroll the page'],
              [<InlineCode key="a5">wait</InlineCode>, <span key="p5"><InlineCode>selector</InlineCode>, <InlineCode>timeout</InlineCode> (ms, default 10000)</span>, 'Wait for element to appear'],
            ]}
          />

          <h3 className="text-base font-semibold mt-6 mb-2 text-text">Examples</h3>
          <CodeBlock>{`// Navigate to a page
{ "action": "navigate", "params": { "url": "https://google.com" } }

// Analyze current page (no params needed)
{ "action": "analyze" }

// Click element #3 from analyze results
{ "action": "click", "params": { "selector": "[data-ac-id=\\"3\\"]" } }

// Type into element #9
{ "action": "type", "params": { "selector": "[data-ac-id=\\"9\\"]", "text": "hello world" } }

// Press Enter
{ "action": "press_key", "params": { "key": "Enter" } }

// Scroll down
{ "action": "scroll", "params": { "direction": "down", "amount": 500 } }

// Screenshot (no params needed)
{ "action": "screenshot" }

// List all tabs
{ "action": "list_tabs" }

// Open new tab
{ "action": "open_tab", "params": { "url": "https://gmail.com" } }

// Switch to tab
{ "action": "switch_tab", "params": { "tab_id": 2 } }

// Close tab (omit tab_id to close active tab)
{ "action": "close_tab", "params": { "tab_id": 3 } }

// Wait for element
{ "action": "wait", "params": { "selector": ".results", "timeout": 10000 } }

// Read page elements (lightweight)
{ "action": "read_page", "params": { "limit": 20 } }`}</CodeBlock>

          <h3 className="text-base font-semibold mt-6 mb-2 text-text">Typical Workflow</h3>
          <CodeBlock>{`1. navigate → go to the page
2. analyze  → understand the page, get element IDs
3. click / type / press_key / scroll → interact
4. analyze  → re-analyze after page changes (old IDs are invalid)
5. repeat until task is done`}</CodeBlock>

          {/* ============ WEBSOCKET ============ */}
          <SectionHeading id="websocket">WebSocket Protocol</SectionHeading>
          <p className="mb-3 text-[15px] leading-relaxed">
            Browsers connect via WebSocket at <InlineCode>wss://browser.getoya.ai/ws</InlineCode>.
          </p>

          <h3 className="text-base font-semibold mt-6 mb-2 text-text">Auth</h3>
          <p className="mb-3 text-[15px] leading-relaxed">First message from browser:</p>
          <CodeBlock>{`{ "type": "auth", "api_key": "...", "browser_id": "...", "browser_name": "..." }`}</CodeBlock>
          <p className="mb-3 text-[15px] leading-relaxed">Server responds:</p>
          <CodeBlock>{`{ "type": "auth_ok", "browser_id": "..." }`}</CodeBlock>

          <h3 className="text-base font-semibold mt-6 mb-2 text-text">Commands</h3>
          <p className="mb-3 text-[15px] leading-relaxed">Server → Browser:</p>
          <CodeBlock>{`{ "type": "cmd", "id": "uuid", "action": "analyze", "params": {} }`}</CodeBlock>
          <p className="mb-3 text-[15px] leading-relaxed">Browser → Server:</p>
          <CodeBlock>{`{ "type": "cmd_result", "id": "uuid", "ok": true, "data": { ... } }`}</CodeBlock>

          <h3 className="text-base font-semibold mt-6 mb-2 text-text">Ping/Pong</h3>
          <p className="mb-3 text-[15px] leading-relaxed">
            Both sides send <InlineCode>{`{ "type": "ping" }`}</InlineCode> and respond with <InlineCode>{`{ "type": "pong" }`}</InlineCode> every 15-20 seconds.
          </p>

          <h3 className="text-base font-semibold mt-6 mb-2 text-text">Live Stream</h3>
          <p className="mb-3 text-[15px] leading-relaxed">
            Server → Browser: <InlineCode>{`{ "type": "stream_start", "fps": 2 }`}</InlineCode>
          </p>
          <p className="mb-3 text-[15px] leading-relaxed">
            Browser → Server: <InlineCode>{`{ "type": "frame", "data": "data:image/jpeg;base64,..." }`}</InlineCode>
          </p>
          <p className="mb-3 text-[15px] leading-relaxed">
            Server → Browser: <InlineCode>{`{ "type": "stream_stop" }`}</InlineCode>
          </p>

        </div>
      </main>
    </div>
    </DocsNav.Provider>
  );
}

/* ------------------------------------------------------------------ */
/*  Helper Components                                                  */
/* ------------------------------------------------------------------ */

function NavSection({ icon, label, children }: { icon: React.ReactNode; label: string; children: React.ReactNode }) {
  return (
    <div className="mb-5">
      <div className="flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-widest text-text-dim mb-2">
        {icon}
        {label}
      </div>
      <div className="flex flex-col gap-0.5">{children}</div>
    </div>
  );
}

function NavLink({ id, children }: { id: string; children: React.ReactNode }) {
  const { active, navigate } = useContext(DocsNav);
  return <a href={'#'+id} aria-current={active === id ? 'location' : undefined} onClick={e => { e.preventDefault(); navigate(id); }} className={`rounded-md px-2.5 py-1.5 text-[12.5px] transition-colors ${active === id ? 'bg-accent/8 text-accent font-medium' : 'text-text-muted hover:bg-text/5 hover:text-text'}`}>{children}</a>;
}

function SectionHeading({ id, children, first }: { id: string; children: React.ReactNode; first?: boolean }) {
  return (
    <h2
      id={id}
      className={`text-[26px] font-medium tracking-tight text-text mb-5 scroll-mt-28 ${
        first ? 'mt-0' : 'mt-16 pt-10 border-t border-border'
      }`}
    >
      {children}
    </h2>
  );
}

function InlineCode({ children }: { children: React.ReactNode }) {
  return (
    <code className="bg-bg-elevated px-1.5 py-0.5 rounded text-sm font-mono text-text-muted">
      {children}
    </code>
  );
}

function CodeBlock({ children }: { children: string }) {
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState(false);
  async function copy() { try { await navigator.clipboard.writeText(children); setCopied(true); setTimeout(() => setCopied(false), 1800); } catch { setError(true); } }
  return <div className="my-5 min-w-0 overflow-hidden rounded-xl border border-border bg-bg-card">
    <div className="flex items-center justify-between border-b border-border px-4 py-2"><span className="font-mono text-[10px] text-text-dim">Example</span><button onClick={copy} className="btn-ghost h-7 text-[11px]" aria-label="Copy code">{copied ? <Check size={12}/> : <Copy size={12}/>} {copied ? 'Copied' : 'Copy'}</button></div>
    <pre className="overflow-x-auto p-5 text-[12px] font-mono leading-[1.9]"><SyntaxCode code={children}/></pre>{error&&<p role="status" className="px-5 pb-3 text-xs text-text-muted">Select the code to copy it.</p>}
  </div>;
}

function NoteBox({ children }: { children: React.ReactNode }) {
  return (
    <div className="bg-indigo/10 border border-indigo/20 rounded-xl p-4 text-sm text-indigo mb-4 leading-relaxed">
      {children}
    </div>
  );
}

function WarnBox({ children }: { children: React.ReactNode }) {
  return (
    <div className="bg-yellow/10 border border-yellow/20 rounded-xl p-4 text-sm text-yellow mb-4 leading-relaxed">
      {children}
    </div>
  );
}

function InlineLink({ href, children }: { href: string; children: React.ReactNode }) {
  return (
    <Link href={href} className="text-accent hover:text-accent-hover transition-colors">
      {children}
    </Link>
  );
}

function InlineAnchor({ onClick, children }: { onClick: () => void; children: React.ReactNode }) {
  return (
    <button onClick={onClick} className="text-accent hover:text-accent-hover transition-colors cursor-pointer">
      {children}
    </button>
  );
}

function Table({ headers, rows }: { headers: string[]; rows: React.ReactNode[][] }) {
  return (
    <div className="overflow-x-auto mb-4">
      <table className="w-full text-sm">
        <thead>
          <tr>
            {headers.map((h, i) => (
              <th
                key={i}
                className="text-left text-xs font-semibold uppercase tracking-wider text-text-dim py-2 px-3 border-b border-border"
              >
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, ri) => (
            <tr key={ri} className="hover:bg-bg-card/50 transition-colors">
              {row.map((cell, ci) => (
                <td key={ci} className="py-2 px-3 border-b border-border text-[14px] text-text-muted">
                  {cell}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
