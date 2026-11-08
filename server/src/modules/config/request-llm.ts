/**
 * A model chosen for one request, not saved: the SDK's `new Oya({ model })`
 * sends it as X-Oya-Llm-* headers on every call. Without this, picking a model
 * meant `oya init` or a config write that also changed the console's and the
 * desktop's model. The choice rides along with the request's work, background
 * runs and heals included, the way a lane does, so no route has to pass it on.
 */
import { AsyncLocalStorage } from 'node:async_hooks';
import { HttpError, sendError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { priced } from '../billing/cost.ts';
import { CONFIG_VALUE_MAX_CHARS } from './constants.ts';
import { LLM_DEFAULTS } from './llm-catalog.ts';
import type { LlmConfig } from './service.ts';

/** The model a request asked for. */
export interface RequestLlm {
  /** An LLM_DEFAULTS id: picks the endpoint, never a caller-supplied URL. */
  provider: string;
  /** The model id; the provider's default when left out. */
  model?: string;
  /** The caller's own provider key; without one the model runs on the deployment's credential. */
  apiKey?: string;
}

/** The model of the request in progress. */
const asked = new AsyncLocalStorage<RequestLlm>();

/**
 * A ChatGPT sign-in is a refresh token, not an API key: one pasted into a header
 * must never be sent on as a bearer, so that provider is saved-config only.
 */
const SAVED_ONLY = new Set(['chatgpt']);

/** The model the request in progress asked for, if it asked. */
export const requestLlm = (): RequestLlm | undefined => asked.getStore();

/** Runs `work` with `llm` as the request's model. */
export const withLlm = <T>(llm: RequestLlm, work: () => T): T => asked.run(llm, work);

/** One header's value, trimmed; a value past the setting limit is refused as a paste gone wrong. */
function header(headers: Record<string, unknown>, name: string): string | undefined {
  const value = typeof headers[name] === 'string' ? (headers[name] as string).trim() : '';
  if (value.length > CONFIG_VALUE_MAX_CHARS) throw new HttpError(Status.BAD_REQUEST, `${name} is too long.`);
  return value || undefined;
}

/** The provider named, refused unless it is one this deployment can call per request. */
function providerOf(provider: string): string {
  if (Object.hasOwn(LLM_DEFAULTS, provider) && !SAVED_ONLY.has(provider)) return provider;
  const offered = Object.keys(LLM_DEFAULTS).filter((id) => !SAVED_ONLY.has(id));
  throw new HttpError(Status.BAD_REQUEST, `X-Oya-Llm-Provider must be one of ${offered.join(', ')}.`);
}

/**
 * A model picked without a key runs on the credential the key already uses, so it
 * must be that credential's vendor. On the deployment's credential it must also be
 * a model with a price: the hourly budget and billing are counted in that price,
 * and an unknown model would be charged as a guess. With no credential at all (an
 * agent's unclaimed key) there is nothing to check; the chat says how to bring one.
 */
function savedRefusal({ provider, model }: RequestLlm, saved: LlmConfig): string | null {
  if (!saved.openaiKey) return null;
  if (!sameHost(LLM_DEFAULTS[provider].base, saved.baseUrl))
    return `This key's model is not ${provider}; pass your ${provider} key in the model option.`;
  if (!saved.own && model && !priced(model))
    return `${model} is not offered on this server's model; pass your own key.`;
  return null;
}

/** The model the headers ask for, or null when they ask none. Throws 400 on a bad one. */
export const llmFromHeaders = (headers: Record<string, unknown> = {}): RequestLlm | null => asParsed(headers);

/**
 * Refuses the request's model when it cannot run on the key's saved credential.
 * Checked where a model is about to be called, after auth has settled which key
 * the request is (a scoped credential's project, not the raw token), and with
 * `saved` read lazily so a call that runs no model pays nothing for it.
 */
export function checkRequestLlm(saved: () => LlmConfig): void {
  const llm = requestLlm();
  const why = llm && !llm.apiKey ? savedRefusal(llm, saved()) : null;
  if (why) throw new HttpError(Status.BAD_REQUEST, why);
}

/** The three headers read and checked on their own, before any key is looked at. */
function asParsed(headers: Record<string, unknown>): RequestLlm | null {
  const provider = header(headers, 'x-oya-llm-provider');
  const model = header(headers, 'x-oya-llm-model');
  const apiKey = header(headers, 'x-oya-llm-api-key');
  if (!provider && !model && !apiKey) return null;
  if (!provider) throw new HttpError(Status.BAD_REQUEST, 'X-Oya-Llm-Provider is required with a model or key.');
  return { provider: providerOf(provider), model, apiKey };
}

/** Whether two endpoints are the same host: a model swap must not move the deployment's key to another vendor. */
const sameHost = (a: string, b: string) => URL.parse(a)?.host === URL.parse(b)?.host;

/**
 * The config a run uses when its request asked for `llm`, given the key's saved one.
 * With the caller's own key: that key, the catalog's endpoint, nothing saved. Without:
 * only the model changes, on whichever credential the key uses, and only when that
 * credential is the same vendor (the middleware refuses the rest with a 400). An
 * agent's unclaimed key has no credential to swap a model on, so it stays without one.
 */
export function onRequestLlm(saved: LlmConfig, llm: RequestLlm): LlmConfig {
  const { base, model } = LLM_DEFAULTS[llm.provider];
  if (llm.apiKey) return { own: true, openaiKey: llm.apiKey, baseUrl: base, model: llm.model || model };
  if (!saved.openaiKey || !sameHost(base, saved.baseUrl)) return saved;
  return { ...saved, model: llm.model || saved.model };
}

/** Middleware: runs the rest of the request with the model its headers ask for. */
export function requestLlmMiddleware(req, res, next) {
  let llm: RequestLlm | null;
  try {
    llm = llmFromHeaders(req.headers);
  } catch (err) {
    return sendError(res, err);
  }
  return llm ? withLlm(llm, next) : next();
}
