/**
 * Every number and fixed string the MCP servers run on, by name.
 */

/** Version the MCP servers report. */
export const MCP_VERSION = '1.0.0';
/** The Authorization scheme a caller's key arrives under. */
export const BEARER = 'Bearer ';
/**
 * JSON-RPC methods the pool endpoint answers without a key: registries and
 * clients read the tool list before signing up. None of them reaches a browser
 * or needs a key; tool calls, resources and streams stay behind sign-in.
 */
export const PUBLIC_METHODS = new Set(['initialize', 'notifications/initialized', 'ping', 'tools/list']);
/** Navigation waits on slow sites. */
export const NAVIGATE_TIMEOUT_MS = 90_000;
/** A scroll that re-analyzes the page it lands on needs longer than a plain command. */
export const SCROLL_TIMEOUT_MS = 15_000;
/** What a scroll moves when no amount is given, as the browser defaults it. */
export const DEFAULT_SCROLL_PX = 500;
/** How long start_browser waits for a cloud browser to dial in. */
export const START_WAIT_MS = 120_000;
/** How often start_browser checks whether it has. */
export const START_POLL_MS = 1000;
/** A lifecycle call through the public API gives up after this long. */
export const SELF_API_TIMEOUT_MS = 150_000;
/** Longest browser name start_browser accepts. */
export const MAX_NAME = 100;
/** Indentation of the pool-status resource's JSON. */
export const JSON_INDENT = 2;

/** The in-chat browser card's address: MCP Apps hosts render ui:// resources in a sandboxed frame. */
export const CARD_URI = 'ui://oya/browser-card.html';
/** The media type MCP Apps hosts render as an app (MCP Apps spec 2026-01-26). */
export const CARD_MIME = 'text/html;profile=mcp-app';
/** JPEG quality of the card's snapshots: a small picture every 2 seconds, not a full screenshot. */
export const CARD_QUALITY = 55;
/** How long one card snapshot may take before the card counts it as failed. */
export const CARD_TIMEOUT_MS = 10_000;
