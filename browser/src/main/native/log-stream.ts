/** Console observation through browser-owned events, scoped to one native page and control lease. */
import type { NativePage } from './page.ts';
/** Native event values are serialized text, never page object handles or debugger remote objects. */
export type NativeEventSink = (method: string, params: Record<string, unknown>) => void;
/** Bound messages from noisy or adversarial pages. */
const LOG_TEXT_MAX = 16384;
/** Native event fields used by the compatibility adapter. */
interface ConsoleMessage {
  /** Electron's level name. */
  level: string;
  /** Browser-rendered console text. */
  message: string;
  /** Script address, if known. */
  sourceId?: string;
  /** One-based source line, if known. */
  lineNumber?: number;
}
/** Convert native console levels to the CDP Log vocabulary without claiming structured Runtime arguments. */
function logEntry(details: ConsoleMessage): object {
  return {
    source: 'javascript',
    level: details.level === 'warning' ? 'warning' : logLevel(details.level),
    text: details.message.slice(0, LOG_TEXT_MAX),
    timestamp: Date.now(),
    ...(details.sourceId ? { url: details.sourceId } : {}),
    ...(details.lineNumber ? { lineNumber: details.lineNumber - 1 } : {}),
  };
}
/** Unsupported native verbosity maps to the protocol's verbose level, not an invented event shape. */
function logLevel(level: string): string {
  return ['info', 'error'].includes(level) ? level : 'verbose';
}
/** No past human activity is replayed: only new authorized native messages are delivered. */
export function watchNativeLog(page: NativePage, allowed: () => boolean, emit: NativeEventSink): () => void {
  const listener = (details: ConsoleMessage): void => {
    if (!page.webContents.isDestroyed() && allowed()) emit('Log.entryAdded', { entry: logEntry(details) });
  };
  page.webContents.on('console-message', listener);
  return () => page.webContents.off('console-message', listener);
}
