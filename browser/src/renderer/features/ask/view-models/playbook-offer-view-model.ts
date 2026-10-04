/**
 * "Save as playbook" under an agent reply. The server keeps the steps of each
 * browser's latest agent run, so only the newest reply offers to save (the
 * conversation withdraws an older offer). A run that only read pages shows
 * the offer disabled with the reason, so it never seems to come and go at
 * random. A saved offer stays as its confirmation.
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import { ASK_TEXT, PLAYBOOK_NAME } from '../model/constants.ts';

/** Where the offer is: the button, the name form, saving, or saved. */
export type OfferStage = 'offer' | 'form' | 'saving' | 'saved';

/** What the offer shows. */
export interface PlaybookOfferState {
  /** Whether the run can be replayed (else the button shows disabled, with why). */
  canSave: boolean;
  /** Where the offer is. */
  stage: OfferStage;
  /** The name in the field (the prompt's slug to start). */
  name: string;
  /** Why saving failed, or ''. */
  error: string;
  /** The name it was saved under. */
  savedAs: string;
}

/** The offer under one reply. */
export class PlaybookOfferViewModel extends ViewModel<PlaybookOfferState> {
  /** The main process, which saves the server's copy of the run. */
  private readonly bridge: Pick<OyaBrowser, 'saveChatPlaybook'>;

  /** The collapsed offer, its name suggested from the prompt. */
  constructor(bridge: Pick<OyaBrowser, 'saveChatPlaybook'>, name: string, canSave: boolean) {
    super({ canSave, stage: 'offer', name, error: '', savedAs: '' });
    this.bridge = bridge;
  }

  /** "Save as playbook" was pressed: the name form. */
  openForm(): void {
    if (this.state.canSave) this.set({ stage: 'form', error: '' });
  }

  /** Cancel: back to the button. */
  cancel(): void {
    this.set({ stage: 'offer', error: '' });
  }

  /** The name was typed. */
  setName(name: string): void {
    this.set({ name });
  }

  /** Saves the run under the typed name, or explains why it could not. */
  async save(): Promise<void> {
    const name = this.state.name.trim();
    if (!PLAYBOOK_NAME.test(name)) return this.set({ error: ASK_TEXT.badName });
    this.set({ stage: 'saving', error: '' });
    const result = await this.bridge.saveChatPlaybook(name).catch((e: Error) => ({ error: e.message }));
    if (result?.error) return this.set({ stage: 'form', error: result.error });
    this.set({ stage: 'saved', savedAs: name });
  }
}
