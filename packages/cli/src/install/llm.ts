/**
 * The agent LLM step: which vendor, its endpoint and model, and a key that is
 * proved before it is written.
 */
import { ask, askSecret, choose, step, note, warn, spinner, type Option } from '../prompt.ts';
import type { Spinner } from '../prompt/frame.ts';
import type { Answers } from './types.ts';
import { Status } from './constants.ts';

/** A vendor's endpoint, default model and menu label. */
interface Preset {
  /** OpenAI-compatible base URL. */
  base: string;
  /** Default model. */
  model: string;
  /** Menu label. */
  label: string;
}

/** The vendors with a known endpoint. */
const LLM_PRESETS: Record<string, Preset> = {
  anthropic: { base: 'https://api.anthropic.com/v1', model: 'claude-sonnet-5', label: 'Anthropic (Claude)' },
  openai: { base: 'https://api.openai.com/v1', model: 'gpt-4o-mini', label: 'OpenAI' },
  gemini: {
    base: 'https://generativelanguage.googleapis.com/v1beta/openai',
    model: 'gemini-3.8-flash',
    label: 'Gemini (Google)',
  },
  // Express mode: a global endpoint, no GCP project or location needed.
  vertex: {
    base: 'https://aiplatform.googleapis.com/v1/publishers/google',
    model: 'gemini-2.5-flash',
    label: 'Gemini Enterprise (Vertex AI)',
  },
};

/** The LLM menu. */
const LLM_OPTIONS: Option[] = [
  { id: 'anthropic', label: 'Anthropic (Claude)' },
  { id: 'openai', label: 'OpenAI' },
  { id: 'gemini', label: 'Gemini (Google)' },
  { id: 'vertex', label: 'Gemini Enterprise (Vertex AI)', note: 'express-mode API key' },
  { id: 'compatible', label: 'An OpenAI-compatible endpoint', note: 'OpenRouter, Together, Groq, Azure' },
  { id: 'local', label: 'A local model', note: 'Ollama, vLLM, LM Studio' },
  { id: 'skip', label: 'Skip', note: 'no agent control; add it later with `oya config`' },
];

/** The LLM answers, and the key to write to .env (empty for none). */
export interface LlmChoice {
  /** What goes in oya-install.json. */
  answers: Answers['llm'];
  /** The API key. */
  key: string;
}

/** No LLM at all. */
const SKIPPED: LlmChoice = { answers: { provider: 'skip', baseUrl: '', model: '' }, key: '' };

/** The endpoint: a preset's, or asked for. */
async function endpointFor(provider: string): Promise<string> {
  const preset = LLM_PRESETS[provider];
  if (preset) return preset.base;
  const fallback = provider === 'local' ? 'http://localhost:11434/v1' : 'https://openrouter.ai/api/v1';
  return (await ask('Base URL:', fallback)).replace(/\/+$/, '');
}

/** Asks which LLM agents should use, and for its key. */
export async function askLlm(): Promise<LlmChoice> {
  step('Agent LLM', 'Drives `oya ask` and the chat API.');
  note('CDP, the SDK and MCP all work without one.');
  const provider = await choose('Which LLM should agents use?', LLM_OPTIONS);
  if (provider === 'skip') return { answers: { provider, baseUrl: '', model: '' }, key: '' };
  const baseUrl = await endpointFor(provider);
  const model = await ask('Model:', LLM_PRESETS[provider]?.model || (provider === 'local' ? 'qwen2.5' : ''));
  return askKey({ provider, baseUrl, model });
}

/** Asks for the key until it verifies, is left blank, or the user moves on without an LLM. */
async function askKey(answers: Answers['llm']): Promise<LlmChoice> {
  // A local model on loopback usually needs no key at all.
  for (;;) {
    const key = await askSecret(answers.provider === 'local' ? 'API key (blank if none):' : 'API key:');
    if (!key) return { answers, key: '' };
    const problem = await verifyLlm(answers.baseUrl, key, answers.model);
    if (!problem) return { answers, key };
    warn(problem);
    if ((await retryOrSkip()) === 'skip') return SKIPPED;
  }
}

/** Losing every previous answer over one mistyped key would be absurd. */
function retryOrSkip(): Promise<string> {
  return choose('What now?', [
    { id: 'retry', label: 'Enter the key again' },
    { id: 'skip', label: 'Continue without an LLM', note: 'add it later with `oya config`' },
  ]);
}

/**
 * Anthropic authenticates with x-api-key and requires a version header; with
 * a valid key but no version it answers 400, which reads as "cannot verify"
 * when the key was in fact fine.
 */
function authHeaders(baseUrl: string, key: string): Record<string, string> {
  const anthropic = /(^|\.)anthropic\.com$/i.test(new URL(baseUrl).hostname);
  return anthropic ? { 'x-api-key': key, 'anthropic-version': '2023-06-01' } : { Authorization: `Bearer ${key}` };
}

/** The cheapest real generation on a Gemini Enterprise express endpoint, keyed in the query. */
function generateOnce(baseUrl: string, key: string, model: string): Promise<Response> {
  const name = encodeURIComponent(model || 'gemini-2.5-flash');
  const body = { contents: [{ role: 'user', parts: [{ text: 'hi' }] }], generationConfig: { maxOutputTokens: 1 } };
  const url = `${baseUrl.replace(/\/+$/, '')}/models/${name}:generateContent?key=${encodeURIComponent(key)}`;
  return fetch(url, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
}

/**
 * Gemini Enterprise express mode has no model listing (its /models 404s) and takes
 * its key only in the query string, a header or bearer token answers 401. So it is
 * verified by the cheapest real generation instead, which does separate a bad key
 * (401) from a good one (200). Matched on the endpoint shape, the same way
 * server/src/platform/llm.ts routes: Gemini's other endpoint is an OpenAI shim that wants a
 * bearer token.
 */
function probe(baseUrl: string, key: string, model: string): Promise<Response> {
  const headers = authHeaders(baseUrl, key);
  if (/\/publishers\/google\/?$/.test(baseUrl)) return generateOnce(baseUrl, key, model);
  return fetch(`${baseUrl}/models`, { headers });
}

/** The model listing's shape, where the endpoint has one. */
interface ModelList {
  /** The models on offer. */
  data?: Array<{
    /** A model's id. */
    id: string;
  }>;
}

/** Reads the probe's answer: a problem to report, or null when the install may carry on. */
async function judge(res: Response, spin: Spinner, baseUrl: string, model: string): Promise<string | null> {
  if (res.status === Status.UNAUTHORIZED || res.status === Status.FORBIDDEN) {
    spin.fail();
    return `${baseUrl} rejected that key (HTTP ${res.status}).`;
  }
  if (res.ok) await confirmModel(res, spin, model);
  // Not proof of a bad key, so it must not block the install, but it is not a tick either.
  else spin.fail(`could not verify (HTTP ${res.status}), continuing anyway`);
  return null;
}

/** The key works; says so, noting when the chosen model is missing from the endpoint's list. */
async function confirmModel(res: Response, spin: Spinner, model: string): Promise<void> {
  const body = (await res.json().catch(() => null)) as ModelList | null;
  const ids = (body?.data || []).map((m) => m.id);
  if (model && ids.length && !ids.includes(model)) {
    spin.done(`key works, though "${model}" is not in this endpoint's list`);
  } else {
    spin.done('ok');
  }
}

/**
 * Prove the credential before writing it, the way `oya login` does. A network
 * failure is not a bad key, so it warns rather than aborting the install.
 */
async function verifyLlm(baseUrl: string, key: string, model: string): Promise<string | null> {
  const spin = spinner('checking the key');
  try {
    return await judge(await probe(baseUrl, key, model), spin, baseUrl, model);
  } catch (err) {
    // A network failure is not a bad key, so it must not block the install.
    spin.fail(`could not reach it (${(err as Error).message}), continuing anyway`);
    return null;
  }
}
