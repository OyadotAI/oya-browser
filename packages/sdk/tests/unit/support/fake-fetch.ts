/**
 * A fetch stand-in for the SDK tests: it records every request and answers
 * from a route table, so no test touches the network. Tests import the built
 * package (dist/), the same code users get.
 */
import { mock } from 'node:test';
import { Oya } from '../../../dist/index.js';

export const BASE = 'https://oya.test';

/** One request the SDK made. */
export interface Call {
  method: string;
  path: string;
  headers: Record<string, string>;
  body: unknown;
}

/** An answer: a JSON body, and a status (200 by default). */
export type Reply = { status?: number; body?: unknown; text?: string };
/** Answers one request; a function may inspect it, or throw to fail the fetch. */
export type Route = Reply | ((call: Call) => Reply);

/** A fake fetch. `routes` maps "METHOD /path" to an answer, or to a queue of answers used in turn. */
export function fakeFetch(routes: Record<string, Route | Route[]> = {}) {
  const calls: Call[] = [];
  const fetch = async (url: string, init: RequestInit) => {
    const call: Call = {
      method: String(init.method),
      path: url.slice(BASE.length),
      headers: init.headers as Record<string, string>,
      body: init.body === undefined ? undefined : JSON.parse(String(init.body)),
    };
    calls.push(call);
    const reply = pick(routes, `${call.method} ${call.path}`);
    const answer = typeof reply === 'function' ? reply(call) : reply;
    const text = answer.text ?? (answer.body === undefined ? '' : JSON.stringify(answer.body));
    return new Response(text, { status: answer.status ?? 200 });
  };
  return { fetch: fetch as typeof globalThis.fetch, calls };
}

/** The route for a key; a queue answers in order and repeats its last entry. */
function pick(routes: Record<string, Route | Route[]>, key: string): Route {
  const route = routes[key];
  if (route === undefined) return { status: 404, body: { error: `no route for ${key}` } };
  if (!Array.isArray(route)) return route;
  return route.length > 1 ? route.shift()! : route[0];
}

/** An Oya client wired to a fake fetch. */
export function client(routes: Record<string, Route | Route[]> = {}) {
  const fake = fakeFetch(routes);
  return { oya: new Oya({ apiKey: 'k-test', baseUrl: BASE, fetch: fake.fetch }), calls: fake.calls };
}

/** Lets pending promise callbacks run. */
export const flush = () => new Promise<void>((r) => setImmediate(r));

/** Advances fake timers step by step, letting async work run between steps, until `done` settles. */
export async function tickUntil(done: Promise<unknown>, stepMs: number, maxSteps = 100): Promise<void> {
  let settled = false;
  done.then(
    () => (settled = true),
    () => (settled = true),
  );
  for (let i = 0; i < maxSteps && !settled; i++) {
    for (let j = 0; j < 5; j++) await flush();
    mock.timers.tick(stepMs);
  }
  for (let j = 0; j < 5; j++) await flush();
}
