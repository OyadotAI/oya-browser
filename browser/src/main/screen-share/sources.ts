/** Request-scoped capture sources; opaque UI tokens resolve only to browser-owned native handles. */
import { randomUUID } from 'node:crypto';
import type {
  DesktopCapturerSource,
  WebContents,
  WebFrameMain,
  WebContentsDidStartNavigationEventParams,
} from 'electron';
import type { AppServices } from '../app/services.ts';
import type { Tab } from '../tabs/types.ts';
import { reopenable } from '../tabs/tab-order.ts';
import type { ShareOption } from '../../shared/share-picker.ts';
import { withinTime } from '../../shared/within-time.ts';
import { PICKER, DESKTOP_SOURCES } from './constants.ts';
import { TabPreviews, previewPNG } from './tab-previews.ts';

/** A native video source suitable for Electron's display-media callback. */
export type ShareVideo = DesktopCapturerSource | WebFrameMain;
/** Only same-profile, human-owned tabs and explicitly listed OS sources enter this catalog. */
type Deps = Pick<AppServices, 'electron' | 'tabs' | 'agentTab'>;
/** A private capability and its public preview. */
interface Choice {
  /** Safe presentation data. */
  option: ShareOption;
  /** Native frame or desktop source retained in the main process. */
  video: ShareVideo;
  /** Reject a tab that was closed, navigated, or claimed after its preview. */
  current: () => boolean;
}

/** A website's host, or null for anything else: never its credentials, path, query or a local file. */
export function webHost(address: string): string | null {
  const url = URL.parse(address);
  return url?.protocol === 'https:' || url?.protocol === 'http:' ? url.host : null;
}

/** Titles and origins provide context without exposing URL credentials, query strings or local paths. */
function tabDetail(tab: Tab): string {
  return webHost(tab.url) ?? 'Oya tab';
}

/** Literal tab metadata contains no capture capability. */
function tabOption(tab: Tab, thumbnail: string): ShareOption {
  return { id: randomUUID(), kind: 'tab', name: tab.title || tabDetail(tab), detail: tabDetail(tab), thumbnail };
}

/** A desktop source exposes its title and PNG, never its OS identifier. */
function desktopOption(video: DesktopCapturerSource): ShareOption {
  const kind = video.id.startsWith('screen:') ? 'screen' : 'window';
  const detail = kind === 'screen' ? 'Entire screen' : 'Application window';
  return { id: randomUUID(), kind, name: video.name, detail, thumbnail: previewPNG(video.thumbnail) };
}

/** One picker gets one catalog, disposed immediately after choice or cancellation. */
export class ShareSources {
  /** Browser APIs and tab ownership policy. */
  private readonly deps: Deps;
  /** Requesting page fixes the profile whose tabs may appear. */
  private readonly page: WebContents;
  /** Request-scoped rendering for tabs that have never been shown. */
  private readonly previews: TabPreviews;
  /** Choice tokens never survive this request. */
  private readonly choices = new Map<string, Choice>();
  /** Navigation listeners released on every exit path. */
  private readonly cleanups: (() => void)[] = [];
  /** A desktop enumeration failure must not hide available browser tabs. */
  desktopError = '';
  /** macOS recovery is offered only when desktop enumeration failed. */
  get canOpenSettings(): boolean {
    return this.platform === 'darwin' && !!this.desktopError;
  }
  /** Injectable platform keeps permission regressions independent of the test host. */
  private readonly platform: string;

  /** Snapshot capabilities belong to the requesting page's exact Electron session. */
  constructor(deps: Deps, page: WebContents, platform = process.platform) {
    this.deps = deps;
    this.page = page;
    this.previews = new TabPreviews(deps);
    this.platform = platform;
  }

  /** Read previews in parallel, preserving tab order before desktop sources. */
  async read(): Promise<ShareOption[]> {
    const tabs = this.deps.tabs.list.filter((tab) => this.eligible(tab)).slice(0, PICKER.TAB_LIMIT);
    const [tabChoices, desktop] = await this.readChoices(tabs);
    for (const choice of [...tabChoices, ...desktop]) if (choice?.current()) this.choices.set(choice.option.id, choice);
    return [...this.choices.values()].map((choice) => choice.option);
  }

  /** Always release the invisible preview surface, including when one native operation fails. */
  private async readChoices(tabs: Tab[]): Promise<[(Choice | null)[], Choice[]]> {
    try {
      return await Promise.all([Promise.all(tabs.map((tab) => this.tab(tab))), this.desktop()]);
    } finally {
      this.previews.dispose();
    }
  }

  /** A renderer cannot name arbitrary windows, other profiles, agent tabs or private sessions. */
  private eligible(tab: Tab): boolean {
    const contents = tab.view.webContents;
    return (
      reopenable(tab) &&
      !contents.isDestroyed() &&
      contents.session === this.page.session &&
      this.deps.agentTab.humanMay(tab.id)
    );
  }

  /** Capture one tab's preview without activating it or changing its page. */
  private async tab(tab: Tab): Promise<Choice | null> {
    const contents = tab.view.webContents;
    const frame = contents.mainFrame;
    const current = this.watch(tab);
    const option = tabOption(tab, await this.previews.capture(tab));
    return current() ? { option, video: frame, current } : null;
  }

  /** A replaced document invalidates its preview even when the new document uses the same URL. */
  private watch(tab: Tab): () => boolean {
    let navigated = false;
    const contents = tab.view.webContents;
    const revoke = (event: WebContentsDidStartNavigationEventParams): void => {
      if (event.isMainFrame && !event.isSameDocument) navigated = true;
    };
    contents.on('did-start-navigation', revoke);
    this.cleanups.push(() => contents.removeListener('did-start-navigation', revoke));
    return () => !navigated && this.deps.tabs.list.includes(tab) && this.eligible(tab);
  }

  /** Native OS previews include both windows and displays; no source is selected automatically. */
  private async desktop(): Promise<Choice[]> {
    try {
      const pending = this.deps.electron.desktopCapturer.getSources(DESKTOP_SOURCES);
      const sources = await withinTime(pending, PICKER.SOURCES_TIMEOUT_MS);
      return this.desktopChoices(sources);
    } catch {
      this.desktopError = PICKER.DESKTOP_ERROR;
      return [];
    }
  }

  /** A partial OS list under denied access is not a usable capture choice. */
  private desktopChoices(sources: DesktopCapturerSource[]): Choice[] {
    if (!this.desktopAllowed()) throw Error('Screen Recording is blocked');
    if (!sources.length) this.desktopError = PICKER.DESKTOP_ERROR;
    return sources.filter((video) => video.id !== this.previews.sourceId).map((video) => this.desktopChoice(video));
  }

  /** Check after enumeration: checking before it can prevent macOS from ever asking for access. */
  private desktopAllowed(): boolean {
    if (this.platform !== 'darwin') return true;
    const status = this.deps.electron.systemPreferences.getMediaAccessStatus('screen');
    return status !== 'denied' && status !== 'restricted';
  }

  /** Native identifiers stay private; only a random token and PNG are exposed to the picker. */
  private desktopChoice(video: DesktopCapturerSource): Choice {
    return { option: desktopOption(video), video, current: () => this.desktopAllowed() };
  }

  /** Resolve only a still-valid capability from this exact catalog. */
  resolve(id: string | null): ShareVideo | null {
    const choice = id && this.choices.get(id);
    return choice && choice.current() ? choice.video : null;
  }

  /** Release all references and listeners after the sheet closes. */
  dispose(): void {
    for (const cleanup of this.cleanups) cleanup();
    this.choices.clear();
    this.previews.dispose();
  }
}
