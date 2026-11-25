/**
 * Chat presentation for a completed browser turn. The card refreshes via the
 * authenticated host and requests a browser-scoped viewing link on click.
 */
import { readFileSync } from 'node:fs';
import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { ownsCloudBrowser } from '../modules/browsers/pool.ts';
import { registry } from '../modules/browsers/registry.ts';
import { QUIET, sendCommand } from '../modules/browsers/socket.ts';
import { consoleUrl } from '../modules/slack/service.ts';
import { pendingBrowser } from './pool-state.ts';
import { toolConfig } from './annotations.ts';
import { selfApi, type Self } from './self-api.ts';
import { fail, text } from './replies.ts';
import { CARD_MIME, CARD_QUALITY, CARD_TIMEOUT_MS, CARD_URI, SCREENSHOT_URI } from './constants.ts';

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
export const APP_ONLY_TOOLS = ['view_browser', 'watch_browser'];

/** What show_browser declares so an MCP Apps host renders the card beside its result. */
export const CARD_META = { ui: { resourceUri: CARD_URI } };

/** The live page where a person watches the browser and takes control. */
export const liveUrl = (id: string) => `${consoleUrl()}/live/${encodeURIComponent(id)}`;

/** What the card is told about the browser it shows: no token, only what it names. */
export const cardContent = (id: string, name?: string) => ({ browserId: id, name: name || '' });

/** These identifiers are sent back to API routes as a single path segment. */
const BROWSER_ID = { browser_id: z.string().regex(/^[a-zA-Z0-9_-]+$/) };

/** What view_browser is for; the model never sees it (an app-only tool). */
const VIEW_DESCRIPTION = 'A fresh snapshot of one of your browsers, for the browser card.';

/** Registers the two static views with no session data in their HTML. */
function registerPage(server: McpServer, name: string, uri: string, html: string) {
  server.registerResource(name, uri, { mimeType: CARD_MIME }, async () => ({
    contents: [{ uri, mimeType: CARD_MIME, text: html, _meta: { ui: { prefersBorder: true } } }],
  }));
}

/** A fixed capture belongs to this tool call, even after the page moves. */
const SCREENSHOT_HTML = readFileSync(new URL('./screenshot.html', import.meta.url), 'utf8');

/** A card is bound to the browser named in the turn, independent of subsequent pool selection. */
function showBrowser(key: string, id: string) {
  if (pendingBrowser(key) === id && !registry.isConnected(id))
    return { ...text('Cloud browser is starting.'), structuredContent: { ...cardContent(id), pending: true } };
  if (!ownsCloudBrowser(key, id)) return fail('Browser unavailable. The session may have ended.');
  return { ...text('Live browser view.'), structuredContent: cardContent(id, registry.get(id)?.name) };
}

/** Returns the short-lived URL only to the component that requested it. */
async function watchBrowser(self: Self, id: string) {
  try {
    const { token } = await selfApi(self, `/control/sessions/${encodeURIComponent(id)}/watch`, {});
    return { content: [], _meta: { watchUrl: `${liveUrl(id)}#t=${encodeURIComponent(token)}` } };
  } catch {
    return fail('Couldn’t open live view.');
  }
}

/**
 * Registers presentation on the pool only, where the host has the authenticated session context.
 * A chat connector leaves show_browser out (`withShow` false): its card opens beside open_tab instead,
 * so the model has no step to remember after each turn.
 */
export function registerBrowserCard(server: McpServer, apiKey: string, self: Self = {}, withShow = true) {
  registerPage(server, 'browser-card', CARD_URI, CARD_HTML);
  registerPage(server, 'screenshot', SCREENSHOT_URI, SCREENSHOT_HTML);
  if (withShow) registerShow(server, apiKey);
  registerCardActions(server, apiKey, self, BROWSER_ID);
}

/** show_browser: the tool an API-key client's model calls to put the card in the chat. */
function registerShow(server: McpServer, apiKey: string) {
  const show = toolConfig('show_browser', 'Show this browser once at the end of each browsing turn.', BROWSER_ID);
  server.registerTool('show_browser', { ...show, _meta: CARD_META }, async ({ browser_id }) =>
    showBrowser(apiKey, browser_id),
  );
}

/** These tools serve the component's refresh loop and its explicit Watch live action. */
function registerCardActions(server: McpServer, apiKey: string, self: Self, schema: typeof BROWSER_ID) {
  const hidden = { ui: { visibility: ['app'] } };
  const view = toolConfig('view_browser', VIEW_DESCRIPTION, schema);
  server.registerTool('view_browser', { ...view, _meta: hidden }, ({ browser_id }) => viewBrowser(apiKey, browser_id));
  const watch = toolConfig('watch_browser', 'Open a temporary watch-only view.', schema);
  server.registerTool('watch_browser', { ...watch, _meta: hidden }, ({ browser_id }) => watchBrowser(self, browser_id));
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
  if (!ownsCloudBrowser(apiKey, id)) return view({ state: 'unavailable' });
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
