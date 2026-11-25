/** Native frame-correlated IPC refuses sibling, replacement and page-world responses. */
import { randomUUID } from 'node:crypto';
import type { IpcMain, IpcMainEvent, WebFrameMain } from 'electron';
import type { NativePage } from '../native/index.ts';
import { BROWSER_DEBUG as C, type DebugResponse } from '../../shared/browser-debug.ts';
import { currentDebugScope, requireDebugScope, type DebugScope } from './scope.ts';
/** A request retains its exact native source until settlement. */
interface Pending {
  /** Admitted operation. */ scope?: DebugScope;
  /** Owned renderer. */ page: NativePage;
  /** Native frame. */ frame: WebFrameMain;
  /** Preload generation. */ document?: string;
  /** Deadline cleanup. */ timer: ReturnType<typeof setTimeout>;
  /** Successful reply. */ resolve(value: DebugResponse): void;
  /** Explicit failure. */ reject(error: Error): void;
}
const pending = new Map<string, Pending>();
let installed = false;
/** The composition root installs this before creating web tabs. */
export function useDebugChannel(ipc: Pick<IpcMain, 'on'>): void {
  if (installed) return;
  installed = true;
  ipc.on(C.RESPONSE, (event: IpcMainEvent, response: DebugResponse) => receiveDebugReply(event, response));
}
/** A native frame must still belong to the same immutable owned tab. */
export function requireDebugFrame(page: NativePage, frame: WebFrameMain): void {
  if (page.webContents.isDestroyed() || frame.detached || !page.webContents.mainFrame.framesInSubtree.includes(frame))
    throw Error('Frame changed or closed. Read the page again with frames:true.');
}
/** Replies from another renderer or frame cannot settle this request. */
function receiveDebugReply(event: IpcMainEvent, response: DebugResponse): void {
  const call = response && pending.get(response.id);
  if (!call || event.sender !== call.page.webContents || event.senderFrame !== call.frame) return;
  clearDebugPending(response.id, call);
  try {
    validateDebugReply(call, response);
    call.resolve(response);
  } catch (why) {
    call.reject(why as Error);
  }
}
/** A late reply cannot borrow a subsequent operation's authorization. */
function validateDebugReply(call: Pending, response: DebugResponse): void {
  requireDebugScope(call.scope);
  requireDebugFrame(call.page, call.frame);
  if (call.document && response.document !== call.document) throw Error('Frame document changed');
  if (!response.ok) throw Error(response.error || 'The frame could not execute that operation');
}
/** Settlement removes the timer and capability together. */
function clearDebugPending(id: string, call: Pending): void {
  pending.delete(id);
  clearTimeout(call.timer);
}
/** Fixed operations cross this boundary; user source has its explicit page-world eval operation. */
export function debugRequest(
  page: NativePage,
  frame: WebFrameMain,
  kind: string,
  params: Record<string, unknown> = {},
  document?: string,
): Promise<DebugResponse> {
  requireDebugScope();
  requireDebugFrame(page, frame);
  if (pending.size >= C.MAX_PENDING) return Promise.reject(Error('Browser inspection capacity exceeded'));
  const id = randomUUID(),
    scope = currentDebugScope();
  return new Promise((resolve, reject) =>
    startDebugRequest(id, { page, frame, document, scope, resolve, reject }, kind, params),
  );
}
/** Native send and timeout share one cleanup path, including synchronous send failure. */
function startDebugRequest(
  id: string,
  input: Omit<Pending, 'timer'>,
  kind: string,
  params: Record<string, unknown>,
): void {
  const timer = setTimeout(() => expireDebugRequest(id, kind), debugRequestWait(input.scope, kind));
  const call = { ...input, timer };
  pending.set(id, call);
  try {
    input.frame.send(C.REQUEST, { id, document: input.document, kind, params });
  } catch (why) {
    clearDebugPending(id, call);
    input.reject(why as Error);
  }
}
/** Requests cannot outlive the finite operation that admitted them. */
function debugRequestWait(scope: DebugScope | undefined, kind: string): number {
  return Math.max(1, Math.min(kind === 'eval' ? C.EVAL_MS : C.REQUEST_MS, (scope?.deadline ?? Infinity) - Date.now()));
}
/** Evaluation may have side effects even if its reply misses the deadline. */
function expireDebugRequest(id: string, kind: string): void {
  const call = pending.get(id);
  if (!call) return;
  clearDebugPending(id, call);
  call.reject(
    Error(
      kind === 'eval'
        ? 'Evaluation timed out. It may have run; inspect before retrying.'
        : 'Frame inspection timed out',
    ),
  );
}
