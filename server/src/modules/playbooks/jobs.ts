/**
 * The work a background run does: replay a playbook, or hand a prompt to the
 * agent, with the checkpoint between page-changing steps.
 */
import { runChat } from '../agent/chat.ts';
import { play } from './service.ts';
import { checkpointFor, challengesFor } from './checkpoint.ts';

/** What a run was submitted with. */
export interface Job {
  /** The caller's key. */
  key: string;
  /** The browser the run drives. */
  browserId: string;
  /** The playbook to replay, or null to run the prompt. */
  pb: any;
  /** The task for the agent, when there is no playbook. */
  prompt: string;
  /** Values for the task's placeholders. */
  data: Record<string, any>;
  /** Values the model never sees. */
  secrets: Record<string, any>;
  /** Whether a broken replay is healed by the agent. */
  autoHeal: boolean;
}

/** Runs the job, parking on a person through `requestHuman` when it needs one. */
export async function runJob(job: Job, requestHuman) {
  const checkpoint = checkpointFor(job.key, job.browserId, requestHuman);
  if (job.pb) return replayJob(job, checkpoint, requestHuman);
  return runPrompt(job, checkpoint, requestHuman);
}

/** Replays the job's playbook with its data and secrets as the variables. */
function replayJob({ key, browserId, pb, data, secrets, autoHeal }: Job, checkpoint, requestHuman) {
  const values = { ...data, ...secrets };
  return play(key, browserId, pb, values, { autoHeal: autoHeal !== false, checkpoint, requestHuman });
}

/** Hands the prompt to the agent; a run that hit its limit or said FAILED fails. */
async function runPrompt({ key, browserId, prompt, data, secrets }: Job, checkpoint, requestHuman) {
  const options = { apiKey: key, data, secrets, checkpoint, requestHuman, challenges: challengesFor(key, browserId) };
  const result = await runChat(browserId, [{ role: 'user', content: prompt }], options);
  if (result.limited) throw new Error('The agent hit its step limit without finishing');
  // Anywhere in the reply, not just the first line: a model that narrates before
  // its verdict still failed, and a "succeeded" run saves the broken steps as a playbook.
  if (result.failed) throw new Error(result.text.trim());
  return { text: result.text };
}
