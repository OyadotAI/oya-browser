/** Connection-local domain subscriptions preserve exact target and flat-session ownership. */
import type { NativeBackend, NativeSubscription } from './types.ts';
/** Native domain observer parameters are all bound to one authenticated socket. */
export interface SubscriptionRequest {
  /** Authorized native backend. */
  backend: NativeBackend;
  /** Exact native tab identity. */
  target: string;
  /** Socket-local session namespace. */
  key: string;
  /** Supported observation domain. */
  domain: string;
  /** Domain-specific native policy, never forwarded to an inspector. */
  params?: Record<string, unknown>;
  /** Enable or disable this domain only. */
  enabled: boolean;
  /** Authenticated event sink. */
  emit(method: string, params: Record<string, unknown>): void;
}
/** One socket can enable independent domains without replacing another socket's observers. */
export class NativeSubscriptions {
  /** Native unsubscribe handles grouped by local session, then domain. */
  private readonly sessions = new Map<string, Map<string, NativeSubscription>>();
  /** Enable is idempotent and failed setup never leaves an installed placeholder. */
  set(request: SubscriptionRequest): object {
    const { backend, target, key, domain, enabled, emit } = request;
    const domains = this.sessions.get(key) || new Map<string, () => void>();
    if (!enabled) return this.disable(key, domain);
    if (domains.has(domain)) return update(domains.get(domain)!, request);
    if (!backend.subscribe) throw Error('Native event observation is unavailable');
    domains.set(domain, backend.subscribe(target, domain, emit, request.params));
    this.sessions.set(key, domains);
    return {};
  }
  /** Disable affects just one domain on one exact session. */
  private disable(key: string, domain: string): object {
    const domains = this.sessions.get(key);
    domains?.get(domain)?.();
    domains?.delete(domain);
    if (!domains?.size) this.sessions.delete(key);
    return {};
  }
  /** Detach and target destruction release all domains owned by that session. */
  release(key: string): void {
    const domains = this.sessions.get(key);
    this.sessions.delete(key);
    for (const stop of domains?.values() || []) stop();
  }
  /** Disconnect releases observers without replaying buffered page activity. */
  dispose(): void {
    for (const key of [...this.sessions.keys()]) this.release(key);
  }
}

/** Other domains remain idempotent; Fetch policy changes require an explicit native update capability. */
function update(observer: NativeSubscription, request: SubscriptionRequest): object {
  if (request.domain !== 'Fetch') return {};
  if (!observer.update) throw Error('Native interception policy update is unavailable');
  observer.update(request.params || {});
  return {};
}
