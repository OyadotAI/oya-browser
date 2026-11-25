/** Explicit source executes only in the unprivileged page world, never the preload realm. */
import { contextBridge } from 'electron';
import { BROWSER_DEBUG as C } from '../shared/browser-debug.ts';
/** This serialized callback rejects unsupported values at every JSON nesting level. */
function debugJsonReplacer(_key: string, item: unknown): unknown {
  if (['undefined', 'function', 'symbol', 'bigint'].includes(typeof item)) throw Error('Unsupported JSON value');
  if (typeof item === 'number' && !Number.isFinite(item)) throw Error('Nonfinite JSON number');
  if (item && typeof item === 'object' && 'nodeType' in item)
    throw Error('DOM nodes are not JSON; return their properties');
  if (item && typeof (item as Promise<unknown>).then === 'function')
    throw Error('Promise results are unsupported; return synchronous JSON.');
  return item;
}
/** This function and its callback are serialized into the page before invocation. */
function serializeDebugValue(value: unknown, replacer: typeof debugJsonReplacer): string {
  const text = JSON.stringify(value, replacer);
  if (text === undefined) throw Error('Undefined is not a JSON result');
  return text;
}
/** Parse without running, then fence payload declarations and early returns inside their own function. */
function debugEvaluationFunction(expression: unknown): () => string {
  if (
    typeof expression !== 'string' ||
    !expression.trim() ||
    new TextEncoder().encode(expression).length > C.MAX_EXPRESSION
  )
    throw Error('Invalid JavaScript expression');
  const body = `return(\n${expression}\n);`;
  new Function(body);
  return new Function(
    `return (${serializeDebugValue.toString()})((function(){${body}})(),${debugJsonReplacer.toString()});`,
  ) as () => string;
}
/** Result size is checked in the isolated world before parsing or returning structured data. */
export function debugEvaluate(expression: unknown): unknown {
  const text = contextBridge.executeInMainWorld({ func: debugEvaluationFunction(expression) });
  if (typeof text !== 'string' || new TextEncoder().encode(text).length > C.MAX_JSON)
    throw Error('The JSON result exceeded 64 KiB. The expression ran.');
  return JSON.parse(text);
}
