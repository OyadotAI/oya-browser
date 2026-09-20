/**
 * Seams for the CLI unit tests: a fake fetch answering from a route table,
 * captured console output, and a process.exit that throws instead of exiting.
 * Nothing here touches the network or the real config file.
 */
import { mock } from 'node:test';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

// Point the saved config at an empty scratch directory before any CLI module reads it.
process.env.OYA_CONFIG_HOME = mkdtempSync(join(tmpdir(), 'oya-cli-test-'));
delete process.env.OYA_API_KEY;
delete process.env.OYA_BASE_URL;

/** The control plane the fake fetch pretends to be. */
export const BASE = 'http://oya.test';

/** One request a command made. */
export interface Call {
  method: string;
  path: string;
  body: unknown;
}

/** Marks a route answer that carries its own status. */
const REPLY = Symbol('reply');

/** An answer with a status other than 200. */
export const reply = (status: number, body: unknown) => ({ [REPLY]: true, status, body });

/** Replaces fetch: `routes` maps "METHOD /path" to a JSON body, or to a `reply()`. */
export function fakeFetch(routes: Record<string, unknown>): Call[] {
  const calls: Call[] = [];
  mock.method(globalThis, 'fetch', async (url: string, init: RequestInit = {}) => {
    const method = init.method || 'GET';
    const path = String(url).slice(BASE.length);
    calls.push({ method, path, body: init.body ? JSON.parse(String(init.body)) : undefined });
    const route = routes[`${method} ${path}`] as { [REPLY]?: true; status: number; body: unknown } | undefined;
    if (route === undefined) return new Response('{"error":"no route"}', { status: 404 });
    const answer = route?.[REPLY] ? route : { status: 200, body: route };
    return new Response(JSON.stringify(answer.body), { status: answer.status });
  });
  return calls;
}

/** Captures console.log and console.error while `fn` runs. */
export async function captured(fn: () => unknown): Promise<{ out: string; err: string }> {
  const out: string[] = [];
  const err: string[] = [];
  const log = mock.method(console, 'log', (...a: unknown[]) => out.push(a.join(' ')));
  const error = mock.method(console, 'error', (...a: unknown[]) => err.push(a.join(' ')));
  try {
    await fn();
  } finally {
    log.mock.restore();
    error.mock.restore();
  }
  return { out: out.join('\n'), err: err.join('\n') };
}

/** Thrown by the fake process.exit. */
export class Exit extends Error {
  code: number | undefined;
  constructor(code: number | undefined) {
    super(`exit ${code}`);
    this.code = code;
  }
}

/** Makes process.exit throw an Exit, so a test can see it. */
export function trapExit(): void {
  mock.method(process, 'exit', (code?: number) => {
    throw new Exit(code);
  });
}

/** Flags that point commands at the fake control plane. */
export const FLAGS = { key: 'k-test', url: BASE };
