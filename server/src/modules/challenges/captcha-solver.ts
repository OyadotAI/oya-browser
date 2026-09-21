/**
 * External CAPTCHA solving: the solver services this server can call and the
 * create-then-poll exchange they share. Used where the browser's provider has
 * no native solver (Oya Cloud, self-hosted, plain CDP).
 */

import { setTimeout as sleep } from 'timers/promises';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { CAPTCHA_POLL_MS, CAPTCHA_REQUEST_TIMEOUT_MS, DEFAULT_CAPTCHA_TIMEOUT_MS } from './constants.ts';

/** CapSolver's task type for each challenge; anything else is treated as reCAPTCHA v2. */
const CAPSOLVER_TASKS = {
  turnstile: 'AntiTurnstileTaskProxyLess',
  hcaptcha: 'HCaptchaTaskProxyLess',
  recaptcha_v3: 'ReCaptchaV3TaskProxyLess',
};

/** Each solver service: its endpoints and how to shape and read its messages. */
const SOLVERS = {
  capsolver: {
    create: 'https://api.capsolver.com/createTask',
    result: 'https://api.capsolver.com/getTaskResult',
    body: (key, task) => ({ clientKey: key, task }),
    taskFor: (type, sitekey, url) => ({
      type: Object.hasOwn(CAPSOLVER_TASKS, type) ? CAPSOLVER_TASKS[type] : 'ReCaptchaV2TaskProxyLess',
      websiteURL: url,
      websiteKey: sitekey,
    }),
    taskId: (r) => r.taskId,
    poll: (key, taskId) => ({ clientKey: key, taskId }),
    done: (r) =>
      r.status === 'ready'
        ? r.solution?.token || r.solution?.gRecaptchaResponse || null
        : r.status === 'failed'
          ? { error: r.errorDescription || 'solver failed' }
          : null,
  },
};

/** The solver settings, read from `env` on every call so a test can pass its own. */
const cfg = (env = process.env) => ({
  provider: env.OYA_CAPTCHA_PROVIDER || 'capsolver',
  key: env.OYA_CAPTCHA_API_KEY || '',
  timeoutMs: Number(env.OYA_CAPTCHA_TIMEOUT_MS) || DEFAULT_CAPTCHA_TIMEOUT_MS,
});

/** Whether an external solver API key is set. */
export const isConfigured = (env = process.env) => !!cfg(env).key;

/**
 * Solve via the configured external service.
 * @returns {Promise<{ token: string }>}
 */
export async function solveExternally(type, sitekey, url, env = process.env) {
  const job = solverJob(sitekey, env);
  const taskId = await createTask(job, type, sitekey, url);
  return awaitToken(job, taskId);
}

/** The configured solver and its key, or the reason a solve cannot start. */
function solverJob(sitekey, env) {
  const { provider, key, timeoutMs } = cfg(env);
  const solver = SOLVERS[provider];
  if (!solver) throw new HttpError(Status.BAD_REQUEST, `Unknown CAPTCHA provider: ${provider}`);
  if (!key) throw new HttpError(Status.CONFLICT, 'OYA_CAPTCHA_API_KEY is not set');
  if (!sitekey) throw new HttpError(Status.UNPROCESSABLE, 'No sitekey found for the challenge');
  return { provider, key, timeoutMs, solver };
}

/** Submits the challenge to the solver and returns its task id. */
async function createTask({ provider, key, solver }, type, sitekey, url) {
  const created = await postJson(provider, solver.create, solver.body(key, solver.taskFor(type, sitekey, url)));
  const taskId = solver.taskId(created);
  if (!taskId) throw new Error(`${provider} did not return a task id`);
  return taskId;
}

/** Polls the task until the solver answers, fails or the timeout passes. */
async function awaitToken({ provider, key, solver, timeoutMs }, taskId) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    await sleep(CAPTCHA_POLL_MS);
    const done = solver.done(await postJson(provider, solver.result, solver.poll(key, taskId)));
    if (typeof done === 'string') return { token: done };
    if (done?.error) throw new Error(done.error);
  }
  throw new HttpError(Status.GATEWAY_TIMEOUT, 'CAPTCHA solve timed out');
}

/** One JSON POST to the solver; a non-2xx answer is an error naming the provider. */
async function postJson(provider, endpoint, body) {
  const res = await fetch(endpoint, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
    signal: AbortSignal.timeout(CAPTCHA_REQUEST_TIMEOUT_MS),
  });
  if (!res.ok) throw new Error(`${provider} returned ${res.status}`);
  return res.json();
}
