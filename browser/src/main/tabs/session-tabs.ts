/**
 * The tabs a person had open, brought back at the next sign-in. Written when
 * the strip settles and before a window or the app closes; read once the
 * persona's cookies are in the jar (never at boot: a page loaded before them
 * mints a logged-out cookie that replaces the login the server held, which is
 * the bug the start page exists to prevent). Only the same profile gets its
 * tabs back, and never a governed browser.
 */
import type { AppServices } from '../app/services.ts';
import type { JsonFile } from '../app/json-file.ts';
import { pinnedCount, reopenable } from './tab-order.ts';
import type { Tab } from './types.ts';
import { RESTORED_TABS_MAX, SESSION_SAVE_DELAY_MS, WEB_URL } from './constants.ts';

/** Only web addresses are kept or restored: the file is an input, and a file: or javascript: entry must never become a tab. */
const webAddress = (url: unknown): url is string => typeof url === 'string' && WEB_URL.test(url);

/** `tabs` with the pinned ones first, each group in its own order. */
function pinnedFirst(tabs: Tab[]): Tab[] {
  return [...tabs.filter((tab) => tab.pinned), ...tabs.filter((tab) => !tab.pinned)];
}

/** How many restored tabs come back pinned: the file is an input, so anything but a whole positive count is none, and never more than were restored. */
function pinnedOf(saved: SavedSession, restored: number): number {
  const pinned = saved.pinned;
  return typeof pinned === 'number' && Number.isInteger(pinned) && pinned > 0 ? Math.min(pinned, restored) : 0;
}

/** What the file holds: whose tabs, their addresses in strip order, and which was shown. */
export interface SavedSession {
  /** The profile's partition, so another profile never gets these. */
  partition: string;
  /** The addresses, in strip order. */
  urls: string[];
  /** The index of the tab that was shown. */
  active: number;
  /** How many of the leading addresses were pinned; absent in files written before pinning, read as none. */
  pinned?: number;
}

/** The tabs, the profile, the window's mode and the governance rule. */
type Deps = Pick<AppServices, 'tabs' | 'persona' | 'shell' | 'governance'>;

/** The last session's tabs: saved as the strip changes, restored once per launch. */
export class SessionTabs {
  /** Services. */
  private readonly deps: Deps;
  /** Where the session is kept. */
  private readonly file: JsonFile<SavedSession>;
  /** The save waiting for the strip to settle. */
  private timer?: ReturnType<typeof setTimeout>;
  /** Set once this launch restored (or found nothing to), so a reconnect never duplicates tabs. */
  private restored = false;

  /** `deps` is the main-process services (see src/main/main.ts); `file` is the session's file. */
  constructor(deps: Deps, file: JsonFile<SavedSession>) {
    this.deps = deps;
    this.file = file;
  }

  /** The strip changed: written once it settles, so a burst of closes writes once and a crash loses little. */
  changed(): void {
    clearTimeout(this.timer);
    this.timer = setTimeout(() => this.flush(), SESSION_SAVE_DELAY_MS);
    this.timer.unref?.();
  }

  /**
   * Writes the open tabs now. A strip with no tabs at all is never written: a
   * person cannot reach zero (closing the last opens the start page), so an
   * empty list is always a bulk close (log out, profile switch, window close)
   * and never a session. The startup page must not erase saved tabs before
   * restoration; after restoration or a real page, a start page alone is empty.
   */
  flush(): void {
    clearTimeout(this.timer);
    this.timer = undefined;
    if (!this.deps.tabs.list.length) return;
    const saved = this.snapshot();
    if (!this.restored && !saved.urls.length) return;
    this.restored = true;
    this.file.write(saved);
  }

  /**
   * The tabs worth bringing back, pinned ones first, and which was shown. The
   * list spans every window, so a pinned block is only leading per window;
   * gathering the pins first, and counting them among the tabs actually kept,
   * is what lets every pin come back.
   */
  private snapshot(): SavedSession {
    const kept = pinnedFirst(this.deps.tabs.list.filter((tab) => reopenable(tab) && webAddress(tab.url)));
    const active = kept.findIndex((tab) => tab.id === this.deps.tabs.activeTabId);
    return {
      partition: this.deps.persona.partitionName(),
      urls: kept.map((tab) => tab.url),
      active: Math.max(active, 0),
      pinned: pinnedCount(kept),
    };
  }

  /** Opens the saved tabs once per launch, into a window showing only the start page, for the same profile; otherwise nothing. */
  restore(): void {
    if (this.restored || !this.restorable()) return;
    this.restored = true;
    const saved = this.file.read();
    if (saved?.partition !== this.deps.persona.partitionName() || !Array.isArray(saved.urls)) return;
    const urls = saved.urls.filter(webAddress).slice(0, RESTORED_TABS_MAX);
    this.open(urls, saved.active, pinnedOf(saved, urls.length));
  }

  /** Not a governed browser, in browsing mode, and no page open yet (a switch reopened its own, or the person already typed one). */
  private restorable(): boolean {
    const { tabs, shell, governance } = this.deps;
    return !governance.configuration && !!shell.browsingMode && !tabs.list.some((tab) => reopenable(tab));
  }

  /** Opens the pages in order, shows the one that was shown, closes the start page they replace, and pins the first `pinned`. */
  private open(urls: string[], active: number, pinned: number): void {
    if (!urls.length) return;
    const starts = this.deps.tabs.list.map((tab) => tab.id);
    const ids = urls.map((url) => this.deps.tabs.createTab(url, false));
    this.deps.tabs.activateTab(ids[Number.isInteger(active) && active >= 0 && active < ids.length ? active : 0]);
    for (const id of starts) this.deps.tabs.closeTab(id);
    this.pin(ids.slice(0, pinned));
  }

  /**
   * Flags the restored tabs that were pinned. They are flagged, not moved:
   * they already lead the strip once the start page closed, and the root
   * strip's list is a copy that a reorder would not reach.
   */
  private pin(ids: number[]): void {
    if (!ids.length) return;
    for (const id of ids) {
      const tab = this.deps.tabs.find(id);
      if (tab) tab.pinned = true;
    }
    this.deps.tabs.sendTabList();
  }
}
