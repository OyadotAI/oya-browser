/**
 * A stand-in for the network: globalThis.fetch replaced by a handler that
 * answers from the test, and the calls it saw. Restore it with
 * mock.restoreAll() in afterEach.
 */
import { mock } from 'node:test';

/** One request the stub answered. */
export type FetchCall = { url: string; init: any };

/** Replaces fetch with `handler`; returns the list of calls it receives. */
export function stubFetch(handler: (url: string, init: any) => Response | Promise<Response>) {
  const calls: FetchCall[] = [];
  mock.method(globalThis, 'fetch', async (url: any, init: any = {}) => {
    calls.push({ url: String(url), init });
    return handler(String(url), init);
  });
  return calls;
}

/** A JSON response. */
export const json = (body: unknown, status = 200, headers: Record<string, string> = {}) =>
  new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json', ...headers } });

/** A plain-text response. */
export const text = (body: string, status = 200, headers: Record<string, string> = {}) =>
  new Response(body, { status, headers });

/**
 * Lets pending promise callbacks run, then moves mocked time on by `ms`,
 * `times` times: how a test walks a poll loop that sleeps between tries.
 */
export async function advance(ms: number, times = 1) {
  for (let i = 0; i < times; i++) {
    await new Promise((resolve) => setImmediate(resolve));
    mock.timers.tick(ms);
  }
  await new Promise((resolve) => setImmediate(resolve));
}
