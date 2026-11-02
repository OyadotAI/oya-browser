/**
 * A routine that plays a saved playbook: started as a background run on the server
 * for this browser, followed until it ends, and answered in the shape a prompt's run
 * answers, so the routine's history reads the same either way.
 */
import { readApiAnswer, type ServerApi } from '../connection/server-api.ts';
import { PLAYBOOK_POLL_MS } from '../connection/constants.ts';
import type { ChatAnswer } from './routines.ts';

/** A background playbook run, as the runs route reports it. */
interface PlaybookRun {
  /** Its id. */
  id: string;
  /** running, needs_attention, succeeded or failed. */
  status: string;
  /** What the playbook answered, once it succeeded. */
  result?: unknown;
  /** Why it failed. */
  error?: string;
}

/** Waits `ms`, without holding the app open. */
const pause = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms).unref?.());

/** Plays `playbook` on this browser with the routine's `values` and answers how it ended; `stopped` ends the wait early. */
export async function playOnServer(
  api: ServerApi,
  playbook: string,
  values: Record<string, string>,
  stopped: () => boolean,
): Promise<ChatAnswer> {
  const started = await api.postToBrowser('runs', { playbook, data: values });
  return followRun(api, (await readApiAnswer(started)) as PlaybookRun, stopped);
}

/** Polls the run until it ends or `stopped`, and answers how it ended. */
async function followRun(api: ServerApi, first: PlaybookRun, stopped: () => boolean): Promise<ChatAnswer> {
  let run = first;
  while (run.status === 'running' && !stopped()) {
    await pause(PLAYBOOK_POLL_MS);
    run = ((await api.get(`runs/${encodeURIComponent(run.id)}`).catch(() => null)) as PlaybookRun | null) ?? run;
  }
  return playedAnswer(run, stopped());
}

/** What a playbook's run did, in words: how many steps played, and who finished a broken one. */
function playedText(result: unknown): string {
  if (typeof result === 'string') return result;
  const r = (result ?? {}) as PlayedResult;
  if (r.total === undefined) return result === undefined ? 'Done' : JSON.stringify(result);
  const played = `Played ${r.steps} of ${r.total} steps`;
  if (!r.fellBack) return `${played}.`;
  return `${played}; ${r.healed ? 'the agent' : 'a person'} finished the rest${r.text ? `: ${r.text}` : '.'}`;
}

/** What a playbook's replay answers. */
interface PlayedResult {
  /** Steps played as recorded. */
  steps?: number;
  /** Steps it has. */
  total?: number;
  /** Whether a broken step was handed on. */
  fellBack?: boolean;
  /** Whether the agent (rather than a person) finished it. */
  healed?: boolean;
  /** What whoever finished it said. */
  text?: string;
}

/** A playbook run's end as a routine records it; one waiting for a person is left to them and said so. */
function playedAnswer(run: PlaybookRun, stopped: boolean): ChatAnswer {
  if (stopped) return { error: 'Stopped' };
  if (run.status === 'succeeded') return { text: playedText(run.result) };
  if (run.status === 'needs_attention')
    return { text: 'The playbook is waiting for you in the browser.', failed: true };
  return { error: run.error || 'The playbook failed.' };
}
