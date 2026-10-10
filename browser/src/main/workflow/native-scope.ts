/** Exact native frame ownership for workflow locators, including out-of-process child documents. */
import type { WebFrameMain } from 'electron';
import type { DriverTab, PageDriver } from '../actions/driver.ts';
import { evaluateFrame } from '../native/index.ts';
/** Browser-owned child token; never derive frame identity from names, indices or URLs. */
interface OwnerFrame extends WebFrameMain {
  /** Identity in its parent renderer. */ _oyaOwnerFrameToken?: () => string;
}
/** One immutable frame-selector hop. */
interface Hop {
  /** Parent native frame. */ parent: WebFrameMain;
  /** Exact selected child. */ child: OwnerFrame;
  /** Unique owner selector. */ selector: string;
  /** Parent-renderer identity. */ token: string;
}
/** Coordinates in the current frame's CSS viewport. */
export interface NativePoint {
  /** Horizontal coordinate. */ x: number;
  /** Vertical coordinate. */ y: number;
}
/** A resolved frame chain remains valid only while every native owner identity remains unchanged. */
export class NativeScope {
  /** Pinned run tab. */ readonly tab: DriverTab;
  /** Exact native child, or the root for unframed steps. */ readonly frame: WebFrameMain;
  /** Root native identity at resolution time. */ private readonly top: WebFrameMain;
  /** Isolated main-frame evaluator, also installs trusted analyzer support. */ private readonly driver: PageDriver;
  /** Immutable native ancestry. */ private readonly hops: Hop[];
  /** Store frame graph independently of the application active tab. */
  constructor(driver: PageDriver, tab: DriverTab, hops: Hop[]) {
    this.driver = driver;
    this.tab = tab;
    this.hops = hops;
    this.top = tab.view.webContents.mainFrame;
    this.frame = hops.at(-1)?.child || this.top;
  }
  /** A replaced tab/frame cannot inherit this scope. */
  check(): void {
    if (this.tab.view.webContents.isDestroyed() || this.tab.view.webContents.mainFrame !== this.top)
      throw Error('Workflow document changed');
    if (
      this.hops.some(
        (h) => h.child.detached || h.child.parent !== h.parent || h.child._oyaOwnerFrameToken?.() !== h.token,
      )
    )
      throw Error('Workflow frame changed');
  }
  /** Every isolated native evaluation is checked before and after its asynchronous boundary. */
  async evaluate<T>(code: string): Promise<T> {
    this.check();
    const result = this.hops.length
      ? await evaluateFrame(this.frame, code)
      : await this.driver.deps.worldEval(this.tab.view, code);
    this.check();
    return result as T;
  }
  /** Cleanup never reinstalls the analyzer or reads website data after run ownership ends. */
  async release(key: string): Promise<void> {
    if (!this.frame || this.frame.detached) return;
    await evaluateFrame(
      this.frame,
      `(()=>{const key=${JSON.stringify(key)};globalThis[key]?.pointerCleanup?.();delete globalThis[key];})()`,
    );
  }
  /** Translate a child hit through every exact owner and refuse parent overlays. */
  async point(point: NativePoint, scroll = false): Promise<NativePoint> {
    for (const hop of [...this.hops].reverse()) point = await parentPoint(hop, point, scroll);
    this.check();
    const zoom = this.tab.view.webContents.getZoomFactor();
    if (!Number.isFinite(zoom) || zoom <= 0) throw Error('Native workflow zoom unavailable');
    return { x: point.x * zoom, y: point.y * zoom };
  }
}
/** Match one unique DOM owner to its exact native child; duplicate URLs never matter. */
async function child(parent: WebFrameMain, selector: string): Promise<Hop> {
  const code = `(() => { const s=${JSON.stringify(selector)}, nodes=document.querySelectorAll(s); if(nodes.length!==1 || !nodes[0].matches('iframe,frame'))throw Error('Workflow frame owner is missing or ambiguous'); return globalThis.__oyaNativeRecording?.ownerToken(s); })()`;
  const token = await evaluateFrame(parent, code);
  if (typeof token !== 'string' || !token) throw Error('Native workflow frame owner capability unavailable');
  const found = (parent.frames as OwnerFrame[]).filter((f) => f._oyaOwnerFrameToken?.() === token);
  if (found.length !== 1 || parent.detached) throw Error('Workflow frame changed');
  return { parent, child: found[0], selector, token };
}
/** Resolve frame locators in their recorded order through browser-owned topology. */
export async function workflowScope(driver: PageDriver, tab: DriverTab, selectors: string[]): Promise<NativeScope> {
  const hops: Hop[] = [];
  let parent = tab.view.webContents.mainFrame;
  for (const selector of selectors) {
    const hop = await child(parent, selector);
    hops.push(hop);
    parent = hop.child;
  }
  return new NativeScope(driver, tab, hops);
}
/** Parent-coordinate projection checks the owner still maps to the same native document. */
async function parentPoint(hop: Hop, point: NativePoint, scroll: boolean): Promise<NativePoint> {
  const params = JSON.stringify({ selector: hop.selector, token: hop.token, point, scroll });
  return evaluateFrame(hop.parent, `(${PROJECT})(${params})`) as Promise<NativePoint>;
}
/** Axis-aligned CSS scaling is accounted for; non-invertible or rotated surfaces are explicitly refused. */
const PROJECT = `({selector,token,point,scroll})=>{
  if(globalThis.__oyaNativeRecording?.ownerToken(selector)!==token)throw Error('Workflow frame owner changed');
  const owner=document.querySelector(selector); if(!owner)throw Error('Workflow frame removed');
  if(scroll)owner.scrollIntoView({block:'nearest',inline:'nearest',behavior:'instant'});
  for(let n=owner;n;n=n.parentElement){const t=getComputedStyle(n).transform;if(t!=='none'){const m=new DOMMatrix(t);if(!m.is2D || m.b || m.c || m.a<=0 || m.d<=0)throw Error('Unsupported rotated native frame input');}}
  const r=owner.getBoundingClientRect(), sx=r.width/owner.offsetWidth, sy=r.height/owner.offsetHeight;
  const x=r.left+(owner.clientLeft+point.x)*sx, y=r.top+(owner.clientTop+point.y)*sy;
  if(!Number.isFinite(x)||!Number.isFinite(y)||x<r.left||x>=r.right||y<r.top||y>=r.bottom)throw Error('Workflow frame point outside owner');
  const hit=document.elementFromPoint(x,y);if(hit!==owner&&!owner.contains(hit))throw Error('Workflow frame is covered');
  return {x,y};
}`;
