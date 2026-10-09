/** Supported native network semantics are explicit; response interception/auth/overrides are not silently ignored. */
import { requestMatcher, validateFailureReason } from '../native-network/index.ts';
import type { NativeCommand } from './types.ts';
/** Narrow request control and original-response reading surface. */
export const NETWORK_PARAMS: Record<string, readonly string[]> = {
  'Oya.enableNetwork': [],
  'Oya.disableNetwork': [],
  'Network.getResponseBody': ['requestId'],
  'Fetch.enable': ['patterns', 'handleAuthRequests'],
  'Fetch.disable': [],
  'Fetch.continueRequest': ['requestId'],
  'Fetch.failRequest': ['requestId', 'errorReason'],
};
/** Initially intercept all frame-attributed requests; unsupported patterns fail rather than claiming to match them. */
export function validateNetwork({ method, params }: NativeCommand): void {
  if (!Object.hasOwn(NETWORK_PARAMS, method)) return;
  if (['Fetch.continueRequest', 'Fetch.failRequest', 'Network.getResponseBody'].includes(method))
    requireRequestId(params.requestId);
  if (method === 'Fetch.failRequest') validateFailureReason(params.errorReason);
  if (method === 'Fetch.enable') validateFetchEnable(params);
}
/** Request identities are opaque, nonempty strings, never numbers or URL selectors. */
function requireRequestId(value: unknown): void {
  if (typeof value !== 'string' || !value) throw Error('requestId is required');
}

/** Native authentication and response-stage interception are separate unsupported capabilities. */
function validateFetchEnable(params: Record<string, unknown>): void {
  if (params.handleAuthRequests !== undefined && params.handleAuthRequests !== false)
    throw Error('Native authentication interception is unavailable');
  requestMatcher(params.patterns);
}
