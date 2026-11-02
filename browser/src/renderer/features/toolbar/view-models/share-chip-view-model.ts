/**
 * The toolbar's share chip: while a call captures a tab, every other tab says
 * what is shared and offers the way back to the call. Oya cannot move a live
 * share to another tab, so the chip never offers to; it only takes the person
 * to the call, where they can stop and share again.
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { ShareChip, TabSummary } from '../../../../shared/ipc.ts';
import { SHARE_TEXT } from '../model/constants.ts';

/** What the chip shows. */
export interface ShareChipState {
  /** Whether it shows: a share is live and the person is on neither its tab nor the call's. */
  visible: boolean;
  /** "Sharing zoom.us". */
  status: string;
  /** Its accessible name. */
  label: string;
  /** Its tooltip, which says why it does not switch the share. */
  title: string;
}

/** The parts of the bridge the chip uses. */
export type ShareChipBridge = Pick<OyaBrowser, 'onShareChip' | 'onTabsUpdated' | 'goToShareCall'>;

/** Hidden, with nothing to say. */
const HIDDEN: ShareChipState = { visible: false, status: '', label: '', title: '' };

/** The chip's words for a share of `shared` with `call`, each a host or null. */
function chipWords(shared: string | null, call: string | null): Omit<ShareChipState, 'visible'> {
  const what = shared ?? SHARE_TEXT.oyaPage;
  const where = call ?? SHARE_TEXT.theCall;
  return {
    status: SHARE_TEXT.status(what),
    label: SHARE_TEXT.label(what, where),
    title: SHARE_TEXT.title(what, where),
  };
}

/** The chip for `share` while `active` is the tab on screen. */
export function chipState(share: ShareChip | null, active: number | undefined): ShareChipState {
  if (!share || active === undefined || active === share.sharedTabId || active === share.callTabId) return HIDDEN;
  return { visible: true, ...chipWords(share.shared, share.call) };
}

/** The share chip. */
export class ShareChipViewModel extends ViewModel<ShareChipState> {
  /** The main process. */
  private readonly bridge: ShareChipBridge;
  /** The live share, as main last said. */
  private share: ShareChip | null = null;
  /** The tab on screen in this window. */
  private active: number | undefined;

  /** Hidden until main reports a share, then following it and the active tab. */
  constructor(bridge: ShareChipBridge) {
    super(HIDDEN);
    this.bridge = bridge;
    this.own(bridge.onShareChip((share) => ((this.share = share), this.update())));
    this.own(bridge.onTabsUpdated((tabs) => this.tabsChanged(tabs)));
  }

  /** Takes the person to the call; a refusal (an agent holds the browser) leaves everything where it was. */
  goToCall(): void {
    void this.bridge.goToShareCall().catch(() => {});
  }

  /** The strip changed: which tab is on screen. */
  private tabsChanged(tabs: readonly TabSummary[]): void {
    this.active = tabs.find((tab) => tab.active)?.id;
    this.update();
  }

  /** Redraws from the share and the active tab. */
  private update(): void {
    this.set(chipState(this.share, this.active));
  }
}
