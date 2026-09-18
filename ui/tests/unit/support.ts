/**
 * Fakes the console's unit tests share: a scripted fetch.
 */
import { vi } from 'vitest';

/** One scripted answer: a status and a body (objects are sent as JSON). */
export interface Answer {
  /** HTTP status; 200 when omitted. */
  status?: number;
  /** The body; strings go as is. */
  body?: unknown;
}

/** Replaces fetch with one that answers each call from `answers` in turn (the last repeats). */
export function fakeFetch(...answers: Answer[]) {
  let i = 0;
  const fn = vi.fn(async () => {
    const { status = 200, body = '' } = answers[Math.min(i++, answers.length - 1)] ?? {};
    return new Response(typeof body === 'string' ? body : JSON.stringify(body), { status });
  });
  vi.stubGlobal('fetch', fn);
  return fn;
}

/** The URL and init of fetch call `n`. */
export function fetchCall(fn: ReturnType<typeof fakeFetch>, n = 0): [string, RequestInit] {
  return fn.mock.calls[n] as unknown as [string, RequestInit];
}
