/**
 * Healing a broken replay: the agent finishes the task from the failed step, and
 * its steps are saved as the draft `<name>:draft` for review.
 */
import { runChat, lastRun } from '../agent/chat.ts';
import * as keyConfig from '../config/service.ts';
import { challengesFor } from './checkpoint.ts';

/** The entries of `values` whose key passes `keep`. */
const pick = (values, keep) => Object.fromEntries(Object.entries(values).filter(([k]) => keep(k)));

/**
 * The agent finishes the task from the failed step and its steps are saved as the draft `<name>:draft`.
 * If it cannot and a person is reachable, they finish it in the live view instead.
 */
export async function heal(apiKey, browserId, pb, i, err, values, hooks) {
  const total = pb.steps.length;
  const task = healTask(pb, i, total, err);
  const outcome = await attempt(apiKey, browserId, pb, task, values, hooks);
  if (outcome.failed) return handOver(outcome.healErr, hooks, i, total);
  return saveDraft(apiKey, browserId, pb, i, outcome.result);
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

/** Saves the recorded steps before `i` plus the agent's as the draft. */
async function saveDraft(apiKey, browserId, pb, i, result) {
  const healed = (lastRun(browserId)?.steps || []).filter((s) => !s.start);
  const steps = [...pb.steps.slice(0, i), ...healed];
  const draft = { ...pb, steps, healedFrom: i, healedAt: new Date().toISOString() };
  await keyConfig.savePlaybook(apiKey, `${pb.name}:draft`, draft);
  const total = pb.steps.length;
  return { steps: i, total, fellBack: true, healed: true, draft: `${pb.name}:draft`, text: result.text };
}
