/**
 * Which tabs a call is capturing. Electron cannot move a live share to another tab,
 * so the shell can only say what is shared: a ring on the shared tab and a chip that
 * takes the person back to the call. Desktop sources never match a tab, so window
 * and screen shares are left out: nothing tells us when those end.
 */
import type { WebContents } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { ShareChip } from '../../shared/ipc.ts';
import type { Tab } from '../tabs/types.ts';
import type { ShareVideo } from './sources.ts';
import { webHost } from './sources.ts';
import { SHARE_POLL_MS } from './constants.ts';

/** The routers (every window's tabs and shells) and window ownership. */
type Deps = Pick<AppServices, 'tabs' | 'shell' | 'windows'>;

/** Live tab shares, polled for their end only while one exists. */
export class ShareIndicator {
  /** Routers wired in the composition root. */
  private readonly deps: Deps;
  /** Shared tab id → calling tab id, oldest first, so the last entry is the most recent share. */
  private readonly shares = new Map<number, number>();
  /** The end-of-share poll, running only while a share is tracked. */
  private timer: NodeJS.Timeout | null = null;
  /** Whether the shells were last told of a chip, so its end is sent exactly once. */
  private chipShown = false;

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** A share was handed to `caller`; tracked only when its source is one of our tabs. */
  started(video: ShareVideo, caller: WebContents): void {
    const shared = this.tabOf((contents) => contents.mainFrame === video);
    const call = this.tabOf((contents) => contents === caller);
    if (!shared || !call) return;
    this.shares.delete(shared.id);
    this.shares.set(shared.id, call.id);
    this.timer ??= setInterval(() => this.tick(), SHARE_POLL_MS);
    this.deps.tabs.sendTabList();
  }

  /** Whether a call is capturing this tab. */
  has(id: number): boolean {
    return this.shares.has(id);
  }

  /** The most recent share whose call tab is still open, as the toolbar names it. */
  chip(): ShareChip | null {
    const live = [...this.shares].map(([shared, call]) => [this.find(shared), this.find(call)] as const);
    const [shared, call] = live.findLast(([s, c]) => s && c) ?? [];
    if (!shared || !call) return null;
    return { sharedTabId: shared.id, callTabId: call.id, shared: webHost(shared.url), call: webHost(call.url) };
  }

  /**
   * A strip changed in some window: drop closed tabs at once and tell every shell
   * the chip, so a window opened mid-share learns it too. Runs inside sendTabList,
   * so it must never call sendTabList itself.
   */
  tabsChanged(): void {
    this.prune((id) => !this.find(id));
    const chip = this.chip();
    if (chip || this.chipShown) this.deps.shell.send('share-chip', chip);
    this.chipShown = !!chip;
  }

  /** Shows the call tab: its window first, which control may refuse, so a refusal moves nothing. */
  goToCall(): void {
    const callId = this.chip()?.callTabId;
    const scope = callId === undefined ? undefined : this.deps.windows?.owner(callId);
    if (!scope || callId === undefined) return;
    this.deps.windows!.present(scope);
    scope.tabs.activateTab(callId);
  }

  /**
   * The first false capture reading ends a share; true is only probable.
   * ponytail: our own capturePage (thumbnails, screenshots) holds the count while in
   * flight, so a poll landing mid-capture clears the ring one tick late. No start-up
   * grace: getDisplayMedia resolves after the count rises (pinned in the integration
   * suite). Add one if a runtime ever shows the ring failing to appear.
   */
  private tick(): void {
    const ended = (id: number): boolean => {
      const contents = this.find(id)?.view.webContents;
      return !contents || contents.isDestroyed() || !contents.isBeingCaptured();
    };
    if (this.prune(ended)) this.deps.tabs.sendTabList();
  }

  /** Forgets the shares `gone` names and stops the poll when none is left; whether any went. */
  private prune(gone: (id: number) => boolean): boolean {
    const before = this.shares.size;
    for (const id of [...this.shares.keys()]) if (gone(id)) this.shares.delete(id);
    if (!this.shares.size && this.timer) clearInterval(this.timer);
    if (!this.shares.size) this.timer = null;
    return this.shares.size !== before;
  }

  /** An open tab, by id, in any window. */
  private find(id: number | undefined): Tab | undefined {
    return id === undefined ? undefined : this.deps.tabs.list.find((tab) => tab.id === id);
  }

  /** The open tab whose live page passes `match`. */
  private tabOf(match: (contents: WebContents) => boolean): Tab | undefined {
    return this.deps.tabs.list.find((tab) => !tab.view.webContents.isDestroyed() && match(tab.view.webContents));
  }
}
