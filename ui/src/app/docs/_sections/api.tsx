/**
 * Docs: the REST API, the command reference and the WebSocket protocol.
 */
'use client';

import type { ReactNode } from 'react';
import { CodeBlock, InlineCode, SectionHeading, Table } from '../_docs/blocks';

/** Rows of a table in the REST API section. */
const REST_API_ROWS_1: ReactNode[][] = [
  [<InlineCode key="m1">GET</InlineCode>, <InlineCode key="e1">/health</InlineCode>, 'Server status + browser count'],
  [
    <InlineCode key="m2">POST</InlineCode>,
    <InlineCode key="e2">/register-key</InlineCode>,
    <span key="d2">
      Register a new API key (<InlineCode>{`{ "key": "..." }`}</InlineCode>)
    </span>,
  ],
  [<InlineCode key="m3">GET</InlineCode>, <InlineCode key="e3">/browsers</InlineCode>, 'List your connected browsers'],
  [
    <InlineCode key="ms">POST</InlineCode>,
    <InlineCode key="es">/browsers/start</InlineCode>,
    <span key="ds">
      Start one (<InlineCode>{`{ "persona": "auto" }`}</InlineCode>), provider comes from your key
    </span>,
  ],
  [
    <InlineCode key="mg">GET</InlineCode>,
    <InlineCode key="eg">/browsers/:id</InlineCode>,
    'One browser with counters, health and its recent activity',
  ],
  [
    <InlineCode key="mst">POST</InlineCode>,
    <InlineCode key="est">/browsers/:id/stop</InlineCode>,
    'Stop it, destroys a cloud sandbox, releases a CDP session',
  ],
  [
    <InlineCode key="msb">POST</InlineCode>,
    <InlineCode key="esb">/browsers/stop</InlineCode>,
    <span key="dsb">
      Bulk: <InlineCode>{`{ "ids": [...] }`}</InlineCode> or <InlineCode>{`{ "all": true }`}</InlineCode>
    </span>,
  ],
  [
    <InlineCode key="mf">GET</InlineCode>,
    <InlineCode key="ef">/fleet</InlineCode>,
    'Totals by health, provider and persona; usage and limits',
  ],
  [
    <InlineCode key="m4">POST</InlineCode>,
    <InlineCode key="e4">/browsers/:id/command</InlineCode>,
    <span key="d4">
      Send command (<InlineCode>{`{ "action": "...", "params": {} }`}</InlineCode>)
    </span>,
  ],
  [
    <InlineCode key="m5">POST</InlineCode>,
    <InlineCode key="e5">/browsers/:id/chat</InlineCode>,
    <span key="d5">
      Chat (<InlineCode>{`{ "messages": [...] }`}</InlineCode>)
    </span>,
  ],
  [
    <InlineCode key="m6">GET</InlineCode>,
    <InlineCode key="e6">/live/:id?ticket=...</InlineCode>,
    'SSE live view frame stream (single-use ticket)',
  ],
  [
    <InlineCode key="m7">GET/POST</InlineCode>,
    <InlineCode key="e7">/mcp/:id</InlineCode>,
    'MCP Streamable HTTP endpoint',
  ],
  [
    <InlineCode key="mp1">GET/POST</InlineCode>,
    <InlineCode key="ep1">/personas</InlineCode>,
    'List or create personas',
  ],
  [
    <InlineCode key="mp2">DELETE</InlineCode>,
    <InlineCode key="ep2">/personas/:id</InlineCode>,
    'Delete a persona (409 while in use)',
  ],
  [
    <InlineCode key="mp4">PUT</InlineCode>,
    <InlineCode key="ep4">/personas/:id</InlineCode>,
    'Rename, set the cap or the proxy hint, never the device',
  ],
  [
    <InlineCode key="mp5">POST</InlineCode>,
    <InlineCode key="ep5">/personas/:id/clone</InlineCode>,
    'A new persona of the same kind of device',
  ],
  [
    <InlineCode key="mp6">POST</InlineCode>,
    <InlineCode key="ep6">/personas/preview</InlineCode>,
    'The fingerprint a set of choices would produce',
  ],
  [
    <InlineCode key="mp7">GET</InlineCode>,
    <InlineCode key="ep7">/personas/options</InlineCode>,
    'Platforms and their coherent timezones and locales',
  ],
  [
    <InlineCode key="mp3">PUT</InlineCode>,
    <InlineCode key="ep3">/personas/:id/mfa</InlineCode>,
    'Store a second factor',
  ],
  [
    <InlineCode key="mc1">POST</InlineCode>,
    <InlineCode key="ec1">/browsers/:id/captcha</InlineCode>,
    'Detect and clear a CAPTCHA',
  ],
  [
    <InlineCode key="mc2">POST</InlineCode>,
    <InlineCode key="ec2">/browsers/:id/mfa</InlineCode>,
    'Answer an MFA prompt',
  ],
  [
    <InlineCode key="mu">GET</InlineCode>,
    <InlineCode key="eu">/usage</InlineCode>,
    "This key's usage, bucketed by hour",
  ],
  [<InlineCode key="ma">GET</InlineCode>, <InlineCode key="ea">/audit</InlineCode>, "This key's audit history"],
  [
    <InlineCode key="m8">GET</InlineCode>,
    <InlineCode key="e8">/config</InlineCode>,
    "This key's settings, credentials masked",
  ],
  [<InlineCode key="m9">POST</InlineCode>, <InlineCode key="e9">/config</InlineCode>, "Update this key's settings"],
];

/** Rows of a table in the REST API section. */
const REST_API_ROWS_2: ReactNode[][] = [
  [
    <InlineCode key="a1">navigate</InlineCode>,
    <span key="p1">
      <InlineCode>url</InlineCode> (required)
    </span>,
    'Navigate to a URL',
  ],
  [
    <InlineCode key="a2">open_tab</InlineCode>,
    <span key="p2">
      <InlineCode>url</InlineCode> (optional)
    </span>,
    'Open a new tab',
  ],
  [
    <InlineCode key="a3">switch_tab</InlineCode>,
    <span key="p3">
      <InlineCode>tab_id</InlineCode> (required)
    </span>,
    'Activate a tab by ID',
  ],
  [
    <InlineCode key="a4">close_tab</InlineCode>,
    <span key="p4">
      <InlineCode>tab_id</InlineCode> (optional, defaults to active)
    </span>,
    'Close a tab',
  ],
  [<InlineCode key="a5">list_tabs</InlineCode>, <em key="params">none</em>, 'List all open tabs'],
];

/** Rows of a table in the REST API section. */
const REST_API_ROWS_3: ReactNode[][] = [
  [
    <InlineCode key="a1">analyze</InlineCode>,
    <InlineCode key="params">format?</InlineCode>,
    'Full page + numbered elements, as markdown (default), toon or jsonl',
  ],
  [
    <InlineCode key="a2">read_page</InlineCode>,
    <span key="p2">
      <InlineCode>selector</InlineCode> (optional), <InlineCode>limit</InlineCode> (default 50)
    </span>,
    'Lightweight element listing',
  ],
  [<InlineCode key="a3">screenshot</InlineCode>, <em key="params">none</em>, 'Capture page as PNG'],
];

/** Rows of a table in the REST API section. */
const REST_API_ROWS_4: ReactNode[][] = [
  [
    <InlineCode key="a1">click</InlineCode>,
    <span key="p1">
      <InlineCode>selector</InlineCode> (e.g. <InlineCode>[data-ac-id=&quot;3&quot;]</InlineCode>)
    </span>,
    'Click an element',
  ],
  [
    <InlineCode key="a2">type</InlineCode>,
    <span key="p2">
      <InlineCode>selector</InlineCode> + <InlineCode>text</InlineCode>
    </span>,
    'Type into an input',
  ],
  [
    <InlineCode key="a3">press_key</InlineCode>,
    <span key="p3">
      <InlineCode>key</InlineCode> (e.g. Enter, Tab, Escape)
    </span>,
    'Press a keyboard key',
  ],
  [
    <InlineCode key="a4">scroll</InlineCode>,
    <span key="p4">
      <InlineCode>direction</InlineCode> (up/down), <InlineCode>amount</InlineCode> (px, default 500)
    </span>,
    'Scroll the page',
  ],
  [
    <InlineCode key="a5">wait</InlineCode>,
    <span key="p5">
      <InlineCode>selector</InlineCode>, <InlineCode>timeout</InlineCode> (ms, default 10000)
    </span>,
    'Wait for element to appear',
  ],
];

/** The REST API section. */
function RestApi() {
  return (
    <>
      {/* ============ REST API ============ */}
      <SectionHeading id="rest-api">REST API</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        All endpoints require <InlineCode>Authorization: Bearer YOUR_API_KEY</InlineCode> header (except health and
        register). Interactive API testing available at{' '}
        <a href="/swagger" className="text-accent hover:text-accent-hover transition-colors">
          /swagger
        </a>
        .
      </p>
      <Table headers={['Method', 'Endpoint', 'Description']} rows={REST_API_ROWS_1} />

      <h3 id="command-api" className="text-base font-semibold mt-6 mb-2 text-text">
        Command API Reference
      </h3>
      <p className="mb-3 text-[15px] leading-relaxed">
        Send commands via <InlineCode>POST /browsers/:id/command</InlineCode>. Each action uses only specific params,
        the rest are ignored.
      </p>

      <h3 className="text-base font-semibold mt-6 mb-2 text-text">Navigation Actions</h3>
      <Table headers={['Action', 'Params', 'Description']} rows={REST_API_ROWS_2} />

      <h3 className="text-base font-semibold mt-6 mb-2 text-text">Page Analysis Actions</h3>
      <Table headers={['Action', 'Params', 'Description']} rows={REST_API_ROWS_3} />

      <h3 className="text-base font-semibold mt-6 mb-2 text-text">Interaction Actions</h3>
      <Table headers={['Action', 'Params', 'Description']} rows={REST_API_ROWS_4} />

      <RestApiPart2 />
      <RestApiPart3 />
    </>
  );
}

/** The REST API section, continued. */
function RestApiPart2() {
  return (
    <>
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
    </>
  );
}

/** The REST API section, continued. */
function RestApiPart3() {
  return (
    <>
      <h3 className="text-base font-semibold mt-6 mb-2 text-text">Typical Workflow</h3>
      <CodeBlock>{`1. navigate → go to the page
2. analyze  → understand the page, get element IDs
3. click / type / press_key / scroll → interact
4. analyze  → re-analyze after page changes (old IDs are invalid)
5. repeat until task is done`}</CodeBlock>
    </>
  );
}

/** The WebSocket Protocol section. */
function Websocket() {
  return (
    <>
      {/* ============ WEBSOCKET ============ */}
      <SectionHeading id="websocket">WebSocket Protocol</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        Browsers connect via WebSocket at <InlineCode>wss://oyabrowser.com/ws</InlineCode>.
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
        Both sides send <InlineCode>{`{ "type": "ping" }`}</InlineCode> and respond with{' '}
        <InlineCode>{`{ "type": "pong" }`}</InlineCode> every 15-20 seconds.
      </p>

      <h3 className="text-base font-semibold mt-6 mb-2 text-text">Live Stream</h3>
      <p className="mb-3 text-[15px] leading-relaxed">
        Server → Browser: <InlineCode>{`{ "type": "stream_start", "fps": 2 }`}</InlineCode>
      </p>
      <p className="mb-3 text-[15px] leading-relaxed">
        Browser → Server: <InlineCode>{`{ "type": "frame", "data": "data:image/jpeg;base64,..." }`}</InlineCode>
      </p>
      <WebsocketPart2 />
    </>
  );
}

/** The WebSocket Protocol section, continued. */
function WebsocketPart2() {
  return (
    <>
      <p className="mb-3 text-[15px] leading-relaxed">
        Server → Browser: <InlineCode>{`{ "type": "stream_stop" }`}</InlineCode>
      </p>
    </>
  );
}

/** The REST API, the command reference and the WebSocket protocol. */
export function ApiDocs() {
  return (
    <>
      <RestApi />
      <Websocket />
    </>
  );
}
