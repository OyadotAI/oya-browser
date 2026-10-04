/**
 * The front door's view of the browser: which targets harnesses may see, and
 * how a tab is opened for one. Every tab goes through the app's createTab, so it
 * takes the protected path (fingerprint, proxy, persona partition).
 */
import { whenProtected } from '../tabs/load.ts';
import { FRONT_DOOR_TAB_WAIT_MS } from './constants.ts';
import type { DoorTab, Finish, FrontDoorOptions, TargetInfo } from './types.ts';

/** The app hooks a door always has: the optional ones filled with do-nothing defaults. */
type DoorOptions = FrontDoorOptions & Required<Pick<FrontDoorOptions, 'beginCommand' | 'clientChanged'>>;

/** The browser's own UI pages (the shell, out/renderer/index.html, and the input shield, out/renderer/control-shield/index.html): never an agent target. */
export const isUi = (info: TargetInfo | null | undefined): boolean =>
  info?.type === 'page' && /^file:.*\/renderer\/(?:control-shield\/)?index\.html(?:[?#]|$)/.test(info.url || '');

/** Waits for the tab's first load, but no longer than FRONT_DOOR_TAB_WAIT_MS: one that never settles is not waited out. */
async function untilFirstLoad(tab: DoorTab): Promise<void> {
  let timer: NodeJS.Timeout | undefined;
  const waited = new Promise((r) => (timer = setTimeout(r, FRONT_DOOR_TAB_WAIT_MS)));
  await Promise.race([tab.ready?.catch(() => {}), waited]);
  clearTimeout(timer);
}

/** What Target.getTargetInfo answers. */
interface TargetInfoReply {
  /** The tab's own target. */
  targetInfo: Required<Pick<TargetInfo, 'targetId'>>;
}

/** The targets behind one front door and the app hooks that open, close and admit them. */
export class FrontDoor {
  /** Target ids a harness must never see. */
  readonly hidden = new Set<string>();
  /** Iframes and workers of a validation run's tabs: part of the run, though opened after it started. */
  readonly runChildren = new Set<string>();
  /** The options `start` was given, with the optional hooks filled in. */
  readonly options: DoorOptions;
  /** Chromium's own debug endpoint, loopback only. */
  readonly up: string;

  /** Takes the options `start` was given; the admission hooks default to letting everything through. */
  constructor(options: FrontDoorOptions) {
    const beginCommand = options.beginCommand ?? ((): Finish => () => {});
    this.options = { ...options, beginCommand, clientChanged: options.clientChanged ?? (() => {}) };
    this.up = `127.0.0.1:${options.upstream}`;
  }

  /** Whether a target belongs to the validation run (always, outside a run): one of its tabs, or their frames and workers. */
  inRun(id: string): boolean {
    const { allowedTarget } = this.options;
    return !allowedTarget || allowedTarget(id) || this.runChildren.has(id);
  }

  /** Whether a target is off limits: the UI, or (in a validation run) outside the run. */
  blocked(info: TargetInfo, id: string): boolean {
    return isUi(info) || !this.inRun(id);
  }

  /** Whether a target seen over CDP (a TargetInfo) is hidden. */
  isHidden(info: TargetInfo): boolean {
    const id = info.targetId ?? '';
    return this.hidden.has(id) || this.blocked(info, id);
  }

  /** Chromium's target list, after marking the ones to hide. */
  async refreshHidden(): Promise<TargetInfo[]> {
    const list = (await (await fetch(`http://${this.up}/json/list`)).json()) as TargetInfo[];
    for (const t of list) if (this.blocked(t, t.id ?? '')) this.hidden.add(t.id ?? '');
    return list;
  }

  /** refreshHidden, as true when it worked and false when Chromium did not answer. */
  tryRefresh(): Promise<boolean> {
    return this.refreshHidden().then(
      () => true,
      () => false,
    );
  }

  /** A tab's targetId, asked of its own debugger (setupTabCDP attaches it). */
  async targetIdOf(tab: DoorTab): Promise<string> {
    if (!tab.targetId) {
      const reply = (await tab.view.webContents.debugger.sendCommand('Target.getTargetInfo')) as TargetInfoReply;
      tab.targetId = reply.targetInfo.targetId;
    }
    return tab.targetId;
  }

  /**
   * Opens a tab the protected way and returns its targetId; a first load that
   * never settles is not waited out. One that could not be protected is
   * closed and refused: a harness handed it, or one that found it in the
   * target list, would drive it straight to the site.
   */
  async openTab(url: string | undefined): Promise<string> {
    const id = this.options.createTab(url || 'about:blank', true);
    const tab = this.options.tabs().find((t) => t.id === id) as DoorTab;
    await whenProtected(tab).catch((err: unknown) => {
      this.options.closeTab(id, { keepOne: false });
      throw err;
    });
    await untilFirstLoad(tab);
    return this.targetIdOf(tab);
  }

  /** Runs `work` inside one admitted automation command. */
  async admitted<T>(work: () => Promise<T>): Promise<T> {
    const finish = await this.options.beginCommand();
    try {
      return await work();
    } finally {
      finish();
    }
  }
}
