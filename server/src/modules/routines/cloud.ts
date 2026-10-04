/**
 * Cloud routines: the ones the server runs itself, on an Oya Cloud browser it
 * starts for the run, so a schedule keeps going with no desktop open. Every
 * replica ticks; the routine's versioned claim lets exactly one of them take a
 * due run. The run starts a browser, waits for it to join, asks the agent
 * through the API's own chat route (so it is forwarded to the replica the
 * browser joined, with the usual run events), records the answer and stops
 * the browser.
 */
import { randomUUID } from 'node:crypto';
import { fingerprint } from '../../platform/audit.ts';
import { checkHourly } from '../../platform/limits.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { control, keyOfProject, terminal } from '../control/service.ts';
import { createSandbox, isConfigured as sandboxConfigured } from '../../drivers/sandbox.ts';
import { bookProvision } from '../browsers/lifecycle/provision.ts';
import { isUnclaimedAgentKey } from '../auth/service.ts';
import { NEEDS_INPUT } from '../agent/chat.ts';
import { container } from '../../app/container.ts';
import * as repository from './repository.ts';
import { claim, finishRun } from './service.ts';
import { isDue } from './schedule.ts';
import type { Routine } from './rules.ts';
import {
  ROUTINE_CLOUD_POLL_MS,
  ROUTINE_CLOUD_READY_MS,
  ROUTINE_CLOUD_TICK_MS,
  ROUTINE_RESULT_CHARS,
  ROUTINE_SELF_URL,
  ROUTINE_STEPS_KEPT,
} from './constants.ts';

/** One tool the agent called. */
interface ToolCall {
  /** The tool's name. */
  name: string;
}

/** The browser a run started, once it has one, so the run can stop it whatever happens. */
interface Started {
  /** The browser's id. */
  id?: string;
}

/** A run this replica claimed, wrapped so awaiting the claim does not await the run. */
interface Begun {
  /** Settles when the run has ended and been recorded. */
  done: Promise<void>;
}

/** What the chat route answers: the agent's reply, or the API's error shape. */
interface ChatAnswer {
  /** The agent's reply. */
  text?: string;
  /** Whether the agent said it could not do the task. */
  failed?: boolean;
  /** The tools it called, in order. */
  toolCalls?: ToolCall[];
  /** Why the run could not happen. */
  error?: string;
}

/** Waits `ms`. */
const pause = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** Refuses the run with a reason that ends up on the routine's history. */
const refuse = (message: string) => new HttpError(Status.FORBIDDEN, message);

/**
 * Starts every due cloud routine and answers its runs, which go on in the
 * background. One that fails to start is logged and waits until it is due again.
 */
export async function runDue(now = Date.now()): Promise<Promise<void>[]> {
  const runs = [];
  for (const { owner, routine } of await repository.cloud())
    if (isDue(routine, now)) runs.push((await begin(owner, routine).catch(startFailed(routine)))?.done);
  return runs.filter(Boolean);
}

/** Logs a start that failed, except losing the claim to another replica, which is the point of the claim. */
const startFailed =
  (routine: Routine) =>
  (err): undefined => {
    if (err.status !== Status.CONFLICT) console.error(`[routines] ${routine.id} did not start:`, err.message);
  };

/** Claims the run for this replica and starts it; answers the run, or nothing when it is not this server's to run. */
async function begin(owner: string, routine: Routine): Promise<Begun | undefined> {
  const key = routine.project ? await keyOfProject(routine.project) : null;
  if (!key || fingerprint(key) !== owner) return;
  const runId = randomUUID();
  await claim(key, routine.id, { lastRunAt: routine.lastRunAt, runId });
  return { done: perform(key, routine, runId) };
}

/** One run, start to end: whatever goes wrong is recorded as the run's failure, and the browser always stops. */
async function perform(key: string, routine: Routine, runId: string) {
  const browser: Started = {};
  try {
    await finishRun(key, routine.id, runId, endingOf(await runOnCloud(key, routine, browser)));
  } catch (err) {
    await finishRun(key, routine.id, runId, { status: 'failed', result: `Error: ${err.message}` }).catch(() => {});
  } finally {
    await stop(key, browser);
  }
}

/** Stops the run's browser, if it got one; the control worker tears down its sandbox. */
async function stop(key: string, browser: Started) {
  if (browser.id)
    await control()
      .cancel(key, browser.id)
      .catch(() => {});
}

/** Starts the browser (noting it in `browser` so it can be stopped), waits for it, and asks the agent. */
async function runOnCloud(key: string, routine: Routine, browser: Started) {
  browser.id = await steps.launch(key, routine.name);
  await steps.connected(key, browser.id);
  return steps.ask(key, browser.id, routine.prompt);
}

/** Starts one cloud browser, held to the same gates as starting one through the API. */
async function launch(key: string, name: string): Promise<string> {
  if (!sandboxConfigured(key)) throw refuse('Oya Cloud browsers are not set up on this server.');
  if (await isUnclaimedAgentKey(key)) throw refuse('Cloud routines need a claimed key.');
  await container.billing.entitlements.admitCloud(key, 1);
  if (!checkHourly('sandboxesPerHour', key).allowed) throw refuse('The hourly cloud browser quota is used up.');
  const made = await createSandbox({ apiKey: key, name: `Routine: ${name}` });
  bookProvision(key, 1, [made], []);
  return made.browserId;
}

/** Waits until the browser's session is ready, failing when it ends first or takes too long. */
async function connected(key: string, browserId: string) {
  const deadline = Date.now() + ROUTINE_CLOUD_READY_MS;
  while (Date.now() < deadline) {
    const { state } = await control().session(key, browserId);
    if (state === 'ready') return;
    if (terminal.has(state)) throw new Error(`The cloud browser ${state} before it connected.`);
    await pause(ROUTINE_CLOUD_POLL_MS);
  }
  throw new Error('The cloud browser did not connect in time.');
}

/** Asks the agent on the browser through the chat route; its error becomes a thrown one. */
async function ask(key: string, browserId: string, prompt: string): Promise<ChatAnswer> {
  const res = await fetch(`${ROUTINE_SELF_URL}/api/browsers/${encodeURIComponent(browserId)}/chat`, {
    method: 'POST',
    headers: { authorization: `Bearer ${key}`, 'content-type': 'application/json' },
    body: JSON.stringify({ messages: [{ role: 'user', content: prompt }] }),
  });
  const answer: ChatAnswer = await res.json();
  if (answer.error) throw new Error(answer.error);
  return answer;
}

/** The run's three steps, as one object so tests can stand in for the cloud. */
export const steps = { launch, connected, ask };

/**
 * The run's record from the agent's answer: done or failed, its reply and the
 * tools it used. A run that stopped to ask a person failed: nobody is at a
 * cloud run to answer, and its browser stops with it.
 */
function endingOf(answer: ChatAnswer) {
  const text = answer.text || '';
  const used = (answer.toolCalls || []).map((c) => c.name).slice(0, ROUTINE_STEPS_KEPT);
  const failed = answer.failed || text.startsWith(NEEDS_INPUT);
  return { status: failed ? 'failed' : 'done', result: text.slice(0, ROUTINE_RESULT_CHARS), steps: used };
}

/** The scheduler's timer, while it runs. */
let timer: NodeJS.Timeout | null = null;

/** Starts looking for due cloud routines every ROUTINE_CLOUD_TICK_MS. */
export function startCloudRoutines() {
  const tick = () => runDue().catch((err) => console.error('[routines] cloud tick failed:', err.message));
  timer ??= setInterval(tick, ROUTINE_CLOUD_TICK_MS);
  timer.unref?.();
}

/** Stops looking; runs already going finish on their own. */
export function stopCloudRoutines() {
  if (timer) clearInterval(timer);
  timer = null;
}
