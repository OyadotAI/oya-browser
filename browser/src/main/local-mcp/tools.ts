/**
 * The tools the local MCP port offers: the same names and behaviour as Oya's
 * hosted MCP, each one a browser command the agent loop already runs. A tool
 * call becomes that command, run as the calling agent's lane, so it works in its
 * own tab and closes only tabs it opened.
 */
import { SAFE_KEYS } from './constants.ts';

/** A tool's JSON Schema for its arguments. */
type Schema = Record<string, unknown>;

/** The arguments a call arrives with. */
export type ToolArgs = Record<string, unknown>;

/** One tool: what the agent reads, what it accepts, and the command it runs. */
export interface LocalTool {
  /** What the agent is told it does. */
  description: string;
  /** Its arguments. */
  inputSchema: Schema;
  /** The browser command and parameters a call runs. */
  command: (args: ToolArgs) => [action: string, params: Record<string, unknown>];
}

/** An object schema with `properties`, of which `required` must be given. */
const object = (properties: Record<string, Schema> = {}, required: string[] = []): Schema => ({
  type: 'object',
  properties,
  required,
  additionalProperties: false,
});

/** The element id argument the element tools share. */
const ELEMENT = { element_id: { type: 'number', description: 'The element id from analyze_page' } };

/** CSS selector for an element by the number analyze gave it. */
const byId = (id: unknown) => `[data-ac-id="${Number(id)}"]`;

/** The page tools: read, move, act. */
const PAGE_TOOLS: Record<string, LocalTool> = {
  analyze_page: {
    description:
      'Read the current page: its content and every interactive element, numbered for click and type. format: toon (default, fewest tokens), markdown or jsonl.',
    inputSchema: object({ format: { type: 'string', enum: ['toon', 'markdown', 'jsonl'] } }),
    command: ({ format }) => ['analyze', format ? { format } : {}],
  },
  navigate: {
    description: 'Load a URL in your tab.',
    inputSchema: object({ url: { type: 'string' } }, ['url']),
    command: ({ url }) => ['navigate', { url }],
  },
  click: {
    description: 'Click an element by its id from analyze_page.',
    inputSchema: object(ELEMENT, ['element_id']),
    command: ({ element_id }) => ['click', { selector: byId(element_id) }],
  },
  type: {
    description:
      'Type text into a field by its id from analyze_page, replacing what it held. Typing keeps every id valid: type the next field straight away, without analyzing again.',
    inputSchema: object({ ...ELEMENT, text: { type: 'string' } }, ['element_id', 'text']),
    command: ({ element_id, text }) => ['type', { selector: byId(element_id), text }],
  },
  press_key: {
    description: `Press one navigation key: ${SAFE_KEYS.join(', ')}.`,
    inputSchema: object({ key: { type: 'string', enum: SAFE_KEYS } }, ['key']),
    command: ({ key }) => ['press_key', { key }],
  },
  scroll: {
    description: 'Scroll the page up or down, by pixels (500 when left out).',
    inputSchema: object({ direction: { type: 'string', enum: ['up', 'down'] }, amount: { type: 'number' } }, [
      'direction',
    ]),
    command: ({ direction, amount }) => ['scroll', { direction, amount }],
  },
  screenshot: {
    description: 'Capture the visible part of your tab as an image.',
    inputSchema: object(),
    command: () => ['screenshot', {}],
  },
  go_back: { description: 'Go back in your tab.', inputSchema: object(), command: () => ['back', {}] },
  go_forward: { description: 'Go forward in your tab.', inputSchema: object(), command: () => ['forward', {}] },
};

/** The tab and dialog tools. */
const TAB_TOOLS: Record<string, LocalTool> = {
  list_tabs: {
    description: 'List the open tabs; the ones marked yours are the tabs you may work in.',
    inputSchema: object(),
    command: () => ['list_tabs', {}],
  },
  open_tab: {
    description: 'Open a new tab of your own, on a URL or the start page.',
    inputSchema: object({ url: { type: 'string' } }),
    command: ({ url }) => ['open_tab', url ? { url } : {}],
  },
  switch_tab: {
    description: 'Switch to one of your own tabs. Tabs the person or another agent is using are refused.',
    inputSchema: object({ tab_id: { type: 'number' } }, ['tab_id']),
    command: ({ tab_id }) => ['switch_tab', { tab_id }],
  },
  close_tab: {
    description: 'Close a tab you opened (your current one when left out). Other tabs are refused; leave them open.',
    inputSchema: object({ tab_id: { type: 'number' } }),
    command: ({ tab_id }) => ['close_tab', tab_id === undefined ? {} : { tab_id }],
  },
  handle_dialog: {
    description: 'Accept or dismiss the alert, confirm or prompt the page opened.',
    inputSchema: object({ accept: { type: 'boolean' }, prompt_text: { type: 'string' } }, ['accept']),
    command: ({ accept, prompt_text }) => ['handle_dialog', { accept, prompt_text }],
  },
};

/** Every local tool, by name. */
export const LOCAL_TOOLS: Readonly<Record<string, LocalTool>> = { ...PAGE_TOOLS, ...TAB_TOOLS };

/** The tools as tools/list answers them. */
export const toolList = () =>
  Object.entries(LOCAL_TOOLS).map(([name, t]) => ({ name, description: t.description, inputSchema: t.inputSchema }));
