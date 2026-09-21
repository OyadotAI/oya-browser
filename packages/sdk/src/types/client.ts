/**
 * The options the `Oya` client is constructed with.
 */

/** How to reach the control plane, and as whom. */
export interface OyaOptions {
  /** Defaults to OYA_API_KEY. */
  apiKey?: string;
  /** Defaults to OYA_BASE_URL, then https://oyabrowser.com. */
  baseUrl?: string;
  /** Per-request timeout. Navigation gets its own, longer budget. */
  timeoutMs?: number;
  /** A fetch to use instead of the global one, for tests, proxies or old runtimes. */
  fetch?: typeof globalThis.fetch;
}
