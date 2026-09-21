/**
 * Snippets for one running browser: drive it from the SDK, the CLI, an MCP
 * client, curl or Playwright. Each code template is its own function so the
 * text stays readable as the file a user will paste.
 */
import type { BrowserRow } from '../types';
import { origins } from './origins';
import type { Snippet } from './types';

/** Drives this browser from the TypeScript SDK. */
const sdkCode = (b: BrowserRow, http: string) => (k: string) => `import { Oya } from "@oya-ai/browser";

const oya = new Oya({ apiKey: "${k}", baseUrl: "${http}" });
const browser = await oya.browser.get("${b.id}");   // ${b.name}

await browser.goto("https://example.com");
const page = await browser.analyze();
await browser.click(page.elements[0].id);
console.log(await browser.status());               // health, counters, activity`;

/** Drives this browser from the CLI. */
const cliCode = (b: BrowserRow, http: string) => (k: string) => `export OYA_API_KEY=${k} OYA_BASE_URL=${http}

oya goto https://example.com --id ${b.id}
oya ask "find the pricing page" --id ${b.id}
oya status --id ${b.id}
oya rm ${b.id}`;

/** An MCP server entry named after the browser, so several can sit side by side. */
const mcpCode = (b: BrowserRow, http: string) => (k: string) => `{
  "mcpServers": {
    "${b.name.replace(/[^a-z0-9-]+/gi, '-').toLowerCase() || 'oya-browser'}": {
      "url": "${http}/mcp/${b.id}",
      "headers": { "Authorization": "Bearer ${k}" }
    }
  }
}`;

/** Navigates, reads and stops this browser over the REST API. */
const curlCode = (b: BrowserRow, http: string) => (k: string) => `curl -X POST ${http}/api/browsers/${b.id}/command \\
  -H "Authorization: Bearer ${k}" -H "Content-Type: application/json" \\
  -d '{"action":"navigate","params":{"url":"https://example.com"}}'

curl -X POST ${http}/api/browsers/${b.id}/command \\
  -H "Authorization: Bearer ${k}" -H "Content-Type: application/json" \\
  -d '{"action":"analyze"}'

curl -X POST ${http}/api/browsers/${b.id}/stop -H "Authorization: Bearer ${k}"`;

/** Attaches Playwright to a CDP-backed browser through the gateway. */
const cdpPlaywrightCode = (b: BrowserRow, ws: string) => (k: string) => `import { chromium } from "playwright";

// Attaches to this exact browser through the gateway. Closing your client
// leaves the browser running in the fleet.
const browser = await chromium.connectOverCDP(
  "${ws}/connect?token=${k}&browser=${b.id}",
);
const page = browser.contexts()[0]?.pages()[0] ?? await browser.newPage();
await page.goto("https://example.com");
console.log(await page.title());`;

/** An Oya client has no CDP endpoint: says so, and how to get one that does. */
const oyaPlaywrightCode = (b: BrowserRow, ws: string) => () =>
  `// This browser is an Oya client (${b.provider || 'desktop'}): it is driven over its
// own socket and has no CDP endpoint to attach to. Use the SDK, CLI or MCP above,
// or start a CDP-backed browser (Browserbase, Steel, Anchor, your own Chrome) and
// attach to that with:
//
//   chromium.connectOverCDP("${ws}/connect?token=<key>&browser=<id>")`;

/** The Playwright tab: a working attach for CDP browsers, an explanation for Oya clients. */
function playwrightSnippet(b: BrowserRow, ws: string): Snippet {
  if (b.clientType !== 'cdp')
    return { id: 'playwright', label: 'Playwright', file: 'attach.ts', code: oyaPlaywrightCode(b, ws) };
  const note =
    'Puppeteer: puppeteer.connect({ browserWSEndpoint: <the same URL> }). browser-use and Stagehand take a CDP URL too.';
  return { id: 'playwright', label: 'Playwright', file: 'attach.ts', code: cdpPlaywrightCode(b, ws), note };
}

/** What the MCP tab adds under its code. */
const MCP_NOTE =
  'Claude Desktop, Cursor, Windsurf and Claude Code all take this shape. The pool endpoint /mcp/pool spreads calls across every browser on the key instead.';

/** Snippets for one running browser. Playwright comes second for CDP browsers, last for Oya clients. */
export function browserSnippets(b: BrowserRow): Snippet[] {
  const { http, ws } = origins();
  const out = baseSnippets(b, http);
  out.splice(b.clientType === 'cdp' ? 1 : out.length, 0, playwrightSnippet(b, ws));
  return out;
}

/** The SDK, CLI, MCP and curl tabs, in that order. */
function baseSnippets(b: BrowserRow, http: string): Snippet[] {
  return [
    { id: 'sdk', label: 'TypeScript', file: 'drive.ts', code: sdkCode(b, http) },
    { id: 'cli', label: 'CLI', file: 'terminal', code: cliCode(b, http) },
    { id: 'mcp', label: 'Agents (MCP)', file: 'mcp.json', code: mcpCode(b, http), note: MCP_NOTE },
    { id: 'curl', label: 'curl', file: 'terminal', code: curlCode(b, http) },
  ];
}
