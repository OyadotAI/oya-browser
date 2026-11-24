/** Diagnostics use the admitted lane's actual view and preserve native identity on failure. */
import type { PageDriver } from '../actions/driver.ts';
import type { NativePage } from '../native/index.ts';
import { debugRegistry, type DebugRegistry } from './registry.ts';
import { debugIdentity, debugNativeIdentity } from './identity.ts';
import { readDebugFrames } from './read.ts';
import { debugHitTest } from './hit-test.ts';
import { clickDebugElement } from './click.ts';
import { withDebugScope } from './scope.ts';
import { currentLane } from '../control/agent-tab.ts';
export { useDebugChannel } from './channel.ts';
export { useDebugApplication, rememberDebugProfile } from './identity.ts';
export { useNativeMouse } from './native-input.ts';
export { inspectDebugMouseSurface } from './surface.ts';
/** Native main receives these after local MCP ownership/control admission. */
export interface DebugParams {
  /** Fixed operation. */ kind?: string;
  /** Descendant inclusion. */ frames?: boolean;
  /** Exact new diagnostic read numbers. */ framed?: boolean;
  /** Explicit source. */ expression?: unknown;
  /** Opaque frame reference. */ frame?: string;
  /** Horizontal CSS px. */ x?: number;
  /** Vertical CSS px. */ y?: number;
  /** Published element number. */ element?: number;
  /** Typed text. */ text?: string;
  /** Submit form. */ submit?: boolean;
}
/** Handler inputs contain the already pinned native view. */
type Handler = (driver: PageDriver, registry: DebugRegistry, params: DebugParams) => Promise<unknown>;
const handlers: Record<string, Handler> = {
  identity: async (_driver, registry) => debugIdentity(registry.page),
  read: async (_driver, registry, p) => readDebugFrames(registry, p.frames !== false),
  eval: async (_driver, registry, params) => {
    const doc = params.frame
      ? registry.resolve(params.frame)
      : await registry.document(registry.page.webContents.mainFrame);
    return { frame: doc.ref, url: doc.url, value: await registry.run(doc, 'eval', { expression: params.expression }) };
  },
  hit_test: async (_driver, registry, p) => debugHitTest(registry, { x: Number(p.x), y: Number(p.y) }),
  click: async (driver, registry, p) => clickDebugElement(driver, registry, registry.target(Number(p.element))),
  type: async (_driver, registry, p) => {
    const target = registry.target(Number(p.element));
    await registry.run(target.document, 'type', { element: target.local, text: p.text, submit: p.submit });
    return { typed: true, frame: target.document.ref, input: 'dom_value' };
  },
};
/** Native metadata survives busy-state errors and renderer inspection failures. */
export async function browserDiagnostic(driver: PageDriver, page: NativePage, params: DebugParams) {
  const identity = debugNativeIdentity(page);
  if (!currentLane().startsWith('mcp-'))
    return debugFailure(identity, Error('Browser diagnostics require the local MCP lane.'));
  try {
    return await withDebugScope(page, () => runBrowserDiagnostic(driver, page, params));
  } catch (why) {
    return debugFailure(identity, why);
  }
}
/** The operation holds its immutable view while measuring native metadata. */
async function runBrowserDiagnostic(driver: PageDriver, page: NativePage, params: DebugParams) {
  const identity = await debugIdentity(page);
  try {
    const data = await invokeDebugHandler(driver, page, params);
    return debugSuccess(identity, params, data);
  } catch (why) {
    return debugFailure(identity, why);
  }
}
/** Fixed handler admission does not allow page data to choose native operations. */
async function invokeDebugHandler(driver: PageDriver, page: NativePage, params: DebugParams): Promise<unknown> {
  if (!Object.hasOwn(handlers, params.kind ?? '')) throw Error('Unknown browser diagnostic operation');
  if (params.kind === 'click' && params.framed === false)
    throw Error('Legacy analyzer numbers cannot select native diagnostic clicks. Read again with browser_read_frames.');
  return handlers[params.kind!](driver, debugRegistry(page), params);
}
/** Only explicit renderer data enters structured JSON; text remains independently bounded. */
function debugSuccess(identity: ReturnType<typeof debugNativeIdentity>, params: DebugParams, data: unknown) {
  return { ok: true, identity, outcome: 'completed', text: debugResultText(params, data), data: unwrapDebugRead(data) };
}
/** A failed input reports every completed event, without implying page activation. */
function debugFailure(identity: ReturnType<typeof debugNativeIdentity>, why: unknown) {
  const dispatched = (why as DebugFailure)?.dispatched ?? [];
  const failure = why as DebugFailure,
    unavailable = failure?.code === 'native_input_unavailable';
  return {
    ok: false,
    identity,
    outcome: unavailable ? 'native_input_unavailable' : dispatched.length ? 'partial_input' : 'failed',
    text: String((why as Error)?.message ?? why),
    data: unavailable ? { code: failure.code, reason: failure.reason, dispatched } : { dispatched },
  };
}
/** Completed native event log retained on an error. */
interface DebugFailure {
  /** Accepted native events. */ dispatched?: string[];
  /** Fixed host capability code. */ code?: string;
  /** Verified native obstruction. */ reason?: string;
}
/** Reads remain text while eval and inspection also return separate valid JSON. */
function debugResultText(params: DebugParams, data: unknown): string {
  return params.kind === 'read' ? (data as DebugRead).text : JSON.stringify(data);
}
/** The inventory is carried once rather than duplicating all page text. */
function unwrapDebugRead(data: unknown): unknown {
  return (data as DebugRead | null)?.data ?? data;
}
/** Frame read output separates human text and native coverage. */
interface DebugRead {
  /** Readable sections. */ text: string;
  /** Coverage inventory. */ data: unknown;
}
