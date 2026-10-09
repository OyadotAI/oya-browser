/** Translate explicitly supported external runtime semantics into native V8 operations. */
/** Evaluation and invocation share serialization options, never execution-world fallbacks. */
function options(params: Record<string, unknown>): object {
  return { byValue: params.returnByValue === true, await: params.awaitPromise === true, group: params.objectGroup };
}
/** Argument shape preserves handles and non-JSON primitives without string coercion. */
function argumentsFor(params: Record<string, unknown>): object[] {
  const args = (params.arguments || []) as Record<string, unknown>[];
  return args.map((arg) => ({
    ...('value' in arg ? { value: arg.value } : {}),
    ...('objectId' in arg ? { objectId: arg.objectId } : {}),
    ...('unserializableValue' in arg ? { special: arg.unserializableValue } : {}),
  }));
}
/** Supported fixed engine verbs and their native parameters. */
const COMMANDS: Record<string, (p: Record<string, unknown>) => object> = {
  evaluate: (p) => ({ ...options(p), source: p.expression }),
  invoke: (p) => ({ ...options(p), source: p.functionDeclaration, receiver: p.objectId, arguments: argumentsFor(p) }),
  inspect: (p) => ({ object: p.objectId, accessorsOnly: p.accessorPropertiesOnly === true }),
  await: (p) => ({ ...options(p), object: p.promiseObjectId, await: true }),
  drop: (p) => ({ object: p.objectId }),
  dropGroup: (p) => ({ group: p.objectGroup }),
};
/** Unknown operations fail before crossing the native capability boundary. */
export function runtimeParameters(operation: string, params: Record<string, unknown>): object {
  if (!Object.hasOwn(COMMANDS, operation)) throw Error('Unsupported native runtime operation');
  return JSON.parse(JSON.stringify(COMMANDS[operation](params))) as object;
}

/** Distinct native opcode prevents old engines from silently returning own properties for a chain request. */
export function runtimeOperation(operation: string, params: Record<string, unknown>): string {
  return operation === 'inspect' && params.ownProperties !== true ? 'inspectChain' : operation;
}
