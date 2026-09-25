/**
 * OpenRouter's model list, read live so its picker offers what OpenRouter
 * serves today without a release. Served from memory and refreshed in the
 * background once it is an hour old, so GET /config never waits on
 * OpenRouter; until the first fetch lands, or when OpenRouter cannot be
 * reached, the caller's built-in list is used instead.
 */
import {
  OPENROUTER_MODELS_MAX,
  OPENROUTER_MODELS_TIMEOUT_MS,
  OPENROUTER_MODELS_TTL_MS,
  OPENROUTER_MODELS_URL,
} from './constants.ts';
import type { ModelChoice } from './llm-catalog.ts';

/** One model as OpenRouter lists it, as much as this file reads. */
interface OpenRouterModel {
  /** vendor/model, the id chat completions take. */
  id: string;
  /** "Vendor: Model". */
  name?: string;
  /** Seconds since the epoch it was added. */
  created?: number;
  /** Request parameters it accepts; the agent needs "tools". */
  supported_parameters?: string[];
}

/** The last list fetched, and the state of fetching it. */
interface Cache {
  /** The list, or null before a fetch has succeeded. */
  models: ModelChoice[] | null;
  /** When the last fetch ended, well or badly; -Infinity before the first. */
  fetchedAt: number;
  /** Whether a fetch is running. */
  inFlight: boolean;
}

/** The last list fetched and when, or nothing yet. */
const cache: Cache = {
  models: null,
  fetchedAt: -Infinity,
  inFlight: false,
};

/** Whether a model can drive the agent: it takes tool calls, and it is not a batch-only variant. */
const usable = (m: OpenRouterModel) => !!m.supported_parameters?.includes('tools') && !m.id.endsWith(':batch');

/** A model as a picker choice. */
const choiceOf = (m: OpenRouterModel): ModelChoice => ({ id: m.id, label: m.name || m.id });

/**
 * OpenRouter's answer as picker choices: the hand-picked `pinned` models it
 * still serves first, in their order (so the default and the well-known
 * models lead), then every other usable model, newest first.
 */
export function toChoices(data: OpenRouterModel[], pinned: ModelChoice[] = []): ModelChoice[] {
  const usableModels = data.filter(usable);
  const byId = new Map(usableModels.map((m) => [m.id, m]));
  const first = pinned.filter((p) => byId.has(p.id)).map((p) => choiceOf(byId.get(p.id)!));
  const rest = usableModels.filter((m) => !pinned.some((p) => p.id === m.id));
  rest.sort((a, b) => (b.created || 0) - (a.created || 0));
  return [...first, ...rest.map(choiceOf)].slice(0, OPENROUTER_MODELS_MAX);
}

/** OpenRouter's current list as choices; empty when it answered with an error. */
async function fetchChoices(fetchImpl: typeof fetch, url: string, pinned: ModelChoice[]): Promise<ModelChoice[]> {
  const res = await fetchImpl(url, { signal: AbortSignal.timeout(OPENROUTER_MODELS_TIMEOUT_MS) });
  return res.ok ? toChoices((await res.json())?.data || [], pinned) : [];
}

/** Fetches the list once; a failure keeps what was there and is retried on the next read after the TTL. */
async function refresh(fetchImpl: typeof fetch, url: string, pinned: ModelChoice[]): Promise<void> {
  cache.inFlight = true;
  // Offline, blocked or slow: the built-in list stands in until the next try.
  const choices = await fetchChoices(fetchImpl, url, pinned).catch((): ModelChoice[] => []);
  if (choices.length) cache.models = choices;
  Object.assign(cache, { fetchedAt: Date.now(), inFlight: false });
}

/** Whether the cached list is due a refresh, and one can be started. */
const due = (url: string) => !!url && !cache.inFlight && Date.now() - cache.fetchedAt >= OPENROUTER_MODELS_TTL_MS;

/**
 * The models to offer now: the cached live list, else `fallback`, which also
 * leads the live list (see toChoices). Starts a
 * background refresh when due; `url` '' (OYA_OPENROUTER_MODELS_URL='') never fetches.
 */
export function openRouterModels(
  fallback: ModelChoice[],
  fetchImpl: typeof fetch = globalThis.fetch,
  url: string = OPENROUTER_MODELS_URL,
): ModelChoice[] {
  if (due(url)) void refresh(fetchImpl, url, fallback);
  return cache.models || fallback;
}

/** Waits for a refresh in progress to land; for tests and for a warm start. */
export const settled = async (): Promise<void> => {
  while (cache.inFlight) await new Promise((r) => setImmediate(r));
};

/** Forgets the cached list (tests). */
export function resetOpenRouterModels(): void {
  Object.assign(cache, { models: null, fetchedAt: -Infinity, inFlight: false });
}
