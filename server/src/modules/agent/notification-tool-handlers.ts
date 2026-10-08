/** Notifications use the same ownership and control-gated socket path as page tools. */
import { sendCommand } from '../browsers/socket.ts';
import { NOTIFICATION_TOOL_NAMES } from './notification-tools.ts';
import type { ToolHandler } from './tool-handlers.ts';
/** Keep structured inbox results and report remote errors honestly. */
const runNotification =
  (action: string): ToolHandler =>
  async (browserId, args) => {
    const result = await sendCommand(browserId, action, args);
    return result.ok ? JSON.stringify(result.data) : `Error: ${result.error}`;
  };
/** The model and MCP invoke the same implementation. */
export const NOTIFICATION_TOOL_HANDLERS: Record<string, ToolHandler> = Object.fromEntries(
  NOTIFICATION_TOOL_NAMES.map((name) => [name, runNotification(name)]),
);
