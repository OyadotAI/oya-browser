/**
 * A fake `fetch` for the project switcher's tests: answers by "METHOD path"
 * and records every call, so tests read like the server conversation.
 */
import { vi } from 'vitest';

/** One canned answer: status and JSON body. */
export type Answer = [status: number, body: unknown];

/** Installs a fake fetch; unknown routes answer 404. Returns the routes table and the spy. */
export function stubFetch(routes: Record<string, Answer> = {}) {
  const spy = vi.fn(async (url: string, init?: RequestInit) => {
    const [status, body] = routes[`${init?.method || 'GET'} ${url}`] ?? [404, {}];
    return { ok: status < 400, status, json: async () => body } as Response;
  });
  vi.stubGlobal('fetch', spy);
  return { routes, spy };
}

/** The "METHOD path" of every call so far. */
export const calls = (spy: ReturnType<typeof stubFetch>['spy']) =>
  spy.mock.calls.map(([url, init]) => `${init?.method || 'GET'} ${url}`);
