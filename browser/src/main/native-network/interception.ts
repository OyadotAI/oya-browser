/** One page's native interception policy changes atomically without releasing previously held requests. */
import { requestMatcher, type RequestMatcher } from './patterns.ts';
import type { NetworkSink, RequestDetails, NetworkObserver } from './types.ts';
/** Native policy input is deliberately narrower than arbitrary protocol commands. */
type Params = Record<string, unknown>;
/** Policy and event sink share a single exact-page interception owner. */
export class NativeInterception {
  /** Original authenticated sink remains stable across policy changes. */
  readonly emit: NetworkSink;
  /** Fully compiled policy, replaced only after validation succeeds. */
  private matcher: RequestMatcher;
  /** Compile before any session observer or paused callback is installed. */
  constructor(emit: NetworkSink, params: Params) {
    this.emit = emit;
    this.matcher = compileInterception(params);
  }
  /** Native request metadata is evaluated after egress and frame attribution checks. */
  accepts(details: RequestDetails): boolean {
    return this.matcher(details);
  }
  /** Previously paused requests retain their individual native continuations. */
  update(params: Params): void {
    this.matcher = compileInterception(params);
  }
}
/** Authentication and response-stage interception cannot be silently activated by a policy update. */
function compileInterception(params: Params): RequestMatcher {
  if (Object.keys(params).some((key) => !['patterns', 'handleAuthRequests'].includes(key)))
    throw Error('Unsupported native interception option');
  if (params.handleAuthRequests !== undefined && params.handleAuthRequests !== false)
    throw Error('Native authentication interception is unavailable');
  return requestMatcher(params.patterns);
}

/** Exact-page cleanup, policy and authorization form one native observer handle. */
type ObserverInputs = [stop: () => void, policy: NativeInterception, allowed: () => boolean];
/** A retained observer cannot update another page or a revoked ownership epoch. */
export function policyObserver(...[stop, policy, allowed]: ObserverInputs): NetworkObserver {
  return Object.assign(stop, {
    update: (params: Params) => {
      if (!allowed()) throw Error('Native interception owner is revoked');
      policy.update(params);
    },
  });
}
