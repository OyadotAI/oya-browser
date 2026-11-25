/** A pre-page capture listener associates actual WindowProxy sources with exact owner nodes. */
import { BROWSER_DEBUG as C } from '../shared/browser-debug.ts';
import { debugOwners } from './debug-geometry.ts';
/** Resolve the one armed native request; author scripts cannot install or replace this callback. */
interface Challenge {
  /** Native result callback. */ resolve(value: string): void;
  /** Failure callback. */ reject(error: Error): void;
  /** Expiry owner. */ timer: ReturnType<typeof setTimeout>;
}
/** One registry exists per isolated preload document and dies with that renderer document. */
export class DebugOwners {
  /** Exact nodes, never URL/name selectors. */ private readonly nodes = new Map<string, HTMLIFrameElement>();
  /** A child challenge is consumed synchronously before page listeners can replay it. */ private readonly pending =
    new Map<string, Challenge>();
  /** One-use results await native collection. */ private readonly results = new Map<string, Promise<string>>();
  /** Monotonic document-local owner identities. */ private next = 0;
  /** Resolved owner observations are consumed without waiting on unrelated candidate parents. */ private readonly ready =
    new Map<string, string>();
  /** Install before author scripts; late installation is never used as a fallback. */
  constructor() {
    addEventListener('message', (event) => this.receive(event), { capture: true });
  }
  /** Native main arms a single-use challenge before instructing the exact child to send it. */
  arm(nonce: string): void {
    if (!nonce || this.results.size >= C.MAX_PENDING || this.results.has(nonce))
      throw Error('Frame challenge capacity exceeded');
    const result = new Promise<string>((resolve, reject) => this.start(nonce, resolve, reject));
    this.results.set(nonce, result);
    void result.catch(() => {});
  }
  /** Expiration cleans abandoned requests even when native collection never arrives. */
  private start(nonce: string, resolve: Challenge['resolve'], reject: Challenge['reject']): void {
    const timer = setTimeout(() => this.cancel(nonce), C.OWNER_MS);
    this.pending.set(nonce, { resolve, reject, timer });
  }
  /** Separate collection lets native main await arming before the child is signalled. */
  async collect(nonce: string): Promise<string> {
    const result = this.results.get(nonce);
    if (!result) throw Error('Frame challenge is missing or already consumed');
    try {
      return await result;
    } finally {
      this.results.delete(nonce);
      this.ready.delete(nonce);
    }
  }
  /** Only trusted native postMessage delivery from the real child window can consume a challenge. */
  private receive(event: MessageEvent): void {
    if (!event.isTrusted || event.data?.type !== C.MESSAGE || typeof event.data.nonce !== 'string') return;
    const challenge = this.pending.get(event.data.nonce);
    if (!challenge) return;
    this.pending.delete(event.data.nonce);
    const owners = debugOwners().filter((node) => node.contentWindow === event.source);
    if (owners.length !== 1) return challenge.reject(Error('Frame owner is missing or ambiguous'));
    event.stopImmediatePropagation();
    const owner = this.remember(owners[0]);
    this.ready.set(event.data.nonce, owner);
    challenge.resolve(owner);
  }
  /** Nonblocking native discovery never waits on every unrelated candidate parent. */
  peek(nonce: string): string | null {
    return this.ready.get(nonce) ?? null;
  }
  /** Retire unused candidate challenges after exact owner discovery or navigation. */
  cancel(nonce: string): void {
    const pending = this.pending.get(nonce);
    if (pending) {
      clearTimeout(pending.timer);
      pending.reject(Error('Frame challenge retired'));
    }
    this.pending.delete(nonce);
    this.results.delete(nonce);
    this.ready.delete(nonce);
  }
  /** An owner reference stays attached to this exact connected DOM node. */
  remember(node: HTMLIFrameElement): string {
    for (const [id, known] of this.nodes) {
      if (!known.isConnected) this.nodes.delete(id);
      else if (known === node) return id;
    }
    if (this.nodes.size >= C.MAX_FRAMES) throw Error('Frame owner limit exceeded');
    const id = `o-${++this.next}`;
    this.nodes.set(id, node);
    return id;
  }
  /** No selector or origin fallback can retarget an old owner reference. */
  get(id: string): HTMLIFrameElement {
    const node = this.nodes.get(id);
    if (!node?.isConnected) throw Error('Frame owner changed or closed');
    return node;
  }
}
