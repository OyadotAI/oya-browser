/** Document-scoped DOM inspection and explicit focus/scroll operations in Oya's reserved world; page globals and debugger APIs are never used. */
import { INSPECTION } from './inspection-constants.ts';
/** Build a bounded, document-local node registry and execute a read against it. */
export function inspectionScript(key: string, base: number, operation: string, params: object): string {
  return `(() => { const key = ${JSON.stringify(key)}, base = ${base}, p = ${JSON.stringify(params)};
    const max = ${INSPECTION.nodes}; let remainingText = ${INSPECTION.text};
    let r = globalThis[key];
    if (!r && ${JSON.stringify(operation)} !== 'document') throw Error('DOM document is unavailable; call DOM.getDocument');
    if (!r) { r = { ids: new WeakMap(), nodes: new Map(), next: base }; Object.defineProperty(globalThis, key, { value:r, configurable:true }); }
    ${REGISTRY_SCRIPT}
    ${operationsScript(operation)} })()`;
}
/** Registry ids are bounded, never reused, and disconnected nodes are not silently retargeted. */
const REGISTRY_SCRIPT = `
    const id = n => { if (!r.ids.has(n)) { if (r.nodes.size >= max) throw Error('DOM node limit exceeded'); const i = ++r.next; r.ids.set(n,i); r.nodes.set(i,n); } return r.ids.get(n); };
    const node = () => { const n = r.nodes.get(p.nodeId); if (!n || (!n.isConnected && n !== document)) throw Error('Stale or foreign DOM node'); return n; };
    const text = value => { remainingText -= value.length; if (remainingText < 0) throw Error('DOM text limit exceeded'); return value; };
    const attrs = n => Array.from(n.attributes || []).flatMap(a => [text(a.name), text(a.value)]);
    const serialize = (n, depth) => {
      const result = { nodeId:id(n), backendNodeId:id(n), nodeType:n.nodeType, nodeName:n.nodeName, localName:n.localName || '', nodeValue:text(n.nodeValue || ''), childNodeCount:n.childNodes.length };
      if (n.nodeType === 1) result.attributes = attrs(n);
      if (n === document) Object.assign(result,{documentURL:document.URL,baseURL:document.baseURI,xmlVersion:''});
      if (depth > 0) result.children = Array.from(n.childNodes, c => serialize(c, depth-1));
      return result;
    };`;
/** Only fixed read operations are injected; selectors and values are encoded, never concatenated as code. */
function operationsScript(operation: string): string {
  if (!Object.hasOwn(OPERATIONS, operation)) throw Error('Unsupported native DOM operation');
  return OPERATIONS[operation];
}

/** Explicit operations run in the reserved world, never by invoking page-world helpers. */
const OPERATIONS: Record<string, string> = {
  focus:
    'const n=node(); if(!(n instanceof HTMLElement) && !(n instanceof SVGElement)) throw Error("Node cannot receive focus"); n.focus(); if(document.activeElement!==n) throw Error("Node did not receive focus"); return {};',
  scroll:
    'const n=node(); if(!(n instanceof Element)) throw Error("Node is not an element"); n.scrollIntoView({block:"nearest",inline:"nearest",behavior:"instant"}); return {};',
  document: 'return {root:serialize(document,p.depth ?? 1)};',
  query: 'const n=node().querySelector(p.selector); return {nodeId:n ? id(n) : 0};',
  queryAll: 'return {nodeIds:Array.from(node().querySelectorAll(p.selector),id)};',
  describe: 'return {node:serialize(node(),p.depth ?? 0)};',
  attributes: 'const n=node(); if(n.nodeType!==1) throw Error("Node is not an element"); return {attributes:attrs(n)};',
  outerHTML: 'const n=node(); return {outerHTML:text((n.outerHTML ?? new XMLSerializer().serializeToString(n)))};',
};
