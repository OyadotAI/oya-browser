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

  /** Hidden until main sends an explicit pending confirmation. */
  constructor(bridge: TranslationDialogBridge) {
    super({ confirmation: null });
    this.bridge = bridge;
    this.own(() => this.follow(undefined));
  }

  /** Follow only the selected tab's pending proposal. */
  follow(tab: TabSummary | undefined): void {
    const t = tab?.translation;
    const proposal = tab && t?.state === 'confirming' && t.confirmation ? { tabId: tab.id, translation: t } : null;
    const key = proposal?.translation.confirmation?.id || '';
    if (key === this.wanted) return;
    this.wanted = key;
    this.changing = this.changing.then(() => this.draw(proposal, key)).catch(() => this.cancelProposal(proposal));
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
  private async cancelProposal(proposal: TranslationConfirmation | null): Promise<void> {
    this.set({ confirmation: null });
    if (proposal) await this.bridge.showOriginal(proposal.tabId).catch(() => {});
    await this.bridge.hideOverlay(TRANSLATE_OVERLAY).catch(() => {});
  }

  /** The only action that approves sending page text; main rechecks its document and scope. */
  confirm(): void {
    const proposal = this.state.confirmation;
    const id = proposal?.translation.confirmation?.id;
    if (proposal && id) void this.bridge.confirmTranslation(proposal.tabId, id).catch(() => {});
  }

  /** Escape, backdrop and Cancel all revoke the pending proposal. */
  cancel(): void {
    const proposal = this.state.confirmation;
    if (proposal) void this.bridge.showOriginal(proposal.tabId).catch(() => {});
  }
}
