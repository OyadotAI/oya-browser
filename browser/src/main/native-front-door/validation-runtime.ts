/** Runtime semantics are explicit; unsupported contexts, previews and side-effect promises are never ignored. */
import type { NativeCommand } from './types.ts';
/** Native boundary limits mirror the engine's own independent bounds. */
const MAX_SOURCE = 1_048_576,
  MAX_ARGUMENTS = 128,
  MAX_WORLD_NAME = 1024;
/** Supported argument names by method, deliberately excluding inspector/debugger options. */
export const RUNTIME_PARAMS: Record<string, readonly string[]> = {
  'Page.createIsolatedWorld': ['frameId', 'worldName', 'grantUniveralAccess'],
  'Runtime.enable': [],
  'Runtime.disable': [],
  'Runtime.evaluate': [
    'expression',
    'objectGroup',
    'contextId',
    'uniqueContextId',
    'returnByValue',
    'awaitPromise',
    'silent',
    'userGesture',
    'includeCommandLineAPI',
    'generatePreview',
    'throwOnSideEffect',
  ],
  'Runtime.callFunctionOn': [
    'functionDeclaration',
    'objectId',
    'executionContextId',
    'uniqueContextId',
    'arguments',
    'objectGroup',
    'returnByValue',
    'awaitPromise',
    'silent',
    'userGesture',
    'generatePreview',
    'throwOnSideEffect',
  ],
  'Runtime.getProperties': ['objectId', 'ownProperties', 'accessorPropertiesOnly', 'generatePreview'],
  'Runtime.awaitPromise': ['promiseObjectId', 'returnByValue', 'generatePreview'],
  'Runtime.releaseObject': ['objectId'],
  'Runtime.releaseObjectGroup': ['objectGroup'],
};
/** Methods requiring a string never coerce undefined, arrays or page values. */
const REQUIRED: Record<string, string> = {
  'Runtime.evaluate': 'expression',
  'Runtime.callFunctionOn': 'functionDeclaration',
  'Runtime.getProperties': 'objectId',
  'Runtime.awaitPromise': 'promiseObjectId',
  'Runtime.releaseObject': 'objectId',
  'Runtime.releaseObjectGroup': 'objectGroup',
};
/** Reject the whole request before allocating native handles or executing any script. */
export function validateRuntime({ method, params }: NativeCommand): void {
  if (method === 'Page.createIsolatedWorld') return validateWorld(params);
  if (!Object.hasOwn(RUNTIME_PARAMS, method)) return;
  if (Object.hasOwn(REQUIRED, method)) requireString(params[REQUIRED[method]]);
  for (const [key, value] of Object.entries(params)) validateField(key, value);
  validateSelectors(params);
}
/** Context selection must be unambiguous and cannot combine a receiver with another context. */
function validateSelectors(p: Record<string, unknown>): void {
  const count = ['objectId', 'contextId', 'executionContextId', 'uniqueContextId'].filter(
    (key) => p[key] !== undefined,
  ).length;
  if (count > 1) throw Error('Provide only one native runtime receiver or context selector');
}
/** Optional inspector behavior is accepted only when explicitly inactive. */
function validateField(key: string, value: unknown): void {
  if (['silent', 'userGesture', 'includeCommandLineAPI', 'generatePreview', 'throwOnSideEffect'].includes(key)) {
    if (value !== false) throw Error('Unsupported native runtime option: ' + key);
  } else if (['returnByValue', 'awaitPromise', 'ownProperties', 'accessorPropertiesOnly'].includes(key)) {
    if (typeof value !== 'boolean') throw Error(key + ' must be a boolean');
  } else validateRuntimeValue(key, value);
}
/** Typed argument and context validation never interprets a string as another native handle. */
function validateRuntimeValue(key: string, value: unknown): void {
  if (key === 'arguments') return validateArguments(value);
  if (['contextId', 'executionContextId'].includes(key)) {
    if (!Number.isSafeInteger(value) || Number(value) <= 0) throw Error('Invalid native execution context id');
  } else requireString(value);
}
/** Source, handle and group identifiers are bounded before native transport. */
function requireString(value: unknown): void {
  if (typeof value !== 'string' || value.length > MAX_SOURCE) throw Error('Expected a bounded runtime string');
}
/** Function arguments follow the external value/handle/unserializable union exactly. */
function validateArguments(value: unknown): void {
  if (!Array.isArray(value) || value.length > MAX_ARGUMENTS) throw Error('Invalid native function arguments');
  for (const arg of value) validateArgument(arg);
}
/** An empty argument means undefined; a handle can never be accompanied by a replacement value. */
function validateArgument(value: unknown): void {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw Error('Invalid runtime argument');
  const entries = Object.entries(value);
  if (entries.length > 1) throw Error('Runtime argument must have only one representation');
  for (const [key, item] of entries) validateRepresentation(key, item);
}
/** Every argument uses exactly one explicit native representation. */
function validateRepresentation(key: string, item: unknown): void {
  if (!['value', 'objectId', 'unserializableValue'].includes(key)) throw Error('Unsupported runtime argument');
  if (key !== 'value') requireString(item);
  if (key === 'unserializableValue' && !/^(NaN|-?Infinity|-0|-?\d+n)$/.test(String(item)))
    throw Error('Invalid unserializable value');
}

/** Creation never grants universal origin access or accepts native world numbers. */
function validateWorld(params: Record<string, unknown>): void {
  requireString(params.frameId);
  if (!params.frameId) throw Error('frameId is required');
  validateWorldName(params.worldName);
  if (params.grantUniveralAccess !== undefined && params.grantUniveralAccess !== false)
    throw Error('Unsupported universal access for native isolated worlds');
}

/** World labels are display metadata, not unbounded storage or engine world IDs. */
function validateWorldName(name: unknown): void {
  if (name !== undefined && (typeof name !== 'string' || name.length > MAX_WORLD_NAME))
    throw Error('Expected a bounded isolated world name');
}
