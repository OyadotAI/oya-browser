/**
 * Isolated world.
 *
 * The analyzer runs in its own JS world, not the page's. The page can then
 * neither see our globals (window.analyzePage was a one-line, 100%-precision
 * identifier for this product) nor reach into them. The name is randomised per
 * process so it is not a constant to match on either.
 *
 * An isolated world shares the DOM but has its own globals, so it also gets the
 * UNPATCHED getBoundingClientRect, which is why the analyzer no longer needs a
 * flag to switch the fingerprint noise off while it measures.
 */
import crypto from 'node:crypto';
import type { PageView } from './cdp.ts';
import { WORLD_ATTR_BYTES } from './constants.ts';

/** Sends one CDP command to a view and answers its result (cdp() in cdp.ts). */
export type CdpSend = <T = unknown>(view: PageView, method: string, params?: object) => Promise<T>;

/** What the world needs: a way to drive the view, the analyzer and the world's name. */
export interface WorldDeps {
  /** Drives the view over CDP. */
  cdp: CdpSend;
  /** The analyzer's source, with `__OYA_ATTR__` and `__OYA_RECORD__` holes. */
  analyzerScript: string;
  /** The isolated world's random name. */
  worldName: string;
}

/** How ensure() behaves. */
export interface EnsureOptions {
  /** Rebuild the world even when the view has one (a new document arrived). */
  force?: boolean;
}

/** How evaluate() behaves. */
export interface EvalOptions {
  /** Retry once in a fresh world when the context was lost to a navigation. */
  retry?: boolean;
}

/** A remote object as CDP describes it. */
interface RemoteObject {
  /** The value, when returned by value. */
  value?: unknown;
  /** A thrown value's text, such as "TypeError: x". */
  description?: string;
}

/** Page.getFrameTree's answer, as far as it is read. */
interface FrameTree {
  /** The main frame's node. */
  frameTree: FrameNode;
}

/** A node of the frame tree. */
interface FrameNode {
  /** The frame itself. */
  frame: Frame;
}

/** A frame, as far as it is read. */
interface Frame {
  /** The frame's id. */
  id: string;
}

/** Page.createIsolatedWorld's answer. */
interface IsolatedWorld {
  /** The new world's execution context. */
  executionContextId: number;
}

/** Why an evaluation in the world threw. */
interface WorldException {
  /** The thrown value, described. */
  exception?: RemoteObject;
  /** The exception's short message. */
  text?: string;
}

/** What Runtime.evaluate answers in the world. */
interface WorldEvaluation {
  /** The value, when the expression returned one. */
  result?: RemoteObject;
  /** Why it threw, when it threw. */
  exceptionDetails?: WorldException;
}

/** The message of a thrown value, or '' when it has none. */
const messageOf = (err: unknown): string => (err instanceof Error ? err.message : '');

/** The evaluation's value, or its exception as an Error. */
function worldValue<T>(res: WorldEvaluation): T {
  const details = res.exceptionDetails;
  if (details) throw new Error(details.exception?.description || details.text || 'Evaluation failed');
  return res.result?.value as T;
}

/** The analyzer's isolated world in each view, created on first use and rebuilt when lost. */
export class World {
  /** Drives the view over CDP. */
  private readonly cdp: CdpSend;
  /** The analyzer's source. */
  private readonly analyzerScript: string;
  /** The isolated world's name. */
  private readonly worldName: string;
  /** Each view's world: its execution context id. */
  private readonly contexts = new WeakMap<PageView, number>();

  /** Takes the CDP sender, the analyzer and the world's name. */
  constructor(deps: WorldDeps) {
    this.cdp = deps.cdp;
    this.analyzerScript = deps.analyzerScript;
    this.worldName = deps.worldName;
  }

  /**
   * Create (or recreate) the isolated world for this view's main frame and load
   * the analyzer into it. Page.createIsolatedWorld returns the context id
   * directly, so this needs no Runtime.enable, that domain is a detection
   * vector. Only an active recording enables it to receive captured events.
   */
  async ensure(view: PageView, { force = false }: EnsureOptions = {}): Promise<number> {
    const known = this.contexts.get(view);
    if (!force && known !== undefined) return known;
    const executionContextId = await this.createContext(view);
    this.contexts.set(view, executionContextId);
    await this.loadAnalyzer(view, executionContextId);
    return executionContextId;
  }

  /**
   * Evaluate in the isolated world. Retries once against a fresh world, because
   * a navigation between calls invalidates the context id.
   */
  async evaluate<T = unknown>(view: PageView, expression: string, { retry = true }: EvalOptions = {}): Promise<T> {
    const params = { expression, contextId: await this.ensure(view), returnByValue: true, awaitPromise: true };
    let res: WorldEvaluation;
    try {
      res = await this.cdp<WorldEvaluation>(view, 'Runtime.evaluate', params);
    } catch (err) {
      return this.evaluateFresh<T>(view, expression, err, retry);
    }
    return worldValue<T>(res);
  }

  /** A lost context (a navigation between calls) gets one retry in a fresh world; anything else rethrows. */
  private async evaluateFresh<T>(view: PageView, expression: string, err: unknown, retry: boolean): Promise<T> {
    if (!retry || !/context|Cannot find/i.test(messageOf(err))) throw err;
    await this.ensure(view, { force: true });
    return this.evaluate<T>(view, expression, { retry: false });
  }

  /** A new isolated world on the view's main frame; its execution context id. */
  private async createContext(view: PageView): Promise<number> {
    const { frameTree } = await this.cdp<FrameTree>(view, 'Page.getFrameTree');
    const params = { frameId: frameTree.frame.id, worldName: this.worldName, grantUniveralAccess: true };
    const world = await this.cdp<IsolatedWorld>(view, 'Page.createIsolatedWorld', params);
    return world.executionContextId;
  }

  /** Runs the analyzer in a fresh world, under a tag attribute of its own. */
  private async loadAnalyzer(view: PageView, contextId: number): Promise<void> {
    // A fresh tag attribute per document, so the marks the analyzer leaves on the
    // DOM are not a constant any MutationObserver can match on.
    const attr = 'data-' + crypto.randomBytes(WORLD_ATTR_BYTES).toString('hex');
    await this.cdp(view, 'Runtime.evaluate', {
      // RecordingChannel arms new documents while a recording is active.
      expression: this.analyzerScript.replace('__OYA_ATTR__', attr).replace('__OYA_RECORD__', 'false'),
      contextId,
      returnByValue: true,
    });
  }
}
