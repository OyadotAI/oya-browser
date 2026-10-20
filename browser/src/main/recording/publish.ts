/**
 * Hand the recording to the server, which saves it as a playbook and returns
 * its Playwright code; the workspace marks the draft published. A draft saved
 * before under another name is renamed first, with the server's own rename
 * (PATCH /playbooks/:name, there since the first server), so a new name moves
 * the playbook instead of leaving the old one beside a second, on any server.
 */
import type { AppServices } from '../app/services.ts';
import type { SaveRename, SaveResult } from '../../shared/ipc.ts';
import { ServerApi } from '../connection/server-api.ts';
import { accountOf } from '../app/saved-accounts.ts';
import { savedAs } from '../workflow/published.ts';
import { SAVE_TIMEOUT_MS } from '../connection/constants.ts';
import { CONFLICT, NOT_FOUND, PLAYBOOK_SCHEMA_VERSION } from './constants.ts';
import type { Recorder } from './recorder.ts';

/** The services a save uses: the recording, the draft, and the server connection. */
type Deps = Pick<AppServices, 'recorder' | 'workspace' | 'socket' | 'config'>;

/** The draft revision a save sent, to mark published only if it is still that one. */
interface Sent {
  /** The draft's id. */
  id?: string;
  /** Its revision. */
  revision?: number;
  /** The playbook name it was saved under. */
  name: string;
  /** The server and project it was saved to (an id that reveals neither). */
  account: string;
}

/** A save that failed. */
export interface SaveError extends SaveResult {
  /** Why, for a person. */
  error: string;
}

/** What a save answers: the server's body (with the rename, if one happened), or why it failed. */
export type SaveAnswer = (Record<string, unknown> & SaveResult) | SaveError;

/** The recorded steps without their capture timestamps, and the secret names. */
function recorded({ recordedSteps, recordedSecrets }: Pick<Recorder, 'recordedSteps' | 'recordedSecrets'>) {
  return { steps: recordedSteps.map(({ t, ...step }) => step), secrets: [...recordedSecrets] };
}

/** A failed call's words: a timeout says so, anything else keeps the server's own. */
function failureText(err: unknown, timedOut: string): string {
  return (err as Error).name === 'TimeoutError' ? timedOut : (err as Error).message;
}

/** A failed call's HTTP status, when the server answered at all. */
const statusOf = (err: unknown): number | undefined => (err as Partial<Record<'status', number>>).status;

/** A refused rename: a playbook already gone lets the save go on; anything else stops it, saying why. */
function renameRefusal(from: string, err: unknown): SaveRename | SaveError {
  const status = statusOf(err);
  if (status === NOT_FOUND) return { from, missing: true };
  const timedOut = (err as Error).name === 'TimeoutError';
  const error = failureText(err, 'The server did not answer in time');
  return { error, renameFailed: { from, ...(status ? { status } : {}), ...(timedOut ? { timedOut } : {}) } };
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

  /**
   * Stops any recording and publishes what was recorded. Under a new name it first renames the
   * playbook the draft was last saved as; `asCopy` saves a new one and leaves that alone.
   */
  async save(name: string, description?: string, asCopy = false): Promise<SaveAnswer> {
    const recorder = this.deps.recorder;
    if (!this.api.canCall()) return { error: 'Not connected to server' };
    if (recorder.recording) await recorder.stopRecording();
    if (!recorder.recordedSteps.length) return { error: 'Nothing recorded yet' };
    const renamed = await this.renameFirst(name, asCopy);
    if (renamed && 'error' in renamed) return renamed;
    const answer = await this.post(name, description);
    return renamed ? { ...answer, renamed } : answer;
  }

  /** This browser's server and project now, as the id a saved draft records. */
  private account(): string {
    return accountOf(this.deps.config.values);
  }

  /**
   * The playbook this save renames: the one the draft was saved as, when the name differs, this is
   * no copy, and the draft was saved to this same server and project. After switching accounts the
   * old name means nothing there, and renaming would move someone else's playbook of that name.
   */
  private renaming(name: string, asCopy: boolean): string | null {
    const saved = savedAs(this.deps.workspace?.draft, this.account());
    return asCopy || saved === name ? null : saved;
  }

  /** Renames the saved playbook before the save; null when there is nothing to rename. */
  private async renameFirst(name: string, asCopy: boolean): Promise<SaveRename | SaveError | null> {
    const from = this.renaming(name, asCopy);
    if (!from) return null;
    const draftId = this.deps.workspace?.draft.id;
    const refused = await this.rename(from, name);
    if (refused && 'missing' in refused) return (await this.takenRefusal(from, name)) ?? refused;
    if (refused) return refused;
    this.keepName(draftId, name);
    return { from };
  }

  /** The server's own rename of playbook `from` to `name`: null once done, else why not. */
  private async rename(from: string, name: string): Promise<SaveRename | SaveError | null> {
    try {
      await this.api.send('PATCH', `playbooks/${encodeURIComponent(from)}`, { name }, SAVE_TIMEOUT_MS);
      return null;
    } catch (err) {
      return renameRefusal(from, err);
    }
  }

  /**
   * With the old playbook not found, whether `name` is taken: the save would overwrite that
   * playbook without a word, so it stops instead. Null when the name is free.
   */
  private async takenRefusal(from: string, name: string): Promise<SaveError | null> {
    try {
      await this.api.send('GET', `playbooks/${encodeURIComponent(name)}/export`, undefined, SAVE_TIMEOUT_MS);
      return { error: `A playbook named ${name} already exists`, renameFailed: { from, status: CONFLICT } };
    } catch (err) {
      if (statusOf(err) === NOT_FOUND) return null;
      return { error: failureText(err, 'The server did not answer in time'), renameFailed: { from } };
    }
  }

  /**
   * The server now knows the playbook by `name`: the draft says so at once, so a save that fails
   * after the rename is retried as a plain save, never a second rename. Its revision is untouched.
   */
  private keepName(draftId: string | undefined, name: string): void {
    const workspace = this.deps.workspace;
    if (!workspace || workspace.draft.id !== draftId) return;
    Object.assign(workspace.draft, { publishedName: name, publishedTo: this.account() });
    workspace.persist();
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
    const draft = this.deps.workspace?.draft;
    const sent: Sent = { id: draft?.id, revision: draft?.revision, name, account: this.account() };
    try {
      const res = await this.api.postToBrowser('playbooks', this.body(name, description), SAVE_TIMEOUT_MS);
      return await this.readAnswer(res, sent);
    } catch (err) {
      return { error: failureText(err, 'The server took too long to save. Try again.') };
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
    const { revision: publishedRevision, name: publishedName, account: publishedTo } = sent;
    Object.assign(workspace.draft, { publishedAt: Date.now(), publishedRevision, publishedName, publishedTo });
    workspace.persist();
  }
}
