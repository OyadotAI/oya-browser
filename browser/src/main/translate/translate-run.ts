/**
 * One translation of one page: collect its text in the translation world, send
 * it to the server with bounded parallelism, and write each answer back. Main
 * rechecks consent and page-side document/run fences reject queued stale writes.
 */
import type { WebContents } from 'electron';
import { runInWorld } from '../native/stock-world.ts';
import { readApiAnswer, type ServerApi } from '../connection/server-api.ts';
import { applyFor, collectFor, restoreFor } from '../../page/translation-guard.ts';
import {
  TOO_MANY_REQUESTS,
  TRANSLATE_BATCH_CHARS,
  TRANSLATE_BATCH_SEGMENTS,
  TRANSLATE_BATCH_TIMEOUT_MS,
  TRANSLATE_FIRST_BATCH_CHARS,
  TRANSLATE_PARALLEL,
  TRANSLATE_PAGE_CHARS,
  TRANSLATE_WORLD_ID,
  TRANSLATE_RESPONSE_CHARS,
} from './constants.ts';

/** A run's failure, with the code the chip looks its words up by. */
export class TranslateError extends Error {
  /** signed_out, offline, timeout, nothing_to_translate, rate_limited, or the server's code. */
  readonly code: string;

  /** `code` names the failure; `message` is the server's own words when it gave some. */
  constructor(code: string, message = code) {
    super(message);
    this.code = code;
  }
}

/** What a run sends: where to, into which language, and from which, when the page said. */
export interface RunRequest {
  /** The server, called as this browser. */
  api: Pick<ServerApi, 'postToBrowser'>;
  /** The tab's page. */
  contents: WebContents;
  /** The person's language tag. */
  target: string;
  /** Aborts the run. */
  signal: AbortSignal;
  /** Exact isolated document identity. */
  document: string;
  /** Run sequence, tombstoned by restoration. */
  serial: number;
  /** Rechecks the main-owned grant before sending or applying text. */
  check(): void;
}

/** An error as the server, fetch or a timeout leaves it. */
interface RawError {
  /** TimeoutError, TypeError (fetch could not reach the server), AbortError. */
  name?: string;
  /** The server's code, when it gave one. */
  code?: string;
  /** The HTTP status, when the server answered. */
  status?: number;
  /** Its words. */
  message?: string;
}

/** What the collect script answers. */
interface Collected {
  /** The page's texts, trimmed, in page order. */
  texts: string[];
  /** Whether some text was left out (the page limit, or a node longer than a batch). */
  partial: boolean;
}

/** What the server answers for one batch. */
interface Translated {
  /** The translations, in the order sent. */
  texts: string[];
}

/** Malformed or oversized model replies never reach a page script. */
function translations(value: unknown, count: number): string[] {
  const texts = (value as Translated)?.texts;
  if (!Array.isArray(texts) || texts.length !== count || texts.some((text) => typeof text !== 'string' || !text.trim()))
    throw new TranslateError('translate_malformed');
  return boundedTexts(texts);
}

/** Bound aggregate expansion before serializing a model reply into a page script. */
function boundedTexts(texts: string[]): string[] {
  if (texts.reduce((total, text) => total + text.length, 0) > TRANSLATE_RESPONSE_CHARS)
    throw new TranslateError('translate_malformed');
  return texts;
}

/** Whether `text` still fits in `batch`, the first batch being the small one. ponytail: re-sums the batch per text; at most 200 short strings. */
const fits = (batch: string[], text: string, first: boolean): boolean =>
  batch.length < TRANSLATE_BATCH_SEGMENTS &&
  batch.reduce((n, t) => n + t.length, 0) + text.length <=
    (first ? TRANSLATE_FIRST_BATCH_CHARS : TRANSLATE_BATCH_CHARS);

/** The collected texts in request-sized batches, each with the index of its first text; the first is small, so the top of the page comes back first. */
export function batches(texts: string[]): Array<[number, string[]]> {
  const out: Array<[number, string[]]> = [];
  texts.forEach((text, i) => {
    const last = out.at(-1);
    if (last && fits(last[1], text, out.length === 1)) last[1].push(text);
    else out.push([i, [text]]);
  });
  return out;
}

/** The failure an error becomes: the server's code, a rate limit, a timeout, no server at all (fetch's TypeError), or anything else. */
export function failureOf(err: unknown): TranslateError {
  const e = (err ?? {}) as RawError;
  if (e.name === 'TimeoutError') return new TranslateError('timeout');
  if (e.code) return new TranslateError(e.code, e.message);
  if (e.status === TOO_MANY_REQUESTS) return new TranslateError('rate_limited', e.message);
  if (e.name === 'TypeError' && !e.status) return new TranslateError('offline');
  return new TranslateError('failed', e.message);
}

/** Sends one batch and answers its translations, failing as a TranslateError unless the run was stopped. */
async function translateBatch(req: RunRequest, texts: string[]): Promise<string[]> {
  // The page's own tag is not sent: the model reads the language from the text, and a tag like "DE" is not one the server takes.
  const body = { texts, target: req.target };
  try {
    req.check();
    const res = await req.api.postToBrowser('translate', body, TRANSLATE_BATCH_TIMEOUT_MS, req.signal);
    return translations(await readApiAnswer(res), texts.length);
  } catch (err) {
    throw req.signal.aborted ? err : failureOf(err);
  }
}

/** Collects the page's text in the translation world. */
const collect = (req: RunRequest): Promise<Collected> =>
  runInWorld<Collected>(
    req.contents,
    TRANSLATE_WORLD_ID,
    collectFor(req.document, req.serial, TRANSLATE_PAGE_CHARS, TRANSLATE_BATCH_CHARS),
  );

/** Takes batches off `queue` one at a time, sending each and writing its answer, until the queue is empty or the run stops. */
async function worker(req: RunRequest, queue: Array<[number, string[]]>): Promise<void> {
  for (let next = queue.shift(); next && !req.signal.aborted; next = queue.shift()) {
    const out = await translateBatch(req, next[1]);
    if (req.signal.aborted) return;
    req.check();
    await runInWorld(req.contents, TRANSLATE_WORLD_ID, applyFor(req.document, req.serial, next[0], out));
  }
}

/**
 * Translates the page, TRANSLATE_PARALLEL requests at a time, writing each answer as it
 * lands; answers whether only part of it was. The first failure stops the other requests,
 * so nothing is written after the caller restores the page. Throws a TranslateError; a
 * stopped run ends quietly.
 */
export async function translatePage(req: RunRequest): Promise<Pick<Collected, 'partial'>> {
  req.check();
  const page = await collect(req);
  req.check();
  if (!page.texts.length) throw new TranslateError('nothing_to_translate');
  await translateCollected(req, page.texts);
  return { partial: page.partial };
}

/** Stop sibling workers on the first failure, then settle every issued operation before reporting it. */
async function translateCollected(req: RunRequest, texts: string[]): Promise<void> {
  const stop = new AbortController();
  const run = { ...req, signal: AbortSignal.any([req.signal, stop.signal]) };
  const queue = batches(texts);
  const workers = workersFor(run, queue, stop);
  const results = await Promise.allSettled(workers);
  const failed = results.find((result) => result.status === 'rejected');
  if (failed?.status === 'rejected') throw failed.reason;
}

/** A failed worker stops its siblings without letting an issued operation escape the settlement barrier. */
const workersFor = (run: RunRequest, queue: Array<[number, string[]]>, stop: AbortController) =>
  Array.from({ length: TRANSLATE_PARALLEL }, () =>
    worker(run, queue).catch((error) => {
      stop.abort();
      throw error;
    }),
  );

/** Puts the page's original text back where it still holds the translation; a page already gone has nothing to restore. */
export function restorePage(contents: WebContents, document: string, serial: number): Promise<unknown> {
  if (contents.isDestroyed()) return Promise.resolve();
  return runInWorld(contents, TRANSLATE_WORLD_ID, restoreFor(document, serial));
}
