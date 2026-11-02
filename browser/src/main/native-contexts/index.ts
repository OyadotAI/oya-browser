/** Public native context ownership and private-session boundaries. */
export { NativeContexts, type ContextDependencies } from './contexts.ts';
export { privateSession, ownSession } from './registry.ts';
export { retireContext } from './cleanup.ts';
