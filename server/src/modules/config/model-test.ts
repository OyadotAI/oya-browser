/** Test an explicitly saved own provider key through the normal model policy and accounting boundary. */
import { createHash } from 'node:crypto';
import { providerFor, LlmError, type ChatRequest, type ChatResponse } from '../../platform/llm/index.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { admitModel, billUsage } from '../agent/chat.ts';
import { get, resolve } from './service.ts';
import { MODEL_TEST_MAX_TOKENS, MODEL_TEST_PROVIDERS, MODEL_TEST_TIMEOUT_MS } from './constants.ts';

/** Injectable seams let failure and cancellation tests run without real keys, storage or a network. */
export interface ModelTestDeps {
  /** Read settings without returning credentials to a client. */ snapshot(key: string): Snapshot;
  /** The same model admission as Ask and translation. */ admit: typeof admitModel;
  /** One provider attempt only; no retry amplification. */ send(request: ChatRequest): Promise<ChatResponse>;
  /** Completed usage is counted even if later configuration changes. */ bill: typeof billUsage;
}
/** Only a process-local snapshot holds the provider credential. */
interface Snapshot {
  /** Selected provider identifier. */ provider: string;
  /** Whether this key is owned by the caller. */ own: boolean;
  /** Key used for the fixed diagnostic request. */ apiKey: string;
  /** Already validated destination used by Ask. */ baseUrl: string;
  /** Catalog default or explicitly saved model. */ model: string;
}
/** Existing configuration services stay responsible for credential storage. */
function snapshot(key: string): Snapshot {
  const llm = resolve(key);
  return {
    provider: get(key).llm_provider as string,
    own: !!llm.own,
    apiKey: llm.openaiKey,
    baseUrl: llm.baseUrl,
    model: llm.model,
  };
}
/** Default composition reuses the production model adapters, policy and accounting. */
const DEFAULT: ModelTestDeps = {
  snapshot,
  admit: admitModel,
  send: (request) => providerFor(request.baseUrl).send(request),
  bill: billUsage,
};
/** A short fingerprint detects changes without retaining another copy of a credential in comparisons. */
const fingerprint = (value: Snapshot) => createHash('sha256').update(JSON.stringify(value)).digest('hex');
/** Invalid selections and host fallbacks never spend an operator's model key. */
function selected(value: Snapshot, provider: string): void {
  if (!MODEL_TEST_PROVIDERS.has(provider) || value.provider !== provider) throw changed();
  if (!value.own || !value.apiKey)
    throw new HttpError(Status.UNPROCESSABLE, 'Save your provider API key before testing.', {
      code: 'model_key_missing',
    });
}
/** Settings edited elsewhere require a new explicit save and test. */
const changed = () =>
  new HttpError(Status.CONFLICT, 'Model settings changed. Connect and test again.', { code: 'model_changed' });
/** Readiness belongs to the same saved provider, model, endpoint and credential. */
function unchanged(key: string, value: Snapshot, deps: ModelTestDeps): void {
  if (fingerprint(value) !== fingerprint(deps.snapshot(key))) throw changed();
}
/** Exactly one fixed, capped, cancellable request follows normal admission. */
export async function testModel(key: string, provider: string, signal?: AbortSignal, deps = DEFAULT) {
  const value = deps.snapshot(key);
  selected(value, provider);
  const { llm } = await deps.admit(key);
  unchanged(key, value, deps);
  return completedTest({ key, value, llm, signal, deps });
}
/** Successful usage is counted even if readiness is later refused after a concurrent settings edit. */
interface TestContext {
  /** Caller identity for accounting and configuration. */ key: string;
  /** Exact saved provider credential and model. */ value: Snapshot;
  /** Ordinary model admission result. */ llm: Awaited<ReturnType<typeof admitModel>>['llm'];
  /** Request disconnect and deadline cancellation. */ signal?: AbortSignal;
  /** Production services or hermetic test seams. */ deps: ModelTestDeps;
}
/** Count completed usage before validating that readiness still belongs to this configuration. */
async function completedTest(c: TestContext) {
  const { key, value, llm, signal, deps } = c;
  const reply = await sendTest(value, signal, deps);
  deps.bill({ apiKey: key, llm }, reply.usage);
  unchanged(key, value, deps);
  if (!hasAnswer(reply))
    throw new HttpError(Status.BAD_GATEWAY, 'The model did not finish its test. Try again.', { code: 'model_empty' });
  return { ok: true, provider: value.provider, model: value.model };
}
/** The test sees no web page, tools, user task or supplied instruction. */
async function sendTest(value: Snapshot, signal: AbortSignal | undefined, deps: ModelTestDeps) {
  const deadline = AbortSignal.timeout(MODEL_TEST_TIMEOUT_MS);
  const abort = signal ? AbortSignal.any([signal, deadline]) : deadline;
  abort.throwIfAborted();
  return deps.send(testRequest(value, abort)).catch((error) => {
    throw safeFailure(error, abort);
  });
}
/** A tiny fixed message cannot invoke browser tools or expose page data. */
const TEST_INPUT = {
  maxTokens: MODEL_TEST_MAX_TOKENS,
  effort: 'low' as const,
  messages: [{ role: 'user', content: 'Reply with OK only.' }],
};
/** Retain only the captured own credential and a genuinely abortable transport. */
function testRequest(value: Snapshot, signal: AbortSignal): ChatRequest {
  return { baseUrl: value.baseUrl, apiKey: value.apiKey, model: value.model, signal, ...TEST_INPUT };
}
/** Only an actual textual completion can establish readiness. */
const hasAnswer = (reply: ChatResponse) =>
  typeof reply.choices?.[0]?.message?.content === 'string' && !!reply.choices[0].message.content.trim();
/** Vendor bodies and endpoint details never cross the diagnostics response boundary. */
function safeFailure(error: unknown, signal: AbortSignal): HttpError {
  if (signal.aborted)
    return new HttpError(Status.GATEWAY_TIMEOUT, 'The model test timed out. Retry or choose another model.', {
      code: 'model_timeout',
    });
  const status = error instanceof LlmError ? error.status : null;
  return status !== null && Object.hasOwn(FAILURES, status) ? FAILURES[status]() : unavailable();
}
/** Refused keys and unsupported model permissions have a safe edit-and-retry path. */
const refused = () =>
  new HttpError(Status.UNPROCESSABLE, 'The key or model was refused. Check the key, model and permissions.', {
    code: 'model_key_refused',
  });
/** Provider balance and throttling are distinct from an invalid credential. */
const quota = () =>
  new HttpError(
    Status.TOO_MANY_REQUESTS,
    'The provider’s quota or balance is unavailable. Check your plan, then retry.',
    { code: 'model_quota' },
  );
/** Network, upstream and model errors never echo vendor bodies or credentials. */
const unavailable = () =>
  new HttpError(Status.BAD_GATEWAY, 'The provider did not finish the test. Check your connection or model and retry.', {
    code: 'model_unavailable',
  });
/** Explicit provider statuses map to stable recovery categories. */
const FAILURES: Record<number, () => HttpError> = {
  [Status.BAD_REQUEST]: refused,
  [Status.UNAUTHORIZED]: refused,
  [Status.FORBIDDEN]: refused,
  [Status.TOO_MANY_REQUESTS]: quota,
  [Status.PAYMENT_REQUIRED]: quota,
};
