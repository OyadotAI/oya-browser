/** DOM arguments are strictly bounded; unsupported shadow-tree and alternate-handle semantics fail closed. */
import type { NativeCommand } from './types.ts';
/** Maximum recursive DOM depth accepted by this compatibility subset. */
const MAX_DEPTH = 32;
/** Never silently reinterpret invalid IDs, negative depth, or shadow-piercing requests. */
export function validateDom({ method, params }: NativeCommand): void {
  if (params.pierce !== undefined && params.pierce !== false) throw Error('Native shadow piercing is unsupported');
  validateDepth(params.depth);
  if (method !== 'DOM.getDocument' && (!Number.isSafeInteger(params.nodeId) || Number(params.nodeId) <= 0))
    throw Error('A valid nodeId is required');
  if (method.startsWith('DOM.querySelector') && typeof params.selector !== 'string')
    throw Error('A selector is required');
}

/** Depth zero is valid; full recursive (-1) trees are deliberately unsupported. */
function validateDepth(depth: unknown): void {
  if (depth === undefined) return;
  if (!Number.isInteger(depth) || Number(depth) < 0 || Number(depth) > MAX_DEPTH)
    throw Error('DOM depth must be an integer from 0 to 32');
}
