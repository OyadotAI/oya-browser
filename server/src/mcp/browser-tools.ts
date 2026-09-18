/**
 * The browser tools shared by the per-browser and pool MCP servers. Each tool is
 * one command sent to a browser; the servers differ only in which browser that is.
 */

import { z } from 'zod';
import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { sendCommand } from '../modules/browsers/socket.ts';
import { analysis, type Page } from './analysis.ts';
import { DEFAULT_SCROLL_PX, NAVIGATE_TIMEOUT_MS, SCROLL_TIMEOUT_MS } from './constants.ts';
import { fail } from './replies.ts';

/** The keys press_key may press: navigation only, never modifiers or F-keys. */
const KEYS = [
  'Enter',
  'Escape',
  'Tab',
  'ArrowDown',
  'ArrowUp',
  'ArrowLeft',
  'ArrowRight',
  'Backspace',
  'Delete',
  'Space',
  'Home',
  'End',
  'PageUp',
  'PageDown',
];
/** CSS selector for an element by the number analyze gave it. */
const byId = (id: number) => `[data-ac-id="${id}"]`;

/** What a tool hands back: a line of text, an analysis, or a base64 PNG. */
type Reply =
  | string
  | Page
  | {
      /** Base64 PNG to return as an image. */
      image: string;
    };
/**
 * name → description, input schema, the command it sends, and how its result
 * reads back. `text` returns a line, `{ page }` for an analysis, or `{ image }`.
 * `advance` moves the pool on to its next browser (a new page context).
 */
type Tool = {
  /** What the model is told the tool does. */
  description: string;
  /** Zod shape of the tool's arguments. */
  schema: z.ZodRawShape;
  /** Whether a call moves the pool to its next browser. */
  advance?: boolean;
  /** The browser command, params and optional timeout a call sends. */
  command: (args: any) => [action: string, params?: object, timeoutMs?: number];
  /** Turns the browser's result into the reply. */
  text: (data: any, args: any) => Reply;
};

/** Every browser tool, by name. */
const TOOLS: Record<string, Tool> = {
  analyze_page: {
    description: `Analyze the current page. Returns structured markdown with all interactive elements numbered as [#id type "label"].
Use element IDs with click/type tools. The output includes:
- Page metadata (URL, title, viewport, scroll position)
- Full page content as markdown with inline element annotations
- Element index with visibility flags (visible = in viewport without scrolling)`,
    schema: {},
    command: () => ['analyze'],
    text: (data) => analysis(data),
  },
  navigate: {
    description: 'Navigate the browser to a URL.',
    schema: { url: z.string().describe('The URL to navigate to') },
    advance: true,
    command: ({ url }) => ['navigate', { url }, NAVIGATE_TIMEOUT_MS],
    text: (_, { url }) => `Navigated to ${url}`,
  },
  click: {
    description: 'Click an interactive element by its ID number (from analyze_page results).',
    schema: { element_id: z.number().describe('The element ID number from analyze_page') },
    command: ({ element_id }) => ['click', { selector: byId(element_id) }],
    text: (_, { element_id }) => `Clicked element ${element_id}`,
  },
  type: {
    description:
      'Type text into an input element by its ID number (from analyze_page results). Clears existing content first.',
    schema: {
      element_id: z.number().describe('The element ID number from analyze_page'),
      text: z.string().describe('The text to type'),
    },
    command: ({ element_id, text }) => ['type', { selector: byId(element_id), text }],
    text: (_, { element_id, text }) => `Typed "${text}" into element ${element_id}`,
  },
  screenshot: {
    description: 'Capture a screenshot of the visible browser tab as a base64 PNG image.',
    schema: {},
    command: () => ['screenshot'],
    text: (data) =>
      data?.screenshot
        ? { image: data.screenshot.replace(/^data:image\/png;base64,/, '') }
        : 'Screenshot captured but no image data returned',
  },
  press_key: {
    description: `Press a safe navigation key. Allowed: ${KEYS.join(', ')}. Do NOT use for F-keys, Meta, Control, Alt, or Shift.`,
    schema: { key: z.enum(KEYS).describe('Key to press') },
    command: ({ key }) => ['press_key', { key }],
    text: (_, { key }) => `Pressed ${key}`,
  },
  scroll: {
    description: 'Scroll the page up or down.',
    schema: {
      direction: z.enum(['up', 'down']).describe('Scroll direction'),
      amount: z.number().optional().describe('Pixels to scroll (default 500)'),
    },
    command: ({ direction, amount }) => ['scroll', { direction, amount }, SCROLL_TIMEOUT_MS],
    // Scroll may re-analyze the page it lands on.
    text: (data, { direction, amount }) =>
      data?.markdown && data?.elements ? analysis(data) : `Scrolled ${direction} ${amount || DEFAULT_SCROLL_PX}px`,
  },
  wait: {
    description: 'Wait for an element matching a CSS selector to appear on the page.',
    schema: {
      selector: z.string().describe('CSS selector to wait for'),
      timeout: z.number().optional().describe('Max wait time in ms (default 10000)'),
    },
    command: ({ selector, timeout }) => ['wait', { selector, timeout }],
    text: (_, { selector }) => `Element found: ${selector}`,
  },
  read_elements: {
    description:
      'List interactive elements on the page. Lighter than analyze_page — returns element metadata without full page markdown.',
    schema: {
      selector: z.string().optional().describe('CSS selector to scope the search (default: entire page)'),
      limit: z.number().optional().describe('Max elements to return (default 50)'),
    },
    command: ({ selector, limit }) => ['read_page', { selector, limit }],
    text: ({ url, title, elements }) =>
      `Page: ${title} (${url})\n\nElements (${elements.length}):\n` +
      elements
        .map((e) => `${e.tag}${e.id ? '#' + e.id : ''} — ${e.text || e.aria_label || e.placeholder || '(no text)'}`)
        .join('\n'),
  },
  click_coordinates: {
    description:
      'Click at specific x,y pixel coordinates on the page. Use when you know the exact position (e.g. from a screenshot).',
    schema: {
      x: z.number().describe('X coordinate in pixels from the left edge'),
      y: z.number().describe('Y coordinate in pixels from the top edge'),
    },
    command: ({ x, y }) => ['click_coordinates', { x, y }],
    text: (_, { x, y }) => `Clicked at (${x}, ${y})`,
  },
  mouse_move: {
    description:
      'Move the mouse cursor to specific x,y coordinates without clicking. Useful for triggering hover states, tooltips, or dropdown menus.',
    schema: {
      x: z.number().describe('X coordinate in pixels'),
      y: z.number().describe('Y coordinate in pixels'),
    },
    command: ({ x, y }) => ['mouse_move', { x, y }],
    text: (_, { x, y }) => `Mouse moved to (${x}, ${y})`,
  },
  double_click: {
    description: 'Double-click an element by ID or at specific x,y coordinates.',
    schema: {
      element_id: z.number().optional().describe('Element ID to double-click (from analyze_page)'),
      x: z.number().optional().describe('X coordinate (used if no element_id)'),
      y: z.number().optional().describe('Y coordinate (used if no element_id)'),
    },
    command: ({ element_id, x, y }) => [
      'double_click',
      element_id !== undefined ? { selector: byId(element_id) } : { x, y },
    ],
    text: (_, { element_id, x, y }) =>
      element_id !== undefined ? `Double-clicked element ${element_id}` : `Double-clicked at (${x}, ${y})`,
  },
  keyboard_type: {
    description:
      'Type text using the keyboard without targeting a specific element. Types into whatever is currently focused.',
    schema: { text: z.string().describe('Text to type') },
    command: ({ text }) => ['keyboard_type', { text }],
    text: (_, { text }) => `Typed "${text}"`,
  },
  drag: {
    description: 'Drag from one point to another. Useful for sliders, drag-and-drop, or selecting text.',
    schema: {
      from_x: z.number().describe('Start X coordinate'),
      from_y: z.number().describe('Start Y coordinate'),
      to_x: z.number().describe('End X coordinate'),
      to_y: z.number().describe('End Y coordinate'),
    },
    command: (p) => ['drag', p],
    text: (_, { from_x, from_y, to_x, to_y }) => `Dragged from (${from_x}, ${from_y}) to (${to_x}, ${to_y})`,
  },
  list_tabs: {
    description: 'List all open tabs in the browser. Returns tab ID, title, URL, and which is active.',
    schema: {},
    command: () => ['list_tabs'],
    text: (data) =>
      `Tabs:\n${(data.tabs || []).map((t) => `${t.active ? '→ ' : '  '}[tab ${t.id}] ${t.title} — ${t.url}`).join('\n')}`,
  },
  open_tab: {
    description: 'Open a new browser tab, optionally navigating to a URL.',
    schema: { url: z.string().optional().describe('URL to open (default: blank tab)') },
    command: ({ url }) => ['open_tab', { url }, NAVIGATE_TIMEOUT_MS],
    text: (data, { url }) => `Opened tab ${data.tab_id}${url ? ' at ' + url : ''}`,
  },
  switch_tab: {
    description: 'Switch to a different browser tab by its tab ID (from list_tabs).',
    schema: { tab_id: z.number().describe('The tab ID to switch to') },
    command: ({ tab_id }) => ['switch_tab', { tab_id }],
    text: (_, { tab_id }) => `Switched to tab ${tab_id}`,
  },
  handle_dialog: {
    description:
      'Answer a native browser dialog (confirm or prompt) blocking the page. Alerts are answered automatically.',
    schema: {
      accept: z.boolean().describe('true clicks OK, false clicks Cancel'),
      prompt_text: z.string().optional().describe('Text to enter, for a prompt dialog only'),
    },
    command: ({ accept, prompt_text }) => ['handle_dialog', { accept, prompt_text }],
    text: (data) => `${data?.accepted === false ? 'Dismissed' : 'Accepted'} the ${data?.type || ''} dialog`,
  },
  close_tab: {
    description: 'Close a browser tab. Closes active tab if no tab_id specified.',
    schema: { tab_id: z.number().optional().describe('Tab ID to close (default: active tab)') },
    command: ({ tab_id }) => ['close_tab', { tab_id }],
    text: () => 'Closed tab',
  },
};

/** How registerBrowserTools routes calls and labels replies. */
type Options = {
  /** The browser a call goes to, or null when there is none. */
  pick: (advance: boolean) => string | null | undefined;
  /** Prefix for every text reply naming the browser that answered. */
  tag?: (id: string) => string;
  /** Tool names to register; all by default. */
  only?: string[];
  /** Error text when pick finds no browser. */
  noBrowser?: string;
};

/**
 * Register the browser tools on `server`. `pick(advance)` names the browser a
 * call goes to, or null when there is none; `tag(id)` prefixes every text reply
 * (the pool says which of its browsers answered). `only` limits the set.
 */
export function registerBrowserTools(
  server: McpServer,
  { pick, tag = () => '', only = Object.keys(TOOLS), noBrowser = 'no browser is connected' }: Options,
) {
  for (const name of only) {
    const tool = TOOLS[name];
    server.tool(name, tool.description, tool.schema, (args) => runTool(tool, args, { pick, tag, noBrowser }));
  }
}

/** Sends the tool's command to the picked browser and turns the result into a reply. */
async function runTool(tool: Tool, args, { pick, tag, noBrowser }: Required<Omit<Options, 'only'>>) {
  const id = pick(!!tool.advance);
  if (!id) return fail(noBrowser);
  const [action, params, timeout] = tool.command(args);
  const result = await sendCommand(id, action, params, timeout);
  if (!result.ok) return fail(result.error);
  return reply(tool.text(result.data, args), () => tag(id));
}

/** An image reply, or text prefixed with the answering browser's tag. */
function reply(out: Reply, prefix: () => string) {
  if (typeof out === 'object' && 'image' in out)
    return { content: [{ type: 'image' as const, data: out.image, mimeType: 'image/png' }] };
  const tag = prefix();
  const text = typeof out === 'object' ? (tag ? `${tag}\n` : '') + out.page : (tag ? `${tag} ` : '') + out;
  return { content: [{ type: 'text' as const, text }] };
}
