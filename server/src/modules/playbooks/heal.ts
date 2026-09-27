/**
 * Healing a broken replay: the agent finishes the task from the failed step, and
 * its steps replace the broken ones in the playbook, so the next replay runs clean.
 */
import { runChat, lastRun } from '../agent/chat.ts';
import * as keyConfig from '../config/service.ts';
import { challengesFor } from './checkpoint.ts';
import { templateValues } from './variables.ts';

/** The entries of `values` whose key passes `keep`. */
const pick = (values, keep) => Object.fromEntries(Object.entries(values).filter(([k]) => keep(k)));

/**
 * The agent finishes the task from the failed step and its steps replace the broken ones in the playbook.
 * If it cannot and a person is reachable, they finish it in the live view instead.
 */
export async function heal(apiKey, browserId, pb, i, err, values, hooks) {
  const total = pb.steps.length;
  const task = healTask(pb, i, total, err);
  const outcome = await attempt(apiKey, browserId, pb, task, values, hooks);
  if (outcome.failed) return handOver(outcome.healErr, hooks, i, total);
  return saveHealed(apiKey, browserId, pb, i, outcome.result);
}

/** The agent's result, or the error it failed with. */
async function attempt(apiKey, browserId, pb, task, values, hooks): Promise<any> {
  try {
    return { result: await finishWithAgent(apiKey, browserId, pb, task, values, hooks) };
  } catch (healErr) {
    return { failed: true, healErr };
  }
}

/** The agent's task: the playbook's prompt, and where the replay broke. */
function healTask(pb, i, total, err) {
  return `${pb.prompt}\n\nA recorded playbook already did ${i} of ${total} steps of this task, then failed (${err.message}). Look at the page and finish the task from where it is.`;
}

/** The playbook's values split into what the agent may read and its secrets. */
function taskData(pb, values) {
  const hidden = new Set(pb.secrets || []);
  // Button and link labels are how the flow was clicked, not what it was about. Handing
  // them over as data makes redact() rewrite "Sign in" to {{signIn}} in the page the
  // agent is reading, which is the opposite of help.
  const ignored = new Set([...hidden, ...(pb.labels || [])]);
  return { data: pick(values, (k) => !ignored.has(k)), secrets: pick(values, (k) => hidden.has(k)) };
}

/** Runs the agent on the task; throws when it did not finish. */
async function finishWithAgent(apiKey, browserId, pb, task, values, hooks) {
  const challenges = challengesFor(apiKey, browserId);
  const options = { apiKey, ...taskData(pb, values), challenges, ...hooks };
  const result = await runChat(browserId, [{ role: 'user', content: task }], options);
  if (result.limited) throw new Error('the agent hit its step limit');
  if (result.failed) throw new Error(result.text.trim());
  return result;
}

/** The agent could not finish: a person does, or the error stands. */
async function handOver(healErr, hooks, i, total) {
  if (!hooks.requestHuman) throw healErr;
  await hooks.requestHuman({
    reason: 'heal_failed',
    message: `Replay broke at step ${i + 1} and the agent could not finish (${healErr.message}). Finish it in the live view, then respond.`,
  });
  return { steps: i, total, fellBack: true, healed: false, text: 'Finished by a person.' };
}

/** The steps the healing agent recorded, without the page it started on. */
const healedSteps = (browserId) => (lastRun(browserId)?.steps || []).filter((s) => !s.start);

/** The recorded steps before `i` plus the agent's, with variables for what it typed. */
function healedPlaybook(pb, i, healed) {
  const fixed = { ...pb, steps: [...pb.steps.slice(0, i), ...healed], defaults: { ...pb.defaults } };
  return { ...templateValues(fixed), healedFrom: i, healedAt: new Date().toISOString() };
}

/**
 * Makes the recorded steps before `i` plus the agent's the playbook. Kept as a draft,
 * the playbook broke and healed at the same step on every replay until someone
 * promoted it. The agent's steps get the same variables a recording does, so
 * whatever it typed literally is not baked in. Any older draft is superseded.
 */
async function saveHealed(apiKey, browserId, pb, i, result) {
  const healed = healedSteps(browserId);
  // An agent that found the task already done (a login it was still signed in to)
  // recorded nothing to replay; saving that cut the playbook off at the broken step.
  if (healed.some((s) => s.action !== 'navigate')) {
    await keyConfig.savePlaybook(apiKey, pb.name, healedPlaybook(pb, i, healed));
    await keyConfig.deletePlaybook(apiKey, `${pb.name}:draft`);
  }
  return { steps: i, total: pb.steps.length, fellBack: true, healed: true, text: result.text };
}
