/**
 * Frame paths: which chain of iframes a recorded step happened in, as CSS
 * selectors for each frame's owner element, so replay can find it again.
 */
import { RECORDING } from '../constants.ts';

/** Sends one CDP command and resolves with its result, untyped JSON each caller reads as the shape it asked for. */
export type Send = (method: string, params?: object) => Promise<unknown>;

/** The owner element's node, as DOM.describeNode reports it. */
interface OwnerNode {
  /** Its tag name, lower case. */
  localName?: string;
  /** CDP's flat [name, value, ...] attribute list. */
  attributes?: string[];
}

/** A frame, as far as its id. */
export interface FrameInfo {
  /** The frame's id. */
  id: string;
}

/** What Page.getFrameTree answers. */
export interface FrameTreeResult {
  /** The page's frame tree. */
  frameTree: FrameTree;
}

/** What DOM.getFrameOwner answers. */
interface FrameOwner {
  /** The owner element's node. */
  backendNodeId: number;
}

/** What DOM.describeNode answers. */
interface Described {
  /** The node. */
  node: OwnerNode;
}

/** A frame and its children, as Page.getFrameTree reports them. */
export interface FrameTree {
  /** The frame itself. */
  frame: FrameInfo;
  /** Its child frames. */
  childFrames?: FrameTree[];
}

/** Attributes that identify a frame's owner element, most stable first. */
const FRAME_KEYS = ['data-testid', 'id', 'name', 'title', 'src'];

/**
 * A frame's address as a selector: its path, since the host and query of an
 * embed are often made up per load (MDN's `<uuid>.mdnplay.dev/runner.html?uuid=…`).
 */
function srcSelector(tag: string, src: string): string {
  try {
    const url = new URL(src);
    if (url.pathname.length > 1) return `${tag}[src*=${JSON.stringify(url.pathname)}]`;
  } catch {
    // Not a URL: matched whole below.
  }
  return `${tag}[src=${JSON.stringify(src)}]`;
}

/** The frame ids from the top frame down to `frameId`, or undefined when it is gone. */
function findChain(tree: FrameTree, frameId: string, chain: string[] = []): string[] | undefined {
  if (tree.frame.id === frameId) return chain;
  return (tree.childFrames || []).map((child) => findChain(child, frameId, [...chain, child.frame.id])).find(Boolean);
}

/** CDP's flat [name, value, name, value…] attribute list as an object. */
function attributeMap(list: string[] = []): Record<string, string> {
  const attributes: Record<string, string> = {};
  for (let i = 0; i < list.length; i += RECORDING.ATTRIBUTE_STRIDE) attributes[list[i]] = list[i + 1];
  return attributes;
}

/** A stable CSS selector for the element that owns frame `id`. */
export async function frameSelector(send: Send, id: string): Promise<string> {
  const owner = (await send('DOM.getFrameOwner', { frameId: id })) as FrameOwner;
  const { node } = (await send('DOM.describeNode', { backendNodeId: owner.backendNodeId })) as Described;
  const attributes = attributeMap(node.attributes);
  const key = FRAME_KEYS.find((name) => attributes[name]);
  if (!key) throw new Error('Frame has no stable selector');
  const tag = node.localName || 'iframe';
  if (key === 'src') return srcSelector(tag, attributes.src);
  return `${tag}[${key}=${JSON.stringify(attributes[key])}]`;
}

/** The selectors from the top frame (`topFrameId`) down to `frameId`; [] for the top frame itself. */
export async function framePath(send: Send, topFrameId: string | undefined, frameId?: string): Promise<string[]> {
  if (!frameId || frameId === topFrameId) return [];
  const { frameTree } = (await send('Page.getFrameTree')) as FrameTreeResult;
  const chain = findChain(frameTree, frameId);
  if (!chain) throw new Error('Frame detached');
  const selectors: string[] = [];
  for (const id of chain) selectors.push(await frameSelector(send, id));
  return selectors;
}
