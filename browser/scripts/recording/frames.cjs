/**
 * Frame paths: which chain of iframes a recorded step happened in, as CSS
 * selectors for each frame's owner element, so replay can find it again.
 */
const { RECORDING } = require('../constants.cjs');

/** Attributes that identify a frame's owner element, most stable first. */
const FRAME_KEYS = ['data-testid', 'id', 'name', 'title', 'src'];

/**
 * A frame's address as a selector: its path, since the host and query of an
 * embed are often made up per load (MDN's `<uuid>.mdnplay.dev/runner.html?uuid=…`).
 */
function srcSelector(tag, src) {
  try {
    const url = new URL(src);
    if (url.pathname.length > 1) return `${tag}[src*=${JSON.stringify(url.pathname)}]`;
  } catch {}
  return `${tag}[src=${JSON.stringify(src)}]`;
}

/** The frame ids from the top frame down to `frameId`, or undefined when it is gone. */
function findChain(tree, frameId, chain = []) {
  if (tree.frame.id === frameId) return chain;
  return (tree.childFrames || []).map((child) => findChain(child, frameId, [...chain, child.frame.id])).find(Boolean);
}

/** CDP's flat [name, value, name, value…] attribute list as an object. */
function attributeMap(list = []) {
  const attributes = {};
  for (let i = 0; i < list.length; i += RECORDING.ATTRIBUTE_STRIDE) attributes[list[i]] = list[i + 1];
  return attributes;
}

/** A stable CSS selector for the element that owns frame `id`. */
async function frameSelector(send, id) {
  const owner = await send('DOM.getFrameOwner', { frameId: id });
  const { node } = await send('DOM.describeNode', { backendNodeId: owner.backendNodeId });
  const attributes = attributeMap(node.attributes);
  const key = FRAME_KEYS.find((name) => attributes[name]);
  if (!key) throw new Error('Frame has no stable selector');
  const tag = node.localName || 'iframe';
  if (key === 'src') return srcSelector(tag, attributes.src);
  return `${tag}[${key}=${JSON.stringify(attributes[key])}]`;
}

/** The selectors from the top frame (`topFrameId`) down to `frameId`; [] for the top frame itself. */
async function framePath(send, topFrameId, frameId) {
  if (!frameId || frameId === topFrameId) return [];
  const { frameTree } = await send('Page.getFrameTree');
  const chain = findChain(frameTree, frameId);
  if (!chain) throw new Error('Frame detached');
  const selectors = [];
  for (const id of chain) selectors.push(await frameSelector(send, id));
  return selectors;
}

module.exports = { framePath, frameSelector };
