/** The analyzer runs in a browser-owned isolated world, without enabling a debugging domain. */
import { randomBytes } from 'node:crypto';
import type { NativePage } from './page.ts';
import { evaluateFrame, type NativeAgentFrame } from './frames.ts';
import { ANALYZER_WORLD_ID, ANALYZER_ATTRIBUTE_BYTES } from './constants.ts';
/** Source configuration contains no CDP sender or other protocol capability. */
export interface WorldDeps {
  /** Trusted analyzer source bundled with the browser. */
  analyzerScript: string;
  /** Per-process name prevents accidental collisions in the isolated global. */
  worldName: string;
}
/** A caller can explicitly reinstall after a navigation. */
export interface EnsureOptions {
  /** Reinstall even if this document already has an analyzer. */
  force?: boolean;
}
/** A caller can refuse the single navigation-context retry. */
export interface EvalOptions {
  /** Retry only a destroyed execution context, never an arbitrary page exception. */
  retry?: boolean;
}
/** Native isolated execution preserves a separate global while sharing the DOM. */
export class World {
  /** Bundled source and randomized installation marker. */
  private readonly deps: WorldDeps;
  /** No debug endpoint, debugger handle or protocol sender enters this service. */
  constructor(deps: WorldDeps) {
    this.deps = deps;
  }
  /** The marker is document-local, so navigation automatically requires a new installation. */
  async ensure(view: NativePage, { force = false }: EnsureOptions = {}): Promise<number> {
    await this.run(view, this.bootstrap(force));
    return ANALYZER_WORLD_ID;
  }
  /** Evaluate only after the current document owns an initialized analyzer. */
  async evaluate<T = unknown>(view: NativePage, expression: string, { retry = true }: EvalOptions = {}): Promise<T> {
    try {
      return await this.run<T>(view, `${this.bootstrap(false)}\n${expression}`);
    } catch (error) {
      return this.retry<T>(view, expression, error, retry);
    }
  }
  /** A frame discarded by navigation can be retried once; application exceptions are never replayed. */
  private async retry<T>(view: NativePage, expression: string, error: unknown, enabled: boolean): Promise<T> {
    const message = error instanceof Error ? error.message : '';
    if (!enabled || !/execution context was destroyed|render frame was disposed/i.test(message)) throw error;
    return this.evaluate<T>(view, expression, { retry: false });
  }
  /** Initialization and the requested read share one native execution, so navigation cannot split them. */
  private bootstrap(force: boolean): string {
    const marker = JSON.stringify(this.deps.worldName);
    return `if (${force} || !globalThis[${marker}]) { ${this.analyzer()}
 Object.defineProperty(globalThis, ${marker}, {value:true, configurable:true}); } void 0;`;
  }
  /** Random DOM marks are recreated per installation; recording is armed by its own service. */
  private analyzer(): string {
    const attr = 'data-' + randomBytes(ANALYZER_ATTRIBUTE_BYTES).toString('hex');
    return this.deps.analyzerScript.replace('__OYA_ATTR__', attr).replace('__OYA_RECORD__', 'false');
  }
  /** The engine frame API avoids Electron’s load-completion wait; older engines retain their native public API. */
  private async run<T>(view: NativePage, code: string): Promise<T> {
    if (view.webContents.isDestroyed()) throw new Error('View is destroyed');
    const frame = view.webContents.mainFrame as NativeAgentFrame | undefined;
    if (frame?._executeJavaScriptInOyaWorld) return evaluateFrame(frame, code) as Promise<T>;
    return view.webContents.executeJavaScriptInIsolatedWorld(ANALYZER_WORLD_ID, [{ code }]);
  }
}
