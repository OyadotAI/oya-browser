/**
 * Snippets for the fleet: how to make browsers appear in the console, from
 * the SDK, the CLI, Playwright, an MCP client or curl.
 */
import { origins } from './origins';
import type { Snippet } from './types';

/** Starts a browser from the TypeScript SDK. */
const sdkCode = (http: string) => (k: string) => `import { Oya } from "@oya-ai/browser";

const oya = new Oya({ apiKey: "${k}", baseUrl: "${http}" });

// The provider comes from this key's settings. persona: "auto" rotates
// across your personas; captcha: "auto" clears them as they appear.
const browser = await oya.browser.start({ persona: "auto", captcha: "auto" });
await browser.goto("https://example.com");
await browser.stop();`;

/** Installs the CLI and runs a browser from it. */
const cliCode = (http: string) => (k: string) => `npm i -g @oya-ai/cli
oya login --url ${http} --key ${k}

oya start --persona auto --name checkout-worker
oya goto https://example.com
oya ls
oya rm --all`;

/** Opens a fresh gateway session from Playwright. */
const playwrightCode = (ws: string) => (k: string) => `import { chromium } from "playwright";

// A fresh browser from whichever provider the gateway routes to.
// Add &profile=<name> to keep cookies, &record=1 to record the session.
const browser = await chromium.connectOverCDP("${ws}/connect?token=${k}");
const page = await browser.newPage();
await page.goto("https://example.com");
await browser.close();`;

/** The pool MCP server entry. */
const mcpCode = (http: string) => (k: string) => `{
  "mcpServers": {
    "oya-browser-pool": {
      "url": "${http}/mcp/pool",
      "headers": { "Authorization": "Bearer ${k}" }
    }
  }
}`;

/** Starts and lists browsers over the REST API. */
const curlCode = (http: string) => (k: string) => `curl -X POST ${http}/api/browsers/start \\
  -H "Authorization: Bearer ${k}" -H "Content-Type: application/json" \\
  -d '{"persona":"auto","name":"worker"}'

curl ${http}/api/browsers -H "Authorization: Bearer ${k}"
curl ${http}/api/fleet    -H "Authorization: Bearer ${k}"`;

/** What the MCP tab adds under its code. */
const MCP_NOTE = 'The pool hands each call to the next healthy browser on this key. One browser: /mcp/<id>.';

/** Snippets for the fleet: how to make browsers appear here. */
export function fleetSnippets(): Snippet[] {
  const { http, ws } = origins();
  return [
    { id: 'sdk', label: 'TypeScript', file: 'start.ts', code: sdkCode(http) },
    { id: 'cli', label: 'CLI', file: 'terminal', code: cliCode(http) },
    { id: 'playwright', label: 'Playwright', file: 'new-session.ts', code: playwrightCode(ws) },
    { id: 'mcp', label: 'Agents (MCP)', file: 'mcp.json', code: mcpCode(http), note: MCP_NOTE },
    { id: 'curl', label: 'curl', file: 'terminal', code: curlCode(http) },
  ];
}
