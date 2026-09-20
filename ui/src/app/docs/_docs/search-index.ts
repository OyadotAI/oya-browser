/**
 * The docs search: a hand-kept index of headings and key sentences, each
 * pointing at the section it lives in, and the substring search over it.
 */
import { DEDUPE_PREFIX_LENGTH, MAX_HITS, MIN_ENTRY_LENGTH, MIN_QUERY_LENGTH, SNIPPET_CONTEXT } from './constants';

/** One searchable line. */
interface SearchIndexEntry {
  /** The text matched against. */
  text: string;
  /** The section it jumps to ('' for the top of the page). */
  id: string;
  /** The element it came from (H2, P, LI…). */
  tag: string;
}

/** One result. */
export interface SearchHit {
  /** The section it jumps to. */
  id: string;
  /** The match with some context around it. */
  snippet: string;
  /** The element it came from. */
  tag: string;
}

/** [text, section id, tag] for every searchable line. */
const ITEMS: [string, string, string][] = [
  ['Documentation', '', 'H1'],
  ['The browser control plane sitting between AI agents and execution runners.', '', 'P'],
  ['Control Plane Architecture', 'control-plane', 'H2'],
  [
    'The control plane model vs raw browser runners like Browserbase, Steel, Anchor, Browser Use.',
    'control-plane',
    'P',
  ],
  ['Why Oya — 10x Comparison', 'comparison', 'H2'],
  ['Detailed comparison vs single-vendor runners: Browserbase, Steel, Anchor, Browser Use.', 'comparison', 'P'],
  ['Multi-Provider Routing & Failover', 'routing-failover', 'H2'],
  [
    'Configuring provider priorities, session capacities, and zero-rewrite automatic failover.',
    'routing-failover',
    'P',
  ],
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
  ["Save your key somewhere safe. If you lose it, you'll need to generate a new one.", 'create-key', 'P'],
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
  [
    'Analyzes the current page. Returns the full page, as markdown (default), TOON or JSONL, with every interactive element numbered.',
    'analyze_page',
    'P',
  ],
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

/** The index, without entries too short to be worth matching. */
const SEARCH_INDEX: SearchIndexEntry[] = ITEMS.filter(([text]) => text.length >= MIN_ENTRY_LENGTH).map(
  ([text, id, tag]) => ({ text, id, tag }),
);

/** The match at `pos` with context either side, ellipsized where cut. */
function snippetOf(text: string, pos: number, length: number): string {
  const start = Math.max(0, pos - SNIPPET_CONTEXT);
  const end = Math.min(text.length, pos + length + SNIPPET_CONTEXT);
  return (start > 0 ? '...' : '') + text.slice(start, end) + (end < text.length ? '...' : '');
}

/** The hit for one entry, or null when it does not match or repeats one already found. */
function hitFor(entry: SearchIndexEntry, query: string, seen: Set<string>): SearchHit | null {
  const pos = entry.text.toLowerCase().indexOf(query);
  const key = entry.id + '|' + entry.text.slice(0, DEDUPE_PREFIX_LENGTH);
  if (pos === -1 || seen.has(key)) return null;
  seen.add(key);
  return { id: entry.id, snippet: snippetOf(entry.text, pos, query.length), tag: entry.tag };
}

/** Matches in index order, at most MAX_HITS. */
function collectHits(query: string): SearchHit[] {
  const hits: SearchHit[] = [];
  const seen = new Set<string>();
  for (const entry of SEARCH_INDEX) {
    const hit = hitFor(entry, query, seen);
    if (hit && hits.push(hit) >= MAX_HITS) break;
  }
  return hits;
}

/** Results for a query; null when the query is too short to show any. */
export function searchDocs(q: string): SearchHit[] | null {
  const query = q.trim().toLowerCase();
  if (!query || query.length < MIN_QUERY_LENGTH) return null;
  return collectHits(query);
}
