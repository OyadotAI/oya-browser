/**
 * Hand the recording to the server, which saves it as a playbook and returns
 * its Playwright code; the workspace marks the draft published.
 */
import type { AppServices } from '../app/services.ts';
import { ServerApi } from '../connection/server-api.ts';
import { SAVE_TIMEOUT_MS } from '../connection/constants.ts';
import { PLAYBOOK_SCHEMA_VERSION } from './constants.ts';
import type { Recorder } from './recorder.ts';

/** The services a save uses: the recording, the draft, and the server connection. */
type Deps = Pick<AppServices, 'recorder' | 'workspace' | 'socket' | 'config'>;

/** The draft revision a save sent, to mark published only if it is still that one. */
interface Sent {
  /** The draft's id. */
  id?: string;
  /** Its revision. */
  revision?: number;
}

/** A save that failed. */
export interface SaveError {
  /** Why, for a person. */
  error: string;
}

/** What a save answers: the server's body, or why it failed. */
export type SaveAnswer = Record<string, unknown> | SaveError;

/** The recorded steps without their capture timestamps, and the secret names. */
function recorded({ recordedSteps, recordedSecrets }: Pick<Recorder, 'recordedSteps' | 'recordedSecrets'>) {
  return { steps: recordedSteps.map(({ t, ...step }) => step), secrets: [...recordedSecrets] };
}

/** Saves the recording as a playbook on the server. */
export class RecordingPublisher {
  /** The services a save uses. */
  private readonly deps: Deps;
  /** The server's HTTP API, called as this browser. */
  private readonly api: ServerApi;

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
    this.api = new ServerApi(deps);
  }

  /** Stops any recording and publishes what was recorded. */
  async save(name: string, description?: string): Promise<SaveAnswer> {
    const recorder = this.deps.recorder;
    if (!this.api.canCall()) return { error: 'Not connected to server' };
    if (recorder.recording) await recorder.stopRecording();
    if (!recorder.recordedSteps.length) return { error: 'Nothing recorded yet' };
    return this.post(name, description);
  }

  /** The playbook the server is sent. Capture timestamps stay here. */
  private body(name: string, description?: string) {
    const variables = this.deps.workspace?.draft.variables || {};
    return {
      schemaVersion: PLAYBOOK_SCHEMA_VERSION,
      variables,
      name,
      prompt: description,
      ...recorded(this.deps.recorder),
    };
  }

  /** Posts the playbook; answers the server's body, or `{ error }`. */
  private async post(name: string, description?: string): Promise<SaveAnswer> {
    const sent: Sent = { id: this.deps.workspace?.draft.id, revision: this.deps.workspace?.draft.revision };
    try {
      const res = await this.api.postToBrowser('playbooks', this.body(name, description), SAVE_TIMEOUT_MS);
      return await this.readAnswer(res, sent);
    } catch (err) {
      if ((err as Error).name === 'TimeoutError') return { error: 'The server took too long to save. Try again.' };
      return { error: (err as Error).message };
    }
  }

  /** The server's answer; a success marks the draft published, and a refusal is always an error. */
  private async readAnswer(res: Response, sent: Sent): Promise<SaveAnswer> {
    const body = await res.json().catch(() => ({}));
    if (!res.ok) return { error: body.error || `Server returned ${res.status}` };
    this.markPublished(sent);
    return body;
  }

  /** Marks the draft published, if it is still the revision that was sent. */
  private markPublished(sent: Sent): void {
    const workspace = this.deps.workspace;
    if (!workspace || workspace.draft.id !== sent.id || workspace.draft.revision !== sent.revision) return;
    Object.assign(workspace.draft, { publishedAt: Date.now(), publishedRevision: sent.revision });
    workspace.persist();
  }
}
