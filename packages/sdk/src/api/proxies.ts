/**
 * `oya.proxies`: proxy exits for your personas. A persona takes one at first
 * connect (by its geo hint) or by `personas.pinProxy`, and keeps it.
 */
import { segment } from '../client.js';
import type { ProxyCreate, ProxyInfo } from '../types/index.js';
import type { HttpRef, ProxyCheck, ProxyCheckList, ProxyList } from './shapes.js';

/** Builds `oya.proxies`. */
export const proxyApi = (http: HttpRef) => ({
  /** Every proxy this key can use, shared ones included. */
  list: async (): Promise<ProxyInfo[]> => (await http().request<ProxyList>('GET', '/api/proxies')).proxies,
  /** Add a proxy. Its credentials are never read back. */
  create: async (proxy: ProxyCreate): Promise<ProxyInfo> => http().request<ProxyInfo>('POST', '/api/proxies', proxy),
  /** Remove one of this key's proxies. */
  remove: async (id: string): Promise<void> => {
    await http().request('DELETE', `/api/proxies/${segment(id)}`);
  },
  /** Dial each proxy and learn its real exit IP. Failing ones cool down and are skipped. */
  check: async (): Promise<Array<ProxyCheck>> =>
    (await http().request<ProxyCheckList>('POST', '/api/proxies/check', {})).results,
});
