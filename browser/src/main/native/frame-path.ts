/** Resolve replayable owner selectors through native frame topology and isolated DOM identity checks. */
import type { WebFrameMain } from 'electron';
import { evaluateFrame } from './frames.ts';
/** Match native owner identity across light and open shadow DOM, never by child index. */
const OWNER_SELECTOR = `(token) => {
  if (typeof globalThis.__oyaNativeRecording?.ownerToken !== 'function') throw new Error('Native frame owner lookup is unsupported');
  const roots = [document];
  const owners = [];
  for (let i = 0; i < roots.length; i++) {
    for (const node of roots[i].querySelectorAll('*')) {
      if (node.shadowRoot) roots.push(node.shadowRoot);
      if (node.matches('iframe,frame')) owners.push(node);
    }
  }
  for (const owner of owners) {
    for (const key of ['data-testid', 'id', 'name', 'title', 'src']) {
      const value = owner.getAttribute(key);
      if (!value) continue;
      const selector = owner.localName + '[' + key + '=' + CSS.escape(value) + ']';
      const matches = roots.flatMap(root => [...root.querySelectorAll(selector)]);
      if (matches.length !== 1 || matches[0] !== owner) continue;
      if (globalThis.__oyaNativeRecording.ownerToken(selector) === token) return selector;
    }
  }
  throw new Error('Frame has no unique stable selector');
}`;
/** The browser supplies the local or remote token as represented in this frame's parent renderer. */
interface OwnerFrame extends WebFrameMain {
  /** Missing capabilities cannot fall back to window.frames indices. */
  _oyaOwnerFrameToken?: () => string;
}
/** Token identity is browser-owned and cannot be supplied by a website or guessed from a URL. */
function ownerToken(frame: OwnerFrame): string {
  if (!frame._oyaOwnerFrameToken) throw new Error('Native frame owner lookup is unsupported');
  const token = frame._oyaOwnerFrameToken();
  if (!token) throw new Error('Native frame owner is unavailable');
  return token;
}
/** Snapshot each ancestor so navigation or topology changes cannot silently select a replacement. */
interface FrameHop {
  /** Browser-owned parent frame. */
  parent: WebFrameMain;
  /** Exact child whose owner is being resolved. */
  child: WebFrameMain;
  /** Stable direct-child enumeration before isolated DOM evaluation. */
  siblings: WebFrameMain[];
  /** Preserve the native owner identity across every asynchronous hop, not just its own read. */
  token: string;
}
/** Refuse changed frame trees rather than emitting a plausible but wrong locator. */
function unchanged(hop: FrameHop): boolean {
  if (hop.child.detached || hop.parent.detached || hop.child.parent !== hop.parent) return false;
  if (ownerToken(hop.child) !== hop.token) return false;
  const frames = hop.parent.frames;
  return frames.length === hop.siblings.length && frames.every((frame, index) => frame === hop.siblings[index]);
}
/** Resolve one frame owner and validate the browser snapshot on both sides of the asynchronous read. */
async function ownerSelector(hop: FrameHop): Promise<string> {
  if (!unchanged(hop)) throw new Error('Frame tree changed');
  const selector = await evaluateFrame(hop.parent, `(${OWNER_SELECTOR})(${JSON.stringify(hop.token)})`);
  if (!unchanged(hop)) throw new Error('Frame tree changed');
  if (typeof selector !== 'string' || !selector) throw new Error('Frame owner is unavailable');
  return selector;
}
/** Only descendants of the explicitly owned top frame may produce a recording path. */
function ancestry(top: WebFrameMain, frame: WebFrameMain): FrameHop[] {
  const hops: FrameHop[] = [];
  for (let child = frame; child !== top;) {
    const parent = child.parent;
    if (child.detached || !parent) throw new Error('Frame is outside the recording page');
    hops.unshift({ parent, child, siblings: parent.frames, token: ownerToken(child) });
    child = parent;
  }
  return hops;
}
/** Frame selectors remain explicit: ambiguous owner attributes are an error, never a URL guess. */
export async function nativeFramePath(top: WebFrameMain, frame: WebFrameMain): Promise<string[]> {
  if (top.detached || frame.detached) throw new Error('Frame detached');
  const hops = ancestry(top, frame);
  const selectors: string[] = [];
  for (const hop of hops) selectors.push(await ownerSelector(hop));
  if (!hops.every(unchanged)) throw new Error('Frame tree changed');
  return selectors;
}
