/**
 * Test doubles for the agent, playbooks and settings tests: a browser driven by
 * a scripted driver, an LLM endpoint answered from a script through a stubbed
 * fetch, and a way to call an Express router without opening a socket.
 */
import { mock } from 'node:test';
import { registry } from '../../../src/modules/browsers/registry.ts';
import { keyCache, keyDigest } from '../../../src/modules/auth/keys.ts';

/** How a scripted browser answers one command. */
export type Answer = (action: string, params: any) => any;

/**
 * Registers a browser whose driver answers every command through `answer`
 * (default: `{ ok: true }`), as a CDP browser unless `clientType` says Oya: the
 * server checks each action against what that kind of browser does. Returns
 * the commands it received and a way to disconnect it.
 */
export function scriptedBrowser(
  browserId: string,
  apiKey = 'key-a',
  answer: Answer = () => ({ ok: true }),
  clientType: 'cdp' | 'oya' = 'cdp',
) {
  const calls: { action: string; params: any; timeout?: number }[] = [];
  const driver = {
    send: async (action, params, timeout) => {
      calls.push({ action, params, timeout });
      return answer(action, params);
    },
  };
  registry.add(browserId, { apiKey, name: 'Test', clientType, persona: { id: 'p-1' }, engine: driver });
  return { calls, disconnect: () => registry.remove(browserId), actions: () => calls.map((c) => c.action) };
}

/** A chat-completions reply that calls tools, one `[name, args]` pair per call. */
export const toolReply = (...calls: [string, object][]) => ({
  choices: [
    {
      message: {
        role: 'assistant',
        content: null,
        tool_calls: calls.map(([name, args], i) => ({
          id: `call_${i}_${name}`,
          type: 'function',
          function: { name, arguments: JSON.stringify(args) },
        })),
      },
    },
  ],
  usage: { prompt_tokens: 10, completion_tokens: 5 },
});

/** A chat-completions reply with a final text answer. */
export const textReply = (text: string) => ({
  choices: [{ message: { role: 'assistant', content: text } }],
  usage: { prompt_tokens: 10, completion_tokens: 5 },
});

/**
 * Replaces globalThis.fetch with an LLM endpoint that answers each request with
 * the next reply in the script (the last one repeats). Returns the parsed
 * request bodies it received; `mock.restoreAll()` puts fetch back.
 */
export function stubLlm(replies: any[]) {
  const requests: any[] = [];
  const urls: string[] = [];
  mock.method(globalThis, 'fetch', async (url, init) => {
    urls.push(String(url));
    requests.push(JSON.parse(init.body));
    const reply = replies[Math.min(requests.length - 1, replies.length - 1)];
    return new Response(JSON.stringify(reply), { status: 200, headers: { 'Content-Type': 'application/json' } });
  });
  return { requests, urls };
}

/** Lets `apiKey` through authMiddleware as a stored key, for the duration of a test. */
export function allowKey(apiKey: string) {
  keyCache.add(keyDigest(apiKey));
  return () => keyCache.delete(keyDigest(apiKey));
}

/** A request as a test describes it. */
export type RouteRequest = {
  /** HTTP method. */
  method?: string;
  /** Path and query, relative to where the router is mounted. */
  url: string;
  /** Bearer token sent as the Authorization header. */
  key?: string;
  /** Parsed JSON body. */
  body?: any;
  /** Extra headers. */
  headers?: Record<string, string>;
  /** Extra request fields (ip, rawBody, secure…). */
  extra?: Record<string, any>;
};

/** What the router answered. */
export type RouteResponse = {
  /** HTTP status. */
  status: number;
  /** The JSON (or text) body. */
  body: any;
  /** Headers set with res.set / writeHead. */
  headers: Record<string, string>;
  /** Where a redirect pointed. */
  redirect?: string;
  /** Cookies set, by name. */
  cookies: Record<string, { value: string; options: any }>;
  /** Cookies cleared. */
  cleared: string[];
};

/**
 * Calls an Express router in-process with a fake request and response, and
 * resolves with what it answered. An error passed to `next` (or thrown by an
 * async handler) resolves as its status and message, the way app/api.ts answers it.
 */
export function callRoute(router, { method = 'GET', url, key, body, headers = {}, extra = {} }: RouteRequest) {
  return new Promise<RouteResponse>((resolve) => {
    const out: RouteResponse = { status: 200, body: undefined, headers: {}, cookies: {}, cleared: [] };
    let chunks = '';
    const done = (b) => resolve({ ...out, body: b });
    const res: any = {
      status: (s) => ((out.status = s), res),
      set: (k, v) => ((out.headers[k] = v), res),
      type: (t) => ((out.headers['Content-Type'] = t), res),
      json: (b) => done(b),
      send: (b) => done(b),
      redirect: (to) => ((out.status = 302), (out.redirect = to), done(undefined)),
      cookie: (name, value, options) => ((out.cookies[name] = { value, options }), res),
      clearCookie: (name) => (out.cleared.push(name), res),
      setTimeout: () => res,
      writeHead: (s, h) => ((out.status = s), Object.assign(out.headers, h), res),
      write: (c) => ((chunks += c), true),
      end: (c = '') => done(JSON.parse(chunks + c)),
    };
    const [path, search = ''] = url.split('?');
    const req: any = {
      method,
      url,
      originalUrl: url,
      path,
      query: Object.fromEntries(new URLSearchParams(search)),
      headers: { ...(key ? { authorization: `Bearer ${key}` } : {}), ...headers },
      body,
      socket: { remoteAddress: '127.0.0.1' },
      setTimeout: () => req,
      ...extra,
    };
    router(req, res, (err) => done(err ? ((out.status = err.status || 500), { error: err.message }) : 'unhandled'));
  });
}
