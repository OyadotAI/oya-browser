/**
 * Per-browser MCP server factory — exposes browser tools via Streamable HTTP.
 */

import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StreamableHTTPServerTransport } from '@modelcontextprotocol/sdk/server/streamableHttp.js';
import { z } from 'zod';
import { registry } from './connection-registry.js';
import { sendCommand } from './ws-handler.js';
import { isFleetToken, validateApiKey, authenticateToken } from './auth.js';
import { nextBrowser, poolStats } from './pool.js';

/** Pool pinned browser state: apiKey → browserId */
const poolPinned = new Map();

/**
 * Create an MCP server for a browser session.
 */
function createMcpServer(browserId) {
  const browser = registry.get(browserId);
  const serverName = browser ? `Oya Browser — ${browser.name}` : `Oya Browser — ${browserId}`;

  const server = new McpServer({
    name: serverName,
    version: '1.0.0',
  });

  // ── Tools ──

  server.tool(
    'analyze_page',
    `Analyze the current page. Returns structured markdown with all interactive elements numbered as [#id type "label"].
Use element IDs with click/type tools. The output includes:
- Page metadata (URL, title, viewport, scroll position)
- Full page content as markdown with inline element annotations
- Element index with visibility flags (visible = in viewport without scrolling)`,
    {},
    async () => {
      const result = await sendCommand(browserId, 'analyze');
      if (!result.ok) {
        return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      }
      const { markdown, elements, scroll, viewport, truncated } = result.data;

      // Build a compact element index grouped by visibility
      const visible = elements.filter((e) => e.visible);
      const offscreen = elements.filter((e) => !e.visible);

      let index = `\n\n## Element Index (${elements.length} total, ${visible.length} visible)\n\n`;

      if (visible.length > 0) {
        index += '### Visible\n';
        index += visible.map((e) => {
          let line = `  [#${e.id}] ${e.type}`;
          if (e.text) line += `: ${e.text}`;
          if (e.href) line += ` → ${e.href}`;
          if (e.value) line += ` value="${e.value}"`;
          if (e.checked) line += ' ✓';
          if (e.disabled) line += ' (disabled)';
          return line;
        }).join('\n');
        index += '\n';
      }

      if (offscreen.length > 0) {
        index += '\n### Off-screen (scroll to reveal)\n';
        index += offscreen.map((e) => {
          let line = `  [#${e.id}] ${e.type}`;
          if (e.text) line += `: ${e.text}`;
          if (e.disabled) line += ' (disabled)';
          return line;
        }).join('\n');
        index += '\n';
      }

      if (truncated) {
        index += '\n⚠ Page content was truncated (very long page). Scroll down and re-analyze to see more.\n';
      }

      return {
        content: [{
          type: 'text',
          text: markdown + index,
        }],
      };
    }
  );

  server.tool(
    'navigate',
    'Navigate the browser to a URL.',
    { url: z.string().describe('The URL to navigate to') },
    async ({ url }) => {
      const result = await sendCommand(browserId, 'navigate', { url }, 90000);
      if (!result.ok) {
        return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      }
      return { content: [{ type: 'text', text: `Navigated to ${url}` }] };
    }
  );

  server.tool(
    'click',
    'Click an interactive element by its ID number (from analyze_page results).',
    { element_id: z.number().describe('The element ID number from analyze_page') },
    async ({ element_id }) => {
      const selector = `[data-ac-id="${element_id}"]`;
      const result = await sendCommand(browserId, 'click', { selector });
      if (!result.ok) {
        return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      }
      return { content: [{ type: 'text', text: `Clicked element ${element_id}` }] };
    }
  );

  server.tool(
    'type',
    'Type text into an input element by its ID number (from analyze_page results). Clears existing content first.',
    {
      element_id: z.number().describe('The element ID number from analyze_page'),
      text: z.string().describe('The text to type'),
    },
    async ({ element_id, text }) => {
      const selector = `[data-ac-id="${element_id}"]`;
      const result = await sendCommand(browserId, 'type', { selector, text });
      if (!result.ok) {
        return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      }
      return { content: [{ type: 'text', text: `Typed "${text}" into element ${element_id}` }] };
    }
  );

  server.tool(
    'screenshot',
    'Capture a screenshot of the visible browser tab as a base64 PNG image.',
    {},
    async () => {
      const result = await sendCommand(browserId, 'screenshot');
      if (!result.ok) {
        return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      }
      if (result.data?.screenshot) {
        // Strip data:image/png;base64, prefix if present
        const base64 = result.data.screenshot.replace(/^data:image\/png;base64,/, '');
        return {
          content: [{
            type: 'image',
            data: base64,
            mimeType: 'image/png',
          }],
        };
      }
      return { content: [{ type: 'text', text: 'Screenshot captured but no image data returned' }] };
    }
  );

  server.tool(
    'press_key',
    'Press a safe navigation key. Allowed: Enter, Escape, Tab, ArrowDown, ArrowUp, ArrowLeft, ArrowRight, Backspace, Delete, Space, Home, End, PageUp, PageDown. Do NOT use for F-keys, Meta, Control, Alt, or Shift.',
    { key: z.enum(['Enter', 'Escape', 'Tab', 'ArrowDown', 'ArrowUp', 'ArrowLeft', 'ArrowRight', 'Backspace', 'Delete', 'Space', 'Home', 'End', 'PageUp', 'PageDown']).describe('Key to press') },
    async ({ key }) => {
      const result = await sendCommand(browserId, 'press_key', { key });
      if (!result.ok) {
        return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      }
      return { content: [{ type: 'text', text: `Pressed ${key}` }] };
    }
  );

  server.tool(
    'scroll',
    'Scroll the page up or down.',
    {
      direction: z.enum(['up', 'down']).describe('Scroll direction'),
      amount: z.number().optional().describe('Pixels to scroll (default 500)'),
    },
    async ({ direction, amount }) => {
      const result = await sendCommand(browserId, 'scroll', { direction, amount }, 15000);
      if (!result.ok) {
        return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      }

      // If scroll returned an analysis (auto-analyze after scroll), format it
      if (result.data?.markdown && result.data?.elements) {
        const { markdown, elements, truncated } = result.data;
        const visible = elements.filter((e) => e.visible);
        const offscreen = elements.filter((e) => !e.visible);

        let index = `\n\n## Element Index (${elements.length} total, ${visible.length} visible)\n\n`;
        if (visible.length > 0) {
          index += '### Visible\n';
          index += visible.map((e) => {
            let line = `  [#${e.id}] ${e.type}`;
            if (e.text) line += `: ${e.text}`;
            if (e.href) line += ` → ${e.href}`;
            if (e.value) line += ` value="${e.value}"`;
            if (e.checked) line += ' ✓';
            if (e.disabled) line += ' (disabled)';
            return line;
          }).join('\n');
          index += '\n';
        }
        if (offscreen.length > 0) {
          index += '\n### Off-screen (scroll to reveal)\n';
          index += offscreen.map((e) => {
            let line = `  [#${e.id}] ${e.type}`;
            if (e.text) line += `: ${e.text}`;
            if (e.disabled) line += ' (disabled)';
            return line;
          }).join('\n');
          index += '\n';
        }
        return { content: [{ type: 'text', text: markdown + index }] };
      }

      return { content: [{ type: 'text', text: `Scrolled ${direction} ${amount || 500}px` }] };
    }
  );

  server.tool(
    'wait',
    'Wait for an element matching a CSS selector to appear on the page.',
    {
      selector: z.string().describe('CSS selector to wait for'),
      timeout: z.number().optional().describe('Max wait time in ms (default 10000)'),
    },
    async ({ selector, timeout }) => {
      const result = await sendCommand(browserId, 'wait', { selector, timeout });
      if (!result.ok) {
        return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      }
      return { content: [{ type: 'text', text: `Element found: ${selector}` }] };
    }
  );

  server.tool(
    'read_elements',
    'List interactive elements on the page. Lighter than analyze_page — returns element metadata without full page markdown.',
    {
      selector: z.string().optional().describe('CSS selector to scope the search (default: entire page)'),
      limit: z.number().optional().describe('Max elements to return (default 50)'),
    },
    async ({ selector, limit }) => {
      const result = await sendCommand(browserId, 'read_page', { selector, limit });
      if (!result.ok) {
        return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      }
      const { url, title, elements } = result.data;
      const summary = elements
        .map((e) => `${e.tag}${e.id ? '#' + e.id : ''} — ${e.text || e.aria_label || e.placeholder || '(no text)'}`)
        .join('\n');
      return {
        content: [{
          type: 'text',
          text: `Page: ${title} (${url})\n\nElements (${elements.length}):\n${summary}`,
        }],
      };
    }
  );

  // ── Mouse & Keyboard Tools ──

  server.tool(
    'click_coordinates',
    'Click at specific x,y pixel coordinates on the page. Use when you know the exact position (e.g. from a screenshot).',
    {
      x: z.number().describe('X coordinate in pixels from the left edge'),
      y: z.number().describe('Y coordinate in pixels from the top edge'),
    },
    async ({ x, y }) => {
      const result = await sendCommand(browserId, 'click_coordinates', { x, y });
      if (!result.ok) {
        return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      }
      return { content: [{ type: 'text', text: `Clicked at (${x}, ${y})` }] };
    }
  );

  server.tool(
    'mouse_move',
    'Move the mouse cursor to specific x,y coordinates without clicking. Useful for triggering hover states, tooltips, or dropdown menus.',
    {
      x: z.number().describe('X coordinate in pixels'),
      y: z.number().describe('Y coordinate in pixels'),
    },
    async ({ x, y }) => {
      const result = await sendCommand(browserId, 'mouse_move', { x, y });
      if (!result.ok) {
        return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      }
      return { content: [{ type: 'text', text: `Mouse moved to (${x}, ${y})` }] };
    }
  );

  server.tool(
    'double_click',
    'Double-click an element by ID or at specific x,y coordinates.',
    {
      element_id: z.number().optional().describe('Element ID to double-click (from analyze_page)'),
      x: z.number().optional().describe('X coordinate (used if no element_id)'),
      y: z.number().optional().describe('Y coordinate (used if no element_id)'),
    },
    async ({ element_id, x, y }) => {
      const params = {};
      if (element_id !== undefined) {
        params.selector = `[data-ac-id="${element_id}"]`;
      } else {
        params.x = x;
        params.y = y;
      }
      const result = await sendCommand(browserId, 'double_click', params);
      if (!result.ok) {
        return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      }
      return { content: [{ type: 'text', text: element_id ? `Double-clicked element ${element_id}` : `Double-clicked at (${x}, ${y})` }] };
    }
  );

  server.tool(
    'keyboard_type',
    'Type text using the keyboard without targeting a specific element. Types into whatever is currently focused.',
    {
      text: z.string().describe('Text to type'),
    },
    async ({ text }) => {
      const result = await sendCommand(browserId, 'keyboard_type', { text });
      if (!result.ok) {
        return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      }
      return { content: [{ type: 'text', text: `Typed "${text}"` }] };
    }
  );

  server.tool(
    'drag',
    'Drag from one point to another. Useful for sliders, drag-and-drop, or selecting text.',
    {
      from_x: z.number().describe('Start X coordinate'),
      from_y: z.number().describe('Start Y coordinate'),
      to_x: z.number().describe('End X coordinate'),
      to_y: z.number().describe('End Y coordinate'),
    },
    async ({ from_x, from_y, to_x, to_y }) => {
      const result = await sendCommand(browserId, 'drag', { from_x, from_y, to_x, to_y });
      if (!result.ok) {
        return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      }
      return { content: [{ type: 'text', text: `Dragged from (${from_x}, ${from_y}) to (${to_x}, ${to_y})` }] };
    }
  );

  // ── Tab Management Tools ──

  server.tool(
    'list_tabs',
    'List all open tabs in the browser. Returns tab ID, title, URL, and which is active.',
    {},
    async () => {
      const result = await sendCommand(browserId, 'list_tabs');
      if (!result.ok) {
        return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      }
      const tabList = (result.data.tabs || [])
        .map(t => `${t.active ? '→ ' : '  '}[tab ${t.id}] ${t.title} — ${t.url}`)
        .join('\n');
      return { content: [{ type: 'text', text: `Tabs:\n${tabList}` }] };
    }
  );

  server.tool(
    'open_tab',
    'Open a new browser tab, optionally navigating to a URL.',
    { url: z.string().optional().describe('URL to open (default: blank tab)') },
    async ({ url }) => {
      const result = await sendCommand(browserId, 'open_tab', { url }, 90000);
      if (!result.ok) {
        return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      }
      return { content: [{ type: 'text', text: `Opened tab ${result.data.tab_id}${url ? ' at ' + url : ''}` }] };
    }
  );

  server.tool(
    'switch_tab',
    'Switch to a different browser tab by its tab ID (from list_tabs).',
    { tab_id: z.number().describe('The tab ID to switch to') },
    async ({ tab_id }) => {
      const result = await sendCommand(browserId, 'switch_tab', { tab_id });
      if (!result.ok) {
        return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      }
      return { content: [{ type: 'text', text: `Switched to tab ${tab_id}` }] };
    }
  );

  server.tool(
    'handle_dialog',
    'Answer a native browser dialog (confirm or prompt) blocking the page. Alerts are answered automatically.',
    {
      accept: z.boolean().describe('true clicks OK, false clicks Cancel'),
      prompt_text: z.string().optional().describe('Text to enter, for a prompt dialog only'),
    },
    async ({ accept, prompt_text }) => {
      const result = await sendCommand(browserId, 'handle_dialog', { accept, prompt_text });
      if (!result.ok) {
        return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      }
      return { content: [{ type: 'text', text: `${result.data?.accepted === false ? 'Dismissed' : 'Accepted'} the ${result.data?.type || ''} dialog` }] };
    }
  );

  server.tool(
    'close_tab',
    'Close a browser tab. Closes active tab if no tab_id specified.',
    { tab_id: z.number().optional().describe('Tab ID to close (default: active tab)') },
    async ({ tab_id }) => {
      const result = await sendCommand(browserId, 'close_tab', { tab_id });
      if (!result.ok) {
        return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      }
      return { content: [{ type: 'text', text: `Closed tab` }] };
    }
  );

  // ── Resources ──

  server.resource(
    'current-page',
    'browser://current-page',
    { description: 'Current URL and page title of the connected browser' },
    async () => {
      const b = registry.get(browserId);
      return {
        contents: [{
          uri: 'browser://current-page',
          text: b ? `URL: ${b.currentUrl || '(unknown)'}\nBrowser: ${b.name}` : 'Browser not connected',
          mimeType: 'text/plain',
        }],
      };
    }
  );

  return server;
}

/**
 * Express handler for MCP Streamable HTTP endpoint.
 * Each browser gets its own endpoint: POST/GET/DELETE /mcp/:browserId
 */
export async function handleMcpRequest(req, res) {
  const { browserId } = req.params;
  const header = req.headers.authorization;
  let apiKey;
  try {
    const principal = await authenticateToken(header?.startsWith('Bearer ') ? header.slice(7) : '');
    if (principal.role === 'viewer') return res.status(403).json({ error: 'Operator permission required' });
    apiKey = principal.key;
  } catch (e) { return res.status(e.status === 503 ? 503 : 401).json({ error: 'Missing or invalid API key' }); }

  if (!registry.isConnected(browserId)) {
    res.status(404).json({ error: `Browser ${browserId} not connected` });
    return;
  }

  // Scope check — only the key that owns this browser (or admin) can access its MCP.
  // Return 404 (not 403) so non-owners can't probe for browser existence.
  if (!registry.belongsTo(browserId, apiKey)) {
    res.status(404).json({ error: `Browser ${browserId} not connected` });
    return;
  }

  try {
    const server = createMcpServer(browserId);
    const transport = new StreamableHTTPServerTransport({
      sessionIdGenerator: undefined, // stateless
    });
    await server.connect(transport);
    await transport.handleRequest(req, res, req.body);
  } catch (err) {
    console.error(`[mcp] Request error for browser ${browserId}:`, err.message);
    if (!res.headersSent) {
      res.status(500).json({ error: err.message });
    }
  }
}

/**
 * Clean up when browser disconnects.
 * Servers are now created per-request (stateless), so nothing to destroy.
 */
export function destroyMcpServer(browserId) {
  // No cached servers to clean up — per-request servers are self-contained.
  // Clear pool pin if this browser was pinned.
  for (const [key, pinned] of poolPinned) {
    if (pinned === browserId) poolPinned.delete(key);
  }
  for (const [key, sticky] of poolSticky) {
    if (sticky === browserId) poolSticky.delete(key);
  }
}

// ─── Pool MCP ───────────────────────────────────────────────────────────────

/**
 * apiKey -> the browser start_browser made. An agent that started a browser
 * means to drive that one, so navigate stays on it instead of advancing the
 * round-robin into someone else's. Cleared by stop_browser or disconnect.
 */
const poolSticky = new Map();

/**
 * Create a pool MCP server that round-robins commands across all browsers
 * sharing the same API key. Commands that start a new page context (navigate,
 * analyze_page) advance the round-robin; subsequent commands (click, type, etc.)
 * stay pinned to the last-used browser so element IDs remain valid. A browser
 * from start_browser is sticky: everything stays on it until stop_browser.
 *
 * `self` is the request's own origin and Authorization header, which the
 * lifecycle tools replay against the public API.
 */
function createPoolMcpServer(apiKey, self = {}) {
  const server = new McpServer({
    name: 'Oya Browser Pool',
    version: '1.0.0',
  });

  /** Pick browser: sticky if set, else pinned if alive, else round-robin. */
  function pick(advance) {
    const pinned = poolPinned.get(apiKey);
    if (pinned && registry.isConnected(pinned)) {
      if (!advance || poolSticky.get(apiKey) === pinned) return pinned;
    }
    const id = nextBrowser(apiKey);
    if (!id) return null;
    poolPinned.set(apiKey, id);
    return id;
  }

  function browserTag(id) {
    const b = registry.get(id);
    return b ? `[${b.name} ${id}]` : `[${id}]`;
  }

  // ── Tools (mirror the per-browser tools but route through pool) ──

  server.tool(
    'analyze_page',
    `Analyze the current page on the pinned pool browser. Returns structured markdown with interactive elements.`,
    {},
    async () => {
      const bid = pick(false); // stay on pinned browser — analyzing current page, not switching
      if (!bid) return { content: [{ type: 'text', text: 'Error: no browser is running. Call start_browser first.' }], isError: true };
      const result = await sendCommand(bid, 'analyze');
      if (!result.ok) return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      const { markdown, elements, truncated } = result.data;
      const visible = elements.filter(e => e.visible);
      const offscreen = elements.filter(e => !e.visible);
      let index = `\n\n## Element Index (${elements.length} total, ${visible.length} visible)\n`;
      if (visible.length > 0) {
        index += '\n### Visible\n' + visible.map(e => {
          let l = `  [#${e.id}] ${e.type}`; if (e.text) l += `: ${e.text}`; if (e.href) l += ` → ${e.href}`;
          if (e.value) l += ` value="${e.value}"`; if (e.checked) l += ' ✓'; if (e.disabled) l += ' (disabled)'; return l;
        }).join('\n') + '\n';
      }
      if (offscreen.length > 0) {
        index += '\n### Off-screen (scroll to reveal)\n' + offscreen.map(e => {
          let l = `  [#${e.id}] ${e.type}`; if (e.text) l += `: ${e.text}`; if (e.disabled) l += ' (disabled)'; return l;
        }).join('\n') + '\n';
      }
      if (truncated) index += '\n⚠ Page content was truncated.\n';
      return { content: [{ type: 'text', text: `${browserTag(bid)}\n${markdown}${index}` }] };
    }
  );

  server.tool('navigate', 'Navigate the next pool browser to a URL.',
    { url: z.string() },
    async ({ url }) => {
      const bid = pick(true);
      if (!bid) return { content: [{ type: 'text', text: 'Error: no browser is running. Call start_browser first.' }], isError: true };
      const result = await sendCommand(bid, 'navigate', { url }, 90000);
      if (!result.ok) return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      return { content: [{ type: 'text', text: `${browserTag(bid)} Navigated to ${url}` }] };
    }
  );

  server.tool('click', 'Click an element on the pinned pool browser.',
    { element_id: z.number() },
    async ({ element_id }) => {
      const bid = pick(false);
      if (!bid) return { content: [{ type: 'text', text: 'Error: no browser is running. Call start_browser first.' }], isError: true };
      const result = await sendCommand(bid, 'click', { selector: `[data-ac-id="${element_id}"]` });
      if (!result.ok) return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      return { content: [{ type: 'text', text: `${browserTag(bid)} Clicked element ${element_id}` }] };
    }
  );

  server.tool('type', 'Type text into an input on the pinned pool browser.',
    { element_id: z.number(), text: z.string() },
    async ({ element_id, text }) => {
      const bid = pick(false);
      if (!bid) return { content: [{ type: 'text', text: 'Error: no browser is running. Call start_browser first.' }], isError: true };
      const result = await sendCommand(bid, 'type', { selector: `[data-ac-id="${element_id}"]`, text });
      if (!result.ok) return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      return { content: [{ type: 'text', text: `${browserTag(bid)} Typed "${text}" into element ${element_id}` }] };
    }
  );

  server.tool('screenshot', 'Capture screenshot from pinned pool browser.', {},
    async () => {
      const bid = pick(false);
      if (!bid) return { content: [{ type: 'text', text: 'Error: no browser is running. Call start_browser first.' }], isError: true };
      const result = await sendCommand(bid, 'screenshot');
      if (!result.ok) return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      if (result.data?.screenshot) {
        const base64 = result.data.screenshot.replace(/^data:image\/png;base64,/, '');
        return { content: [{ type: 'image', data: base64, mimeType: 'image/png' }] };
      }
      return { content: [{ type: 'text', text: 'No image data returned' }] };
    }
  );

  server.tool('press_key', 'Press a safe navigation key on the pinned pool browser. Allowed: Enter, Escape, Tab, ArrowDown, ArrowUp, ArrowLeft, ArrowRight, Backspace, Delete, Space.',
    { key: z.enum(['Enter', 'Escape', 'Tab', 'ArrowDown', 'ArrowUp', 'ArrowLeft', 'ArrowRight', 'Backspace', 'Delete', 'Space', 'Home', 'End', 'PageUp', 'PageDown']) },
    async ({ key }) => {
      const bid = pick(false);
      if (!bid) return { content: [{ type: 'text', text: 'Error: no browser is running. Call start_browser first.' }], isError: true };
      const result = await sendCommand(bid, 'press_key', { key });
      if (!result.ok) return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      return { content: [{ type: 'text', text: `${browserTag(bid)} Pressed ${key}` }] };
    }
  );

  server.tool('handle_dialog', 'Answer a native browser dialog (confirm or prompt) blocking the page. Alerts are answered automatically.',
    { accept: z.boolean(), prompt_text: z.string().optional() },
    async ({ accept, prompt_text }) => {
      const bid = pick(false);
      if (!bid) return { content: [{ type: 'text', text: 'Error: no browser is running. Call start_browser first.' }], isError: true };
      const result = await sendCommand(bid, 'handle_dialog', { accept, prompt_text });
      if (!result.ok) return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      return { content: [{ type: 'text', text: `${browserTag(bid)} ${result.data?.accepted === false ? 'Dismissed' : 'Accepted'} the ${result.data?.type || ''} dialog` }] };
    }
  );

  server.tool('scroll', 'Scroll the pinned pool browser.',
    { direction: z.enum(['up', 'down']), amount: z.number().optional() },
    async ({ direction, amount }) => {
      const bid = pick(false);
      if (!bid) return { content: [{ type: 'text', text: 'Error: no browser is running. Call start_browser first.' }], isError: true };
      const result = await sendCommand(bid, 'scroll', { direction, amount }, 15000);
      if (!result.ok) return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      if (result.data?.markdown && result.data?.elements) {
        const { markdown, elements } = result.data;
        const visible = elements.filter(e => e.visible);
        let index = `\n\n## Element Index (${elements.length} total, ${visible.length} visible)\n`;
        if (visible.length > 0) {
          index += '\n### Visible\n' + visible.map(e => {
            let l = `  [#${e.id}] ${e.type}`; if (e.text) l += `: ${e.text}`; return l;
          }).join('\n') + '\n';
        }
        return { content: [{ type: 'text', text: `${browserTag(bid)}\n${markdown}${index}` }] };
      }
      return { content: [{ type: 'text', text: `${browserTag(bid)} Scrolled ${direction} ${amount || 500}px` }] };
    }
  );

  server.tool('wait', 'Wait for element on pinned pool browser.',
    { selector: z.string(), timeout: z.number().optional() },
    async ({ selector, timeout }) => {
      const bid = pick(false);
      if (!bid) return { content: [{ type: 'text', text: 'Error: no browser is running. Call start_browser first.' }], isError: true };
      const result = await sendCommand(bid, 'wait', { selector, timeout });
      if (!result.ok) return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      return { content: [{ type: 'text', text: `${browserTag(bid)} Element found: ${selector}` }] };
    }
  );

  server.tool('click_coordinates', 'Click at x,y coordinates on pinned pool browser.',
    { x: z.number(), y: z.number() },
    async ({ x, y }) => {
      const bid = pick(false);
      if (!bid) return { content: [{ type: 'text', text: 'Error: no browser is running. Call start_browser first.' }], isError: true };
      const result = await sendCommand(bid, 'click_coordinates', { x, y });
      if (!result.ok) return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      return { content: [{ type: 'text', text: `${browserTag(bid)} Clicked at (${x}, ${y})` }] };
    }
  );

  server.tool('mouse_move', 'Move mouse to x,y on pinned pool browser.',
    { x: z.number(), y: z.number() },
    async ({ x, y }) => {
      const bid = pick(false);
      if (!bid) return { content: [{ type: 'text', text: 'Error: no browser is running. Call start_browser first.' }], isError: true };
      const result = await sendCommand(bid, 'mouse_move', { x, y });
      if (!result.ok) return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      return { content: [{ type: 'text', text: `${browserTag(bid)} Mouse moved to (${x}, ${y})` }] };
    }
  );

  server.tool('double_click', 'Double-click element or coordinates on pinned pool browser.',
    { element_id: z.number().optional(), x: z.number().optional(), y: z.number().optional() },
    async ({ element_id, x, y }) => {
      const bid = pick(false);
      if (!bid) return { content: [{ type: 'text', text: 'Error: no browser is running. Call start_browser first.' }], isError: true };
      const params = element_id !== undefined ? { selector: `[data-ac-id="${element_id}"]` } : { x, y };
      const result = await sendCommand(bid, 'double_click', params);
      if (!result.ok) return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      return { content: [{ type: 'text', text: `${browserTag(bid)} Double-clicked${element_id !== undefined ? ` element ${element_id}` : ` at (${x}, ${y})`}` }] };
    }
  );

  server.tool('keyboard_type', 'Type text using keyboard on pinned pool browser (types into whatever is focused).',
    { text: z.string() },
    async ({ text }) => {
      const bid = pick(false);
      if (!bid) return { content: [{ type: 'text', text: 'Error: no browser is running. Call start_browser first.' }], isError: true };
      const result = await sendCommand(bid, 'keyboard_type', { text });
      if (!result.ok) return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      return { content: [{ type: 'text', text: `${browserTag(bid)} Typed "${text}"` }] };
    }
  );

  server.tool('drag', 'Drag from one point to another on pinned pool browser.',
    { from_x: z.number(), from_y: z.number(), to_x: z.number(), to_y: z.number() },
    async ({ from_x, from_y, to_x, to_y }) => {
      const bid = pick(false);
      if (!bid) return { content: [{ type: 'text', text: 'Error: no browser is running. Call start_browser first.' }], isError: true };
      const result = await sendCommand(bid, 'drag', { from_x, from_y, to_x, to_y });
      if (!result.ok) return { content: [{ type: 'text', text: `Error: ${result.error}` }], isError: true };
      return { content: [{ type: 'text', text: `${browserTag(bid)} Dragged from (${from_x}, ${from_y}) to (${to_x}, ${to_y})` }] };
    }
  );

  /**
   * Browser lifecycle goes through the public API as the caller, so quotas,
   * budgets, persona caps, audit and billing apply exactly as over REST, and
   * a scoped credential cannot do more here than there.
   */
  async function selfApi(path, body) {
    if (!self.origin || !self.authorization) throw new Error('browser lifecycle is unavailable on this endpoint');
    const res = await fetch(`${self.origin}/api${path}`, {
      method: 'POST',
      headers: { Authorization: self.authorization, 'Content-Type': 'application/json', 'Idempotency-Key': globalThis.crypto.randomUUID() },
      body: JSON.stringify(body || {}),
      signal: AbortSignal.timeout(150_000),
    });
    const data = await res.json().catch(() => ({}));
    if (!res.ok) throw new Error(data.error || `${res.status} ${res.statusText}`);
    return data;
  }
  const text = (t) => ({ content: [{ type: 'text', text: t }] });
  const fail = (t) => ({ content: [{ type: 'text', text: `Error: ${t}` }], isError: true });

  server.tool('start_browser',
    'Start a new browser on this key and make it the one every other tool drives. Uses the key\'s configured provider unless one is given. It costs a session until stop_browser.',
    {
      persona: z.string().optional().describe("Persona id, 'auto' (least recently used under its concurrency cap) or 'default'"),
      provider: z.enum(['oya-cloud', 'oya-selfhosted', 'browserbase', 'steel', 'anchor', 'browseruse']).optional(),
      name: z.string().max(100).optional(),
      url: z.string().optional().describe('Navigate here once the browser is ready'),
    },
    async ({ persona, provider, name, url }) => {
      let started;
      try { started = await selfApi('/browsers/start', { profile: persona, provider, name }); }
      catch (e) { return fail(e.message); }
      const id = started.id;
      // Cloud browsers dial in after they boot; the tools need it connected.
      const deadline = Date.now() + 120_000;
      while (!registry.isConnected(id) && Date.now() < deadline) await new Promise((r) => setTimeout(r, 1000));
      if (!registry.isConnected(id)) return text(`Started ${id} on ${started.provider}; it is still booting. Check pool_status, then retry.`);
      poolPinned.set(apiKey, id);
      poolSticky.set(apiKey, id);
      if (url) {
        const nav = await sendCommand(id, 'navigate', { url }, 90000);
        if (!nav.ok) return fail(`${browserTag(id)} started, but navigating failed: ${nav.error}`);
      }
      return text(`${browserTag(id)} ready on ${started.provider} as persona ${started.persona}${url ? `, at ${url}` : ''}. `
        + 'Every tool now drives this browser. Call stop_browser when you are done.');
    }
  );

  server.tool('stop_browser', 'Stop a browser and release its session. Defaults to the browser the tools are driving.',
    { browser_id: z.string().optional() },
    async ({ browser_id }) => {
      const id = browser_id || poolPinned.get(apiKey);
      if (!id) return fail('no browser to stop');
      let result;
      try { result = (await selfApi('/browsers/stop', { ids: [id] })).results?.[0]; }
      catch (e) { return fail(e.message); }
      if (result && result.ok === false) return fail(result.error || `could not stop ${id}`);
      if (poolPinned.get(apiKey) === id) poolPinned.delete(apiKey);
      if (poolSticky.get(apiKey) === id) poolSticky.delete(apiKey);
      return text(`Stopped ${id}.`);
    }
  );

  server.tool('pool_status', 'Show pool size and connected browsers.', {},
    async () => {
      const stats = poolStats(apiKey);
      const pinned = poolPinned.get(apiKey);
      const lines = stats.browsers.map(b => {
        const pin = b.id === pinned ? ' ★' : '';
        return `  ${b.name} (${b.id})${pin} — ${b.currentUrl || 'idle'}`;
      });
      return { content: [{ type: 'text', text: `Pool: ${stats.size} browsers\n${lines.join('\n')}` }] };
    }
  );

  // ── Resource ──
  server.resource('pool-status', 'browser://pool-status',
    { description: 'Pool size and browser list' },
    async () => {
      const stats = poolStats(apiKey);
      return { contents: [{ uri: 'browser://pool-status', text: JSON.stringify(stats, null, 2), mimeType: 'application/json' }] };
    }
  );

  return server;
}

/**
 * Express handler for pool MCP endpoint: POST/GET/DELETE /mcp/pool
 */
export async function handlePoolMcpRequest(req, res) {
  const header = req.headers.authorization;
  let apiKey;
  try {
    const principal = await authenticateToken(header?.startsWith('Bearer ') ? header.slice(7) : '');
    if (principal.role === 'viewer') return res.status(403).json({ error: 'Operator permission required' });
    apiKey = principal.key;
  } catch (e) { return res.status(e.status === 503 ? 503 : 401).json({ error: 'Missing or invalid API key' }); }

  try {
    const server = createPoolMcpServer(apiKey, {
      origin: `http://127.0.0.1:${req.socket.localPort}`,
      authorization: header,
    });
    const transport = new StreamableHTTPServerTransport({
      sessionIdGenerator: undefined,
    });
    await server.connect(transport);
    await transport.handleRequest(req, res, req.body);
  } catch (err) {
    console.error(`[mcp] Pool request error:`, err.message);
    if (!res.headersSent) {
      res.status(500).json({ error: err.message });
    }
  }
}
