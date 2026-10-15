/**
 * The address bar's translate chip: what it says for the active tab's
 * translation (which main owns and sends with the tab list), what a click does
 * in each state, and the one line a screen reader hears when it changes.
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { TabSummary, TabTranslation } from '../../../../shared/ipc.ts';
import { languageName } from '../../../../shared/language.ts';
import { TRANSLATE_TEXT as T } from '../model/constants.ts';

/** The parts of the bridge the chip uses. */
export type TranslateBridge = Pick<OyaBrowser, 'translatePage' | 'showOriginal' | 'onTabsUpdated'>;

/** Where a setup or refusal click takes the person: the root wires each to the Assistant panel. */
export interface TranslateHost {
  /** Opens the Assistant panel, where the sign-in card is. */
  openAsk(): void;
  /** Opens the Assistant panel's model card. */
  openModels(): void;
  /** Opens the page that turns this browser's key into an account. */
  openClaim(): void;
}

/** What a click does: translate, show the original, open a host's page, or nothing. */
type Action = 'translate' | 'original' | 'ask' | 'models' | 'claim' | 'none';

/** How the chip looks. */
export interface ChipLook {
  /** Its visible text, which is also its accessible name. */
  text: string;
  /** The full reason: its tooltip and description. */
  title: string;
  /** Its colour: plain, accent once translated, danger on a failure. */
  tone: 'idle' | 'done' | 'failed';
  /** What a click does. */
  action: Action;
}

/** The chip's state: its look (null: hidden) and the last announcement. */
export interface TranslateState {
  /** How the chip looks, or null when it is hidden. */
  chip: ChipLook | null;
  /** What the live line last said. */
  said: string;
}

/** One failure's words: the chip's text, a fixed reason (else the server's own), its tone and its click. */
type Refusal = [text: string, title: string | null, action: Action, tone?: ChipLook['tone']];

/** Each failure code's words. Anything unlisted is "Couldn't translate" with the server's message. */
const REFUSALS: Record<string, Refusal> = {
  signed_out: [T.signIn, T.signInTitle, 'ask', 'idle'],
  llm_unconfigured: [T.connect, T.connectTitle, 'models', 'idle'],
  llm_bring_your_own: [T.connect, T.connectTitle, 'models', 'idle'],
  llm_not_allowed: [T.notAllowed, null, 'models'],
  llm_rejected: [T.keyRefused, null, 'models'],
  llm_payment_required: [T.payment, null, 'translate'],
  llm_unavailable: [T.unavailable, null, 'translate'],
  llm_quota: [T.limit, T.limitTitle, 'translate'],
  rate_limited: [T.rate, T.rateTitle, 'translate'],
  claim_required: [T.claim, null, 'claim'],
  offline: [T.offline, T.offlineTitle, 'translate'],
  timeout: [T.timeout, T.timeoutTitle, 'translate'],
  nothing_to_translate: [T.empty, T.emptyTitle, 'none', 'idle'],
  translate_malformed: [T.failed, T.malformedTitle, 'translate'],
};

/** `text` with `{lang}` and `{target}` filled in by name, in the person's language. */
function fill(text: string, t: TabTranslation): string {
  const name = (tag: string) => languageName(tag, t.target || 'en');
  return text.replace('{lang}', t.lang ? name(t.lang) : '').replace('{target}', name(t.target));
}

/** The chip for a failure. */
function failedLook(t: TabTranslation): ChipLook {
  const code = t.error?.code ?? '';
  const [text, title, action, tone = 'failed'] = Object.hasOwn(REFUSALS, code)
    ? REFUSALS[code]
    : ([T.failed, null, 'translate'] as Refusal);
  return { text, title: title ?? t.error?.message ?? '', tone, action };
}

/** The chip once the page is translated, in whole or in part. */
function doneLook(t: TabTranslation): ChipLook {
  if (t.state === 'partial')
    return { text: T.partial, title: fill(T.partialTitle, t), tone: 'done', action: 'original' };
  const text = fill(t.lang ? T.translated : T.translatedTo, t);
  return { text, title: text, tone: 'done', action: 'original' };
}

/** How the chip looks for a tab's translation; null hides it. */
export function chipLook(t: TabTranslation | undefined): ChipLook | null {
  if (!t) return null;
  if (t.state === 'offer')
    return { text: fill(T.offer, t), title: fill(T.offerTitle, t), tone: 'idle', action: 'translate' };
  if (t.state === 'translating') {
    return { text: T.translating, title: fill(T.translatingTitle, t), tone: 'idle', action: 'original' };
  }
  return t.state === 'failed' ? failedLook(t) : doneLook(t);
}

/** What a screen reader hears when a tab's chip goes from `before` to `now`; '' for nothing (an offer appearing, a tab switch). */
export function announcement(before: TabTranslation | undefined, now: TabTranslation | undefined): string {
  if (!now || before?.state === now.state) return '';
  if (now.state === 'translating') return fill(T.saidStart, now);
  if (now.state === 'translated') return fill(T.saidDone, now);
  if (now.state === 'partial') return T.saidPartial;
  if (now.state === 'failed') return chipLook(now)?.title ?? '';
  return before && before.state !== 'failed' ? T.saidOriginal : '';
}

/** The translate chip, following the active tab. */
export class TranslateViewModel extends ViewModel<TranslateState> {
  /** The main process. */
  private readonly bridge: TranslateBridge;
  /** Where setup and refusal clicks go. */
  private readonly host: TranslateHost | undefined;
  /** The active tab and its translation, as last drawn. */
  private shown: Pick<Partial<TabSummary>, 'id' | 'translation'> = {};

  /** Hidden until a tab list says otherwise; `host` handles clicks that lead elsewhere. */
  constructor(bridge: TranslateBridge, host?: TranslateHost) {
    super({ chip: null, said: '' });
    this.bridge = bridge;
    this.host = host;
    this.own(bridge.onTabsUpdated((tabs) => this.follow(tabs)));
  }

  /** Draws the active tab's chip, announcing a change on the same tab only. */
  private follow(tabs: readonly TabSummary[]): void {
    const active = tabs.find((t) => t.active);
    const translation = /^https?:/i.test(active?.url ?? '') ? active?.translation : undefined;
    const said = active?.id === this.shown.id ? announcement(this.shown.translation, translation) : '';
    this.shown = { id: active?.id, translation };
    this.set({ chip: chipLook(translation), ...(said ? { said } : {}) });
  }

  /** What each click does, for the tab `id`. */
  private actions(id: number): Record<Action, () => unknown> {
    return {
      translate: () => this.bridge.translatePage(id),
      original: () => this.bridge.showOriginal(id),
      ask: () => this.host?.openAsk(),
      models: () => this.host?.openModels(),
      claim: () => this.host?.openClaim(),
      none: () => undefined,
    };
  }

  /** The chip was clicked: does what its state says. A refusal from main shows on the chip, so it is not repeated here. */
  activate(): void {
    const id = this.shown.id;
    const action = this.state.chip?.action;
    if (id === undefined || !action) return;
    void Promise.resolve(this.actions(id)[action]()).catch(() => undefined);
  }
}
