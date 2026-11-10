/**
 * Fakes for the browser's extension API and for fetch, so the popup's rules
 * run under node:test with no browser and no network.
 */

/** What a fake browser starts with. */
export type FakeBrowserOptions = {
  /** The address of the tab the popup opened over. */
  url?: string;
  /** What local storage holds. */
  stored?: Record<string, unknown>;
  /** The cookies the browser holds, returned for any query. */
  cookies?: Record<string, unknown>[];
  /** Whether this site's access was already given. */
  granted?: boolean;
  /** Whether the person allows access when asked. */
  allows?: boolean;
  /** The extension's own address prefix: chrome-extension:// or moz-extension://. */
  origin?: string;
  /** The tab's Firefox container store, when it is in one. */
  cookieStoreId?: string;
  /** Chrome's cookie stores and the tabs each serves. */
  stores?: { id: string; tabIds: number[] }[];
};

/** The id of the tab the popup opened over. */
export const TAB_ID = 7;

/** A fake `chrome` namespace that records what it was asked. */
export function fakeBrowser(options: FakeBrowserOptions = {}) {
  const stored: Record<string, unknown> = { ...options.stored };
  const asked = { cookieQueries: [] as unknown[], permissionRequests: [] as unknown[] };
  const api = {
    runtime: { getURL: (path: string) => `${options.origin ?? 'chrome-extension://abc/'}${path}` },
    tabs: {
      query: async () => [
        { id: TAB_ID, url: options.url ?? 'https://mail.example.com/inbox', cookieStoreId: options.cookieStoreId },
      ],
    },
    storage: {
      local: {
        get: async (keys: string | string[]) =>
          Object.fromEntries(
            [keys]
              .flat()
              .filter((k) => k in stored)
              .map((k) => [k, stored[k]]),
          ),
        set: async (values: Record<string, unknown>) => void Object.assign(stored, values),
      },
    },
    permissions: {
      contains: async () => options.granted ?? false,
      request: async (request: unknown) => (asked.permissionRequests.push(request), options.allows ?? true),
    },
    cookies: {
      getAll: async (query: unknown) => (asked.cookieQueries.push(query), options.cookies ?? []),
      getAllCookieStores: async () => options.stores ?? [{ id: '0', tabIds: [TAB_ID] }],
    },
  };
  return { api, stored, asked };
}

/** One answer the fake server gives: a status and a JSON body. */
export type FakeAnswer = { status: number; body?: unknown };

/** A fake fetch answering by "METHOD /path" (query string dropped), recording every request. */
export function fakeFetch(answers: Record<string, FakeAnswer>) {
  const requests: { method: string; url: string; headers: Record<string, string>; body?: unknown }[] = [];
  const fetchFn = async (url: string, init: { method: string; headers: Record<string, string>; body?: string }) => {
    requests.push({ ...init, url, body: init.body && JSON.parse(init.body) });
    const answer = answers[`${init.method} ${new URL(url).pathname}`] ?? { status: 404, body: {} };
    return { ok: answer.status < 400, status: answer.status, json: async () => answer.body ?? {} };
  };
  return { fetchFn, requests };
}

/** A fetch that never reaches the server. */
export const offlineFetch = async () => {
  throw new TypeError('Failed to fetch');
};
