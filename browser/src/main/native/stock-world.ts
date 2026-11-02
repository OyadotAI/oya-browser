/**
 * Isolated-world code on stock Electron (desktop) through the tab preload.
 * Stock `executeJavaScriptInIsolatedWorld` waits until the page stops loading, so one
 * hung image or long-poll held every agent read for a whole command timeout. The
 * preload runs the code in the page's process at once. It runs only after main accepts
 * its acknowledgement, so a late one is refused and the stock fallback never runs twice.
 */
import type { IpcMain, IpcMainEvent, IpcMainInvokeEvent, WebContents } from 'electron';
import { STOCK_PERSONA } from '../../shared/stock-persona.ts';
import { STOCK_WORLD_ACK_MS } from './constants.ts';
/** A preload's reply. */
interface Reply {
  /** The request it answers. */
  id: number;
  /** The code's value. */
  value?: unknown;
  /** The code's error message. */
  error?: string;
}
/** Every run in flight, by id. */
const pending = new Map<number, Run>();
/** Request ids, unique for the process. */
let nextId = 0;
/** Tab events that discard the document a run was sent to. */
const DISPOSING = ['did-navigate', 'destroyed', 'render-process-gone'] as const;
/** One run waiting on its preload. */
class Run {
  /** Pairs the preload's reply with this run. */
  readonly id = ++nextId;
  /** The tab it was sent to; replies from any other renderer are ignored. */
  readonly contents: WebContents;
  /** Settles with the preload's value. */
  private readonly resolve: (value: unknown) => void;
  /** Settles with an error. */
  private readonly reject: (error: Error) => void;
  /** Falls back to the stock API when no acknowledgement arrives. */
  private timer?: ReturnType<typeof setTimeout>;
  /** The disposal listener, removed once the run settles. */
  private readonly gone = () => this.fail(new Error('Render frame was disposed before the run finished'));
  /** Arms the disposal listeners on the tab. */
  constructor(contents: WebContents, resolve: (value: unknown) => void, reject: (error: Error) => void) {
    this.contents = contents;
    this.resolve = resolve;
    this.reject = reject;
    for (const name of DISPOSING) contents.on(name as 'destroyed', this.gone);
    pending.set(this.id, this);
  }
  /** Calls `fallback` unless the preload acknowledges first. */
  awaitAck(fallback: () => void): void {
    this.timer = setTimeout(() => (this.release(), fallback()), STOCK_WORLD_ACK_MS);
  }
  /** The preload may run it: the stock fallback must never run it a second time. */
  acknowledged(): true {
    clearTimeout(this.timer);
    return true;
  }
  /** Settles with the preload's answer. */
  answer({ value, error }: Reply): void {
    this.release();
    if (error === undefined) this.resolve(value);
    else this.reject(new Error(error));
  }
  /** Settles with `error`. */
  private fail(error: Error): void {
    this.release();
    this.reject(error);
  }
  /** Forgets the run and its listeners. */
  private release(): void {
    clearTimeout(this.timer);
    for (const name of DISPOSING) this.contents.removeListener(name as 'destroyed', this.gone);
    pending.delete(this.id);
  }
}
/** Wired once by the composition root; until then every run uses the stock API. */
let installed = false;
/** Listens for acknowledgements and results from tab preloads. */
export function useStockWorld(ipc: Pick<IpcMain, 'on' | 'handle'>): void {
  ipc.handle(STOCK_PERSONA.ACK, (event, id: number) => from(event, id)?.acknowledged() ?? false);
  ipc.on(STOCK_PERSONA.RESULT, (event: IpcMainEvent, reply: Reply) => from(event, reply?.id)?.answer(reply));
  installed = true;
}
/** The run `id`, only when the reply came from the main frame of the tab it was sent to, never a subframe. */
function from(event: IpcMainEvent | IpcMainInvokeEvent, id: number): Run | undefined {
  const run = pending.get(id);
  return run && run.contents === event.sender && event.senderFrame === run.contents.mainFrame ? run : undefined;
}
/**
 * Runs `code` in isolated world `world` of the tab's main frame without waiting for
 * the page to finish loading. `preloadOnly` never falls back to the stock API, which
 * waits for the load: a caller asking whether the renderer answers at all (the
 * Reload probe, tabs/tab-recovery.ts) must not mistake a slow load for silence.
 */
export function runInWorld<T>(contents: WebContents, world: number, code: string, preloadOnly = false): Promise<T> {
  const stock = () => contents.executeJavaScriptInIsolatedWorld(world, [{ code }]) as Promise<T>;
  if (!installed) return stock();
  return new Promise<T>((resolve, reject) => {
    const run = new Run(contents, resolve as (value: unknown) => void, reject);
    if (!preloadOnly) run.awaitAck(() => void stock().then(resolve, reject));
    contents.mainFrame.send(STOCK_PERSONA.RUN, { id: run.id, world, code });
  });
}
