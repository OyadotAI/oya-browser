/** Library tools use the same authenticated, control-gated command path as tab tools. */
import { sendCommand } from '../browsers/socket.ts';
import { LIBRARY_TOOL_NAMES } from './library-tools.ts';
import type { ToolHandler } from './tool-handlers.ts';
/** Preserve structured pagination and mutation results, and report browser refusals honestly. */
const run =
  (action: string): ToolHandler =>
  async (browserId, args) => {
    const result = await sendCommand(browserId, action, args);
    return result.ok ? JSON.stringify(result.data) : `Error: ${result.error}`;
  };
/** Retry-safe bookmark mutations are distinct commands, never a UI toggle. */
export const LIBRARY_TOOL_HANDLERS: Record<string, ToolHandler> = Object.fromEntries(
  LIBRARY_TOOL_NAMES.map((name) => [name, run(name)]),
);
