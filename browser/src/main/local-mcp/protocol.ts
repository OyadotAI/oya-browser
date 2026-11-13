/**
 * The MCP conversation, without the HTTP around it: one JSON-RPC message in,
 * its reply out (null for a notification). Tool calls go to `run`, which runs
 * the browser command as the calling agent.
 */
import type { LocalResult } from '../connection/commands.ts';
import { LOCAL_TOOLS, toolList, type ToolArgs } from './tools.ts';
import { MCP_PROTOCOL_VERSION, RpcError } from './constants.ts';

/** Runs one browser command for the calling agent. */
export type RunCommand = (action: string, params: Record<string, unknown>) => Promise<LocalResult>;

/** A JSON-RPC request or notification. */
interface RpcMessage {
  /** Always '2.0'. */
  jsonrpc?: unknown;
  /** Absent on a notification. */
  id?: string | number | null;
  /** The method. */
  method?: unknown;
  /** Its parameters. */
  params?: Record<string, unknown>;
}

/** What tools/call answers: content blocks, flagged when the tool failed. */
interface ToolReply {
  /** Text or image blocks. */
  content: Array<Record<string, string>>;
  /** Set when the call failed, so the agent reads the text as the reason. */
  isError?: boolean;
}

/** What the agent is told when it connects. */
const INSTRUCTIONS =
  'Oya Browser on this computer. Work in your own tab: open_tab to start, analyze_page to read, then click/type by element id. Ids from the last analyze_page stay valid until you analyze again or the page navigates: fill a whole form from one analyze_page, and analyze again only after a click changes the page. The person keeps their tabs; you close only tabs you opened.';

/** A data URL's media type and payload. */
const DATA_URL = /^data:(image\/[\w+.-]+);base64,/;

/** A failed tool call as the agent reads it. */
const failed = (text: string): ToolReply => ({ content: [{ type: 'text', text }], isError: true });

/** A screenshot as an image block; a bare payload is taken to be PNG. */
function image(dataUrl: string): ToolReply {
  const type = dataUrl.match(DATA_URL)?.[1] ?? 'image/png';
  return { content: [{ type: 'image', data: dataUrl.replace(DATA_URL, ''), mimeType: type }] };
}

/** A command's answer as tool content: the page text for a read, an image for a screenshot, else its data. */
export function toolReply(result: LocalResult): ToolReply {
  if (!result.ok) return failed(result.error || 'The browser could not do that.');
  const data = (result.data ?? {}) as Record<string, unknown>;
  if (typeof data.screenshot === 'string') return image(data.screenshot);
  const text = typeof data.page === 'string' ? data.page : JSON.stringify(data);
  return { content: [{ type: 'text', text: text === '{}' ? 'Done.' : text }] };
}

/** Runs tools/call: the named tool's command, as the calling agent. */
async function callTool(params: Record<string, unknown> | undefined, run: RunCommand): Promise<ToolReply> {
  const name = String(params?.name ?? '');
  if (!Object.hasOwn(LOCAL_TOOLS, name)) return failed(`Unknown tool: ${name}`);
  const args = (params?.arguments ?? {}) as ToolArgs;
  const [action, commandParams] = LOCAL_TOOLS[name].command(args);
  return toolReply(await run(action, commandParams));
}

/** What initialize answers. */
const initialized = () => ({
  protocolVersion: MCP_PROTOCOL_VERSION,
  capabilities: { tools: { listChanged: false } },
  serverInfo: { name: 'Oya Browser', version: process.env.npm_package_version || 'desktop' },
  instructions: INSTRUCTIONS,
});

/** Each method's answer. */
const METHODS: Record<string, (params: Record<string, unknown> | undefined, run: RunCommand) => unknown> = {
  initialize: () => initialized(),
  ping: () => ({}),
  'tools/list': () => ({ tools: toolList() }),
  'tools/call': (params, run) => callTool(params, run),
};

/** A JSON-RPC error reply. */
const rpcError = (id: RpcMessage['id'], code: number, message: string) => ({
  jsonrpc: '2.0',
  id: id ?? null,
  error: { code, message },
});

/** Answers one message; null for a notification, which gets no reply. */
export async function answer(message: unknown, run: RunCommand): Promise<object | null> {
  const msg = (message ?? {}) as RpcMessage;
  if (typeof msg !== 'object' || Array.isArray(msg) || msg.jsonrpc !== '2.0' || typeof msg.method !== 'string')
    return rpcError(msg?.id, RpcError.INVALID_REQUEST, 'Expected one JSON-RPC 2.0 request');
  if (msg.id === undefined) return null;
  if (!Object.hasOwn(METHODS, msg.method))
    return rpcError(msg.id, RpcError.METHOD_NOT_FOUND, `No method ${msg.method}`);
  return { jsonrpc: '2.0', id: msg.id, result: await METHODS[msg.method](msg.params, run) };
}
