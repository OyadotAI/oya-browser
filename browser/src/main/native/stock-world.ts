/**
 * Isolated-world code on stock Electron (desktop) through the tab preload.
 * Stock `executeJavaScriptInIsolatedWorld` waits until the page stops loading, so one
 * hung image or long-poll held every agent read for a whole command timeout. The
 * preload runs the code in the page's process at once. It runs only after main accepts
 * its acknowledgement, so a late one is refused and the stock fallback never runs twice.
 */
import type { IpcMain, IpcMainEvent, IpcMainInvokeEvent, WebContents, WebFrameMain } from 'electron';
import { STOCK_PERSONA } from '../../shared/stock-persona.ts';
import { STOCK_WORLD_ACK_MS, SUBFRAME_RUN_MS } from './constants.ts';
/** A preload's reply. */
interface Reply {
  /** The request it answers. */
  id: number;
  /** The code's value. */
  value?: unknown;
  /** The code's error message. */
  error?: string;
}
/** Where a run goes: a tab and one of its frames. */
interface Target {
  /** The tab. */
  contents: WebContents;
  /** The frame in it. */
  frame: WebFrameMain;
}
/** How a run settles. */
interface Settle {
  /** With the code's value. */
  resolve: (value: unknown) => void;
  /** With an error. */
  reject: (error: Error) => void;
}
/** What the preload is asked to run, besides the request id. */
interface WorldRun {
  /** The isolated world. */
  world: number;
  /** Trusted browser code. */
  code: string;
}
/** How a run is sent. */
export interface RunOptions {
  /** Never fall back to the stock API, which waits for the page to load. */
  preloadOnly?: boolean;
  /** A subframe instead of the tab's main frame. */
  frame?: WebFrameMain;
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
  /** The exact frame it was sent to; a reply from any other frame of the tab is ignored. */
  readonly frame: WebFrameMain;
  /** Settles with the preload's value. */
  private readonly resolve: (value: unknown) => void;
  /** Settles with an error. */
  private readonly reject: (error: Error) => void;
  /** Falls back to the stock API when no acknowledgement arrives. */
  private timer?: ReturnType<typeof setTimeout>;
  /** A subframe run's end: the tab's disposal events do not cover a frame that navigated or went away. */
  private deadline?: ReturnType<typeof setTimeout>;
  /** Fails a subframe run that outlived its deadline. */
  private readonly late = () => this.fail(new Error('The frame did not finish'));
  /** The disposal listener, removed once the run settles. */
  private readonly gone = () => this.fail(new Error('Render frame was disposed before the run finished'));
  /** Arms the disposal listeners on the tab. */
  constructor(target: Target, { resolve, reject }: Settle) {
    this.contents = target.contents;
    this.frame = target.frame;
    this.resolve = resolve;
    this.reject = reject;
    for (const name of DISPOSING) this.contents.on(name as 'destroyed', this.gone);
    if (this.frame !== this.contents.mainFrame) this.deadline = setTimeout(this.late, SUBFRAME_RUN_MS);
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
    clearTimeout(this.deadline);
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
/** The run `id`, only when the reply came from the exact frame of the tab it was sent to: a sibling or the parent cannot answer for it. */
function from(event: IpcMainEvent | IpcMainInvokeEvent, id: number): Run | undefined {
  const run = pending.get(id);
  return run && run.contents === event.sender && event.senderFrame === run.frame ? run : undefined;
}
/**
 * Runs `code` in isolated world `world` of one frame of the tab (its main frame unless
 * `frame` names a subframe) without waiting for the page to finish loading. `preloadOnly`
 * never falls back to the stock API, which waits for the load: a caller asking whether
 * the renderer answers at all (the Reload probe, tabs/tab-recovery.ts) must not mistake
 * a slow load for silence. A subframe is always preload-only: the stock API reaches the
 * main frame alone, so a fallback there would run the code in the wrong document.
 */
export function runInWorld<T>(contents: WebContents, world: number, code: string, opts: RunOptions = {}): Promise<T> {
  const frame = opts.frame ?? contents.mainFrame;
  const main = frame === contents.mainFrame;
  const stock = (): Promise<T> => contents.executeJavaScriptInIsolatedWorld(world, [{ code }]);
  if (!installed) return main ? stock() : Promise.reject(new Error(SUBFRAME_UNAVAILABLE));
  return viaPreload<T>({ contents, frame }, { world, code }, (resolve, reject) =>
    fallbackFor(main, opts, () => void stock().then(resolve, reject), reject),
  );
}

/** A run through the frame's preload; `fallbackOf` says what happens when the preload stays silent. */
function viaPreload<T>(
  target: Target,
  request: Pick<WorldRun, 'world' | 'code'>,
  fallbackOf: (resolve: (value: T) => void, reject: (e: Error) => void) => (() => void) | null,
): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const run = new Run(target, { resolve: resolve as (value: unknown) => void, reject });
    dispatchRun(run, fallbackOf(resolve, reject), request);
  });
}

/** Sends the run to its frame's preload, arming `fallback` for when no acknowledgement comes. */
function dispatchRun(run: Run, fallback: (() => void) | null, request: Pick<WorldRun, 'world' | 'code'>): void {
  if (fallback) run.awaitAck(fallback);
  run.frame.send(STOCK_PERSONA.RUN, { id: run.id, ...request });
}

/**
 * What happens when the preload does not acknowledge: the main frame retries through the
 * stock API (unless `preloadOnly`), a subframe fails, since the stock API cannot reach it.
 */
function fallbackFor(main: boolean, opts: RunOptions, retry: () => void, reject: (e: Error) => void) {
  if (!main) return () => reject(new Error(SILENT));
  return opts.preloadOnly ? null : retry;
}

/** Why a subframe run cannot start: the preload channel is not wired yet. */
const SUBFRAME_UNAVAILABLE = 'This frame cannot be read before the tab preload is ready';
/** Why a run with no fallback failed: its frame never acknowledged it. */
const SILENT = 'The frame did not answer';
