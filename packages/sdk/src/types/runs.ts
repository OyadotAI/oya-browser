/**
 * Types for agent work: task values and files, playbooks and their replays,
 * and background runs with their attention requests.
 */
import type { OyaError } from '../errors.js';

/** A saved flow, replayable without an LLM. */
export interface Playbook {
  /** The name it is played by. */
  name: string;
  /** Inputs `play()` accepts; any left out reuse the recorded value. */
  variables: string[];
  /** What each variable was recorded with. A secret has none, it never left the page. */
  defaults: Record<string, string>;
  /** How many steps it replays. */
  steps: number;
  /** The same flow as a Playwright module: `export default async function run(page, vars)`. */
  code: string;
}

/** What a `play()` did. */
export interface PlayResult {
  /** Steps replayed before finishing or handing over to the agent. */
  steps: number;
  /** Steps in the playbook. */
  total: number;
  /** A step no longer fit the page and the agent finished the task. */
  fellBack: boolean;
  /** The agent's fix was saved as `draft`; promote it with `oya.playbooks.promote(name)`. */
  healed?: boolean;
  /** The draft's name, when one was saved. */
  draft?: string;
  /** The agent's summary, when it fell back. */
  text?: string;
}

/** A playbook in the listing, with its history and any pending fix. */
export interface PlaybookSummary extends Playbook {
  /** When it was saved. */
  createdAt: string | null;
  /** When a draft last replaced it. */
  promotedAt: string | null;
  /** A healed replay's fix, waiting for `promote()` or `remove('<name>:draft')`. */
  draft:
    | (Playbook & {
        /** When the replay was healed. */
        healedAt: string;
        /** The step the replay broke at. */
        healedFrom: number;
      })
    | null;
}

/**
 * A file attached to a task value. Build it with `file()`, never by hand. Only `data`
 * takes one: a file is not typed through a placeholder, so `secrets` has nothing to hide
 * and rejects it.
 */
export interface FileValue {
  /** The filename the site sees. */
  file: string;
  /** MIME type, guessed from the extension unless you pass one. */
  type: string;
  /** The bytes, base64. 10MB ceiling. */
  b64: string;
}

/**
 * Task values, referred to as `{{name}}` in prompts. As `data` the agent can read them
 * (to split a name or pick the right option); as `secrets` it never sees them. Either
 * way they are typed through placeholders, so playbooks store no values.
 *
 * A {@link FileValue} from `file()` is the exception: the agent attaches it with its
 * upload tool rather than typing it.
 */
export type RunData = Record<string, string | number | FileValue>;

/** A run is waiting on a person. */
export interface AttentionRequest {
  /** Identifies this request; a new one means a new problem. */
  id: string;
  /** captcha / login / mfa: finish it in the live view. agent: the agent's question. heal_failed: replay and the agent both gave up. */
  reason: 'captcha' | 'login' | 'mfa' | 'agent' | 'heal_failed';
  /** What is needed, in words. */
  message: string;
  /** Where to finish it by hand. */
  liveViewUrl?: string;
  /** When it was raised, in epoch milliseconds. */
  at: number;
}

/** What a finished run produced: a replay's result, an agent's answer, or both. */
export type RunResult = Partial<PlayResult> & {
  /** The agent's answer. */
  text?: string;
};

/** A background run's state. */
export interface RunInfo {
  /** The run's id. */
  id: string;
  /** The browser it runs on. */
  browserId: string;
  /** Where it is. */
  status: 'running' | 'needs_attention' | 'succeeded' | 'failed';
  /** When it started, in epoch milliseconds. */
  createdAt: number;
  /** When it ended. */
  endedAt?: number;
  /** The open request for a person, if any. */
  attention: AttentionRequest | null;
  /** What it produced, once it succeeded. */
  result?: RunResult;
  /** Why it failed. */
  error?: string;
  /** HTTP-style status of a failure: 429 when a quota stopped the run. */
  errorStatus?: number;
}

/** The inputs and callbacks for `browser.submit()`. */
export interface SubmitOptions {
  /** Task values the agent can read; for a playbook, its variables (secret ones included). */
  data?: RunData;
  /** Prompts only: values the agent never sees, like passwords. A playbook already knows which of its variables are secret. */
  secrets?: RunData;
  /** Playbooks only: let the agent finish a broken replay and save its fix as a draft. Default true. */
  autoHeal?: boolean;
  /** Fires once with the result when the run succeeds. */
  onSuccess?: (result: RunResult) => unknown;
  /** Fires once when the run fails, or when it can no longer be polled. */
  onFailure?: (error: OyaError) => unknown;
  /** Call `respond()` once it is handled: `'done'` after finishing by hand, or your answer to the agent. */
  onHumanAttention?: (request: AttentionRequest & { respond(response?: string): Promise<void> }) => unknown;
  /** Fires before onSuccess when a replay was healed; `result.draft` names the draft. */
  onHealed?: (result: RunResult) => unknown;
  /** How often to check on the run. Default 2000. */
  pollMs?: number;
}
