/** Browser-neutral isolated document runtime shared by the Electron and WKWebView adapters. */
import { BROWSER_DEBUG as C, type DebugRequest, type DebugResponse } from '../shared/browser-debug.ts';
import { DebugNodes, debugElement } from './debug-nodes.ts';
import { DebugOwners } from './debug-owners.ts';
import { debugHit, debugProject } from './debug-geometry.ts';
/** Only the browser-owned adapter implements page-world evaluation. */
type Evaluate = (expression: unknown) => unknown;
/** Registration carries identity only, never host execution or input authority. */
interface Registration {
  /** Fixed lifecycle event. */ kind: string;
  /** Isolated generation. */ document: string;
}
/** Native-only registration callback. */
type Register = (event: Registration) => void;
const randomValues = crypto.getRandomValues.bind(crypto);
/** Capture the original generator before author scripts can replace page primitives. */
function generation(): string {
  return Array.from(randomValues(new Uint32Array(C.TOKEN_WORDS))).join('-');
}
/** One isolated document retains exact nodes and owners until navigation retires it. */
export class DebugDocument {
  /** Changes on trusted navigation, even when BFCache restores the same realm. */ private document = generation();
  /** Node capabilities never leave the isolated world. */ private readonly nodes = new DebugNodes();
  /** The earliest capture listener owns parent/child association. */ private readonly owners = new DebugOwners();
  /** Fixed operation map cannot expose arbitrary preload execution. */ private readonly operations: Record<
    string,
    (p: Record<string, unknown>) => unknown
  >;
  /** Install during native document-start initialization, never as a late page-script fallback. */
  /** Browser-owned evaluator. */ private readonly evaluate: Evaluate;
  /** Registration transport. */ private readonly registration: Register;
  constructor(evaluate: Evaluate, registration: Register) {
    this.evaluate = evaluate;
    this.registration = registration;
    this.operations = this.operationMap();
    addEventListener('pagehide', (event) => this.retire(event), { capture: true });
    addEventListener(
      'pageshow',
      (event) => {
        if (event.isTrusted) this.register();
      },
      { capture: true },
    );
  }
  /** A registration contains no page-controlled native frame identity. */
  register(): void {
    this.registration({ kind: 'register', document: this.document });
  }
  /** A stale document reference stays stale after navigation and BFCache restoration. */
  private retire(event: PageTransitionEvent): void {
    if (!event.isTrusted) return;
    this.registration({ kind: 'retire', document: this.document });
    this.document = generation();
    this.nodes.releasePointer();
  }
  /** Requests validate their document before evaluating anything, within the same renderer turn. */
  async execute(request: DebugRequest) {
    try {
      this.admit(request);
      return boundedDebugResponse({
        id: request.id,
        document: this.document,
        ok: true,
        data: await this.operations[request.kind](request.params ?? {}),
      });
    } catch (why) {
      return {
        id: request.id,
        document: this.document,
        ok: false,
        error: String((why as Error)?.message ?? why).slice(0, C.MAX_ERROR),
      };
    }
  }
  /** Only initial metadata can omit a document capability. */
  private admit(request: DebugRequest): void {
    if (request.document !== this.document && (request.document !== undefined || request.kind !== 'metadata'))
      throw Error('Frame changed or closed. Read the page again with frames:true.');
    if (!Object.hasOwn(this.operations, request.kind)) throw Error('Unsupported diagnostic operation');
  }
  /** The native boundary invokes named trusted handlers, never a caller-selected function. */
  private operationMap(): Record<string, (p: Record<string, unknown>) => unknown> {
    return { ...this.documentOperations(), ...this.nodeOperations(), ...this.ownerOperations() };
  }
  /** Metadata is measured in the exact isolated document. */
  private documentOperations() {
    return {
      metadata: () => ({
        document: this.document,
        url: location.href.length > C.MAX_URL ? location.href.slice(0, C.MAX_URL) + '… [URL shortened]' : location.href,
        width: innerWidth,
        height: innerHeight,
        dpr: devicePixelRatio,
      }),
      read: () => this.nodes.read(),
      eval: (p: Record<string, unknown>) => this.evaluate(p.expression),
    };
  }
  /** Native main alone supplies node capability numbers issued by a read. */
  private nodeOperations() {
    return {
      point: (p: Record<string, unknown>) => this.nodes.point(Number(p.element), p.scroll === true),
      type: (p: Record<string, unknown>) => this.nodes.type(Number(p.element), String(p.text ?? ''), p.submit === true),
      arm_pointer: (p: Record<string, unknown>) => this.nodes.armPointer(Number(p.element)),
      pointer_reached: () => this.nodes.pointerReached(),
      release_pointer: () => this.nodes.releasePointer(),
    };
  }
  /** A one-use challenge associates actual WindowProxy sources with parent-private owner nodes. */
  private ownerOperations() {
    return {
      owner_peek: (p: Record<string, unknown>) => this.owners.peek(String(p.nonce)),
      cancel_owner: (p: Record<string, unknown>) => this.owners.cancel(String(p.nonce)),
      arm: (p: Record<string, unknown>) => this.owners.arm(String(p.nonce)),
      owner_result: (p: Record<string, unknown>) => this.owners.collect(String(p.nonce)),
      challenge: (p: Record<string, unknown>) => parent.postMessage({ type: C.MESSAGE, nonce: p.nonce }, '*'),
      hit: (p: Record<string, unknown>) => this.hit(Number(p.x), Number(p.y)),
      project: (p: Record<string, unknown>) => this.project(p),
    };
  }
  /** Only actual browser hits can produce a private frame-owner reference. */
  private hit(x: number, y: number) {
    const hit = debugElement(x, y);
    if (!hit) return { element: null };
    return {
      element: hit.data,
      owner: hit.node.matches('iframe,frame') ? this.owners.remember(hit.node as HTMLIFrameElement) : null,
    };
  }
  /** Projection checks actual interception unless the caller asks only for geometry. */
  private project(p: Record<string, unknown>) {
    const owner = this.owners.get(String(p.owner));
    if (p.scroll === true) owner.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'instant' });
    const point = debugProject(owner, { x: Number(p.x), y: Number(p.y) }, p.inverse === true);
    const hit = p.inverse === true ? debugHit(Number(p.x), Number(p.y)) : debugHit(point.x, point.y);
    if (p.geometry !== true && hit !== owner)
      throw Error('Frame is covered at the requested point. No click was sent.');
    // An agent click inside a frame first gives the frame focus from its parent, as a person's press would.
    if (p.focus === true) owner.focus({ preventScroll: true });
    return point;
  }
}

/** Page-sized results are refused inside the renderer before IPC can allocate them in native main. */
function boundedDebugResponse(response: DebugResponse): DebugResponse {
  if (new TextEncoder().encode(JSON.stringify(response)).length > C.MAX_REPLY_BYTES)
    return {
      id: response.id,
      document: response.document,
      ok: false,
      error: 'Frame diagnostic result exceeded 256 KiB.',
    };
  return response;
}
