/** Public context identities never alias another document, world or connection. */
import { randomUUID } from 'node:crypto';
import type { RuntimeContext } from './runtime-types.ts';
/** Process-wide monotonic identities complement unguessable unique identities. */
let sequence = 0;
/** Allocate metadata only after the native engine has confirmed the exact context. */
export function runtimeIdentity(context: Omit<RuntimeContext, 'id' | 'uniqueId'>): RuntimeContext {
  if (!Number.isSafeInteger(++sequence)) throw Error('Native context identity space exhausted');
  return { ...context, id: sequence, uniqueId: randomUUID() };
}
