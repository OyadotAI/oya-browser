/** Serializes native overlay ownership around the main-owned translation confirmation. */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { TabSummary, TabTranslation } from '../../../../shared/ipc.ts';
import { languageName } from '../../../../shared/language.ts';
import { TRANSLATE_OVERLAY, TRANSLATE_CONSENT as T } from '../model/constants.ts';

/** Native overlay and confirmation operations used by the dialog. */
export type TranslationDialogBridge = Pick<
  OyaBrowser,
  'showOverlay' | 'hideOverlay' | 'confirmTranslation' | 'showOriginal'
>;
/** The exact tab and main-owned proposal currently displayed. */
export interface TranslationConfirmation {
  /** The tab that requested translation. */
  tabId: number;
  /** Includes the opaque confirmation id and disclosed site/language scope. */
  translation: TabTranslation;
}
/** A hidden dialog has no confirmation. */
interface State {
  /** Published only after the native page has been lowered behind the overlay. */
  confirmation: TranslationConfirmation | null;
  /** Local bridge progress belongs only to the currently displayed proposal. */
  action: 'confirm' | 'cancel' | 'confirm_failed' | 'cancel_failed' | null;
}

/** Local bridge progress never claims a main-process outcome that has not been published. */
const actionStatus = {
  confirm: T.starting,
  cancel: T.cancelling,
  confirm_failed: T.confirmFailed,
  cancel_failed: T.cancelFailed,
};
/** Stable buttons retain focus while guarded actions and their status follow local bridge progress. */
export function confirmationActions(action: State['action']) {
  return {
    status: action ? actionStatus[action] : '',
    failed: !!action?.endsWith('_failed'),
    confirmBlocked: !!action && action !== 'confirm_failed',
    cancelBlocked: action === 'cancel',
    confirmLabel: action === 'confirm_failed' ? 'Try again' : 'Translate',
    cancelLabel: action === 'cancel_failed' ? 'Retry cancel' : 'Cancel',
  };
}

/** Confirmation copy derives from the exact main-owned proposal. */
export function confirmationText({ translation: t }: TranslationConfirmation) {
  const name = (tag: string) => languageName(tag, t.target);
  const continuation = t.confirmation?.pages
    ? T.continuation(name(t.lang), t.confirmation.origin, t.confirmation.pages)
    : T.pageOnly;
  return { title: T.title(name(t.target)), disclosure: T.disclosure, continuation, cancellation: T.cancellation };
}

/** One window's dialog; switching tabs cannot accept a proposal from another tab. */
export class TranslateDialogViewModel extends ViewModel<State> {
  /** Native operations remain behind the trusted shell bridge. */
  private readonly bridge: TranslationDialogBridge;
  /** Latest requested proposal id; stale overlay completions cannot reopen an older dialog. */
  private wanted = '';
  /** Overlay changes are serialized because main owns one named overlay slot. */
  private changing = Promise.resolve();
  /** Cancel supersedes a pending confirm; stale completions cannot affect its status. */
  private actionGeneration = 0;
  /** Disposed shell models cannot reopen or retain stale proposal state. */
  private disposed = false;

  /** Hidden until main sends an explicit pending confirmation. */
  constructor(bridge: TranslationDialogBridge) {
    super({ confirmation: null, action: null });
    this.bridge = bridge;
    this.own(() => {
      this.disposed = true;
      this.follow(undefined);
    });
  }

  /** Follow only the selected tab's pending proposal. */
  follow(tab: TabSummary | undefined): void {
    const t = this.disposed ? undefined : tab?.translation;
    const proposal = tab && t?.state === 'confirming' && t.confirmation ? { tabId: tab.id, translation: t } : null;
    const key = proposal ? `${proposal.tabId}:${proposal.translation.confirmation!.id}` : '';
    this.request(proposal, key);
  }

  /** Invalidate action feedback synchronously while native overlay transitions remain serialized. */
  private request(proposal: TranslationConfirmation | null, key: string): void {
    if (key === this.wanted) return;
    this.wanted = key;
    this.actionGeneration++;
    this.set({ confirmation: null, action: null });
    this.changing = this.changing.then(() => this.draw(proposal, key)).catch(() => this.cancelProposal(proposal, key));
  }

  /** Native page removal finishes before the focusable dialog appears. */
  private async draw(proposal: TranslationConfirmation | null, key: string): Promise<void> {
    if (key !== this.wanted) return;
    if (proposal) await this.bridge.showOverlay(TRANSLATE_OVERLAY);
    if (key !== this.wanted) return;
    this.set({ confirmation: proposal });
    if (!proposal) await this.bridge.hideOverlay(TRANSLATE_OVERLAY);
  }

  /** Failure to show consent must not leave a hidden proposal that could send page text. */
  private async cancelProposal(proposal: TranslationConfirmation | null, key: string): Promise<void> {
    if (key !== this.wanted) return;
    this.set({ confirmation: null });
    if (proposal) await this.bridge.showOriginal(proposal.tabId).catch(() => {});
    await this.bridge.hideOverlay(TRANSLATE_OVERLAY).catch(() => {});
  }

  /** The only action that approves sending page text; main rechecks its document and scope. */
  confirm(): void {
    if (!confirmationActions(this.state.action).confirmBlocked) this.perform('confirm');
  }

  /** Escape, backdrop and Cancel all revoke the pending proposal. */
  cancel(): void {
    if (!confirmationActions(this.state.action).cancelBlocked) this.perform('cancel');
  }

  /** Main state remains authoritative; promise success alone cannot close consent or claim translation started. */
  private perform(action: 'confirm' | 'cancel'): void {
    const proposal = this.state.confirmation;
    if (!proposal || this.disposed) return;
    const generation = ++this.actionGeneration;
    this.set({ action });
    void this.send(proposal, action).catch(() => this.failed(proposal, generation, action));
  }

  /** Only a current browser-owned action can send the proposal id to the trusted bridge. */
  private send(proposal: TranslationConfirmation, action: 'confirm' | 'cancel'): Promise<unknown> {
    return action === 'confirm'
      ? this.bridge.confirmTranslation(proposal.tabId, proposal.translation.confirmation!.id)
      : this.bridge.showOriginal(proposal.tabId);
  }

  /** Rejections belong to one action and one exact proposal, never a new tab or a later cancellation intent. */
  private failed(proposal: TranslationConfirmation, generation: number, action: 'confirm' | 'cancel'): void {
    if (this.disposed || generation !== this.actionGeneration || this.state.confirmation !== proposal) return;
    this.set({ action: action === 'confirm' ? 'confirm_failed' : 'cancel_failed' });
  }
}
