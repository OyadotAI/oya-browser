/**
 * The browser card hosts with MCP Apps (Claude, ChatGPT) show for start_browser: a snapshot
 * of the browser that refreshes while it is on screen, and a link to take over in the live
 * page. The card holds only the browser's id. Its snapshots come through the host's own MCP
 * connection, so no token ever reaches the card's frame, and a key sees only its own browsers.
 */
import { readFileSync } from 'node:fs';
import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { registry } from '../modules/browsers/registry.ts';
import { QUIET, sendCommand } from '../modules/browsers/socket.ts';
import { consoleUrl } from '../modules/slack/service.ts';
import { toolConfig } from './annotations.ts';
import { CARD_MIME, CARD_QUALITY, CARD_TIMEOUT_MS, CARD_URI } from './constants.ts';

/** The card's page, read once: it is static and carries no data of its own. */
const CARD_HTML = readFileSync(new URL('./browser-card.html', import.meta.url), 'utf8');

/** A screenshot command's answer. */
interface Shot {
  /** A data URL, or a bare base64 payload. */
  screenshot?: unknown;
}

/** An error that may carry the API's reason code. */
interface Coded {
  /** The reason, such as control_paused. */
  code?: string;
}

/** Tools only the card calls: hosts hide them from the model, so the docs and llms.txt leave them out. */
export const APP_ONLY_TOOLS = ['view_browser'];

/** What start_browser declares so an MCP Apps host renders the card beside its result. */
export const CARD_META = { ui: { resourceUri: CARD_URI } };

/** The live page where a person watches the browser and takes control. */
export const liveUrl = (id: string) => `${consoleUrl()}/live/${id}`;

/** What the card is told about the browser it shows: no token, only what it names. */
export const cardContent = (id: string, name?: string) => ({ browserId: id, name: name || '', liveUrl: liveUrl(id) });

/** What view_browser is for; the model never sees it (an app-only tool). */
const VIEW_DESCRIPTION = 'A fresh snapshot of one of your browsers, for the browser card.';

/** Registers the card's page and the snapshot tool only the card may call. */
export function registerBrowserCard(server: McpServer, apiKey: string) {
  const page = { mimeType: CARD_MIME, description: 'The in-chat browser card' };
  server.registerResource('browser-card', CARD_URI, page, async () => ({
    contents: [{ uri: CARD_URI, mimeType: CARD_MIME, text: CARD_HTML, _meta: { ui: { prefersBorder: true } } }],
  }));
  const config = toolConfig('view_browser', VIEW_DESCRIPTION, { browser_id: z.string() });
  const appOnly = { ...config, _meta: { ui: { visibility: ['app'] } } };
  server.registerTool('view_browser', appOnly, async ({ browser_id }) => viewBrowser(apiKey, browser_id));
}

/** The card's answer: its state and, when live, the picture; the text is for a host that shows it. */
const view = (card: Record<string, unknown>) => ({
  content: [{ type: 'text' as const, text: `Browser card: ${card.state}` }],
  structuredContent: card,
});

/**
 * One snapshot of `id` for the card. Another key's browser, or one not connected, is
 * `unavailable`, the same answer, so the card cannot tell them apart or probe for ids. A
 * browser a person has taken over is `held`: the control fence refuses agent commands then.
 */
export async function viewBrowser(apiKey: string, id: string) {
  if (!registry.belongsTo(id, apiKey)) return view({ state: 'unavailable' });
  try {
    const params = { format: 'jpeg', quality: CARD_QUALITY, [QUIET]: true };
    const shot = await sendCommand(id, 'screenshot', params, CARD_TIMEOUT_MS);
    return shot.ok ? live(id, (shot.data as Shot)?.screenshot) : view({ state: 'failed' });
  } catch (e) {
    return view({ state: (e as Coded)?.code === 'control_paused' ? 'held' : 'failed' });
  }
}

/** A live snapshot with the browser's name and address; a bare base64 payload is taken to be PNG. */
function live(id: string, screenshot: unknown) {
  if (typeof screenshot !== 'string' || !screenshot) return view({ state: 'failed' });
  const image = screenshot.startsWith('data:image/') ? screenshot : `data:image/png;base64,${screenshot}`;
  const browser = registry.get(id);
  return view({ state: 'live', image, name: browser?.name || '', url: browser?.currentUrl || '' });
}
