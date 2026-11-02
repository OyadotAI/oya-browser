/** Browser-owned capture picker facade: enumerate native previews, ask the person, then resolve one exact source. */
import type { WebContents, Streams } from 'electron';
import type { AppServices } from '../app/services.ts';
import { ShareSources } from './sources.ts';
import { ShareSheet } from './window.ts';
import { PICKER, DESKTOP_SOURCES } from './constants.ts';
import { withinTime } from '../../shared/within-time.ts';

/** Fixed macOS recovery destination, never supplied by a website. */
export const SCREEN_RECORDING_SETTINGS_URL = PICKER.SETTINGS_URL;

export { ShareIndicator } from './indicator.ts';

/** Native capture, tab policy and private UI services. */
type Deps = Pick<AppServices, 'electron' | 'appDir' | 'windows' | 'tabs' | 'agentTab'>;
/** Identity and guard of a pending share. */
interface PickRequest {
  /** Exact requesting tab. */
  page: WebContents;
  /** Recipient displayed in the picker. */
  origin: string;
  /** Requester's document and ownership remain unchanged. */
  current: () => boolean;
}

/** One browser-owned picker at a time; every share asks for an explicit source. */
export class ScreenPicker {
  /** Shared services wired in the composition root. */
  private readonly deps: Deps;
  /** Concurrent site requests cannot stack modal source pickers. */
  private pending = false;

  /** Native APIs and tab ownership remain main-process-only. */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** A first native enumeration lets macOS register the application and ask for Screen Recording access. */
  async requestOSPermission(): Promise<void> {
    await withinTime(this.deps.electron.desktopCapturer.getSources(DESKTOP_SOURCES), PICKER.SOURCES_TIMEOUT_MS).catch(
      () => undefined,
    );
  }

  /** Ask for a source without selecting the first display or remembering an old choice. */
  pick(page: WebContents, origin: string, current: () => boolean): Promise<Streams | null> {
    if (this.pending || !current()) return Promise.resolve(null);
    this.pending = true;
    const catalog = new ShareSources(this.deps, page);
    return this.choose(catalog, { page, origin, current }).finally(() => this.release(catalog));
  }

  /** Release request capabilities and allow another picker, including after UI failures. */
  private release(catalog: ShareSources): void {
    catalog.dispose();
    this.pending = false;
  }

  /** Resolve opaque tokens only after rechecking the requester and selected native source. */
  private async choose(catalog: ShareSources, { page, origin, current }: PickRequest): Promise<Streams | null> {
    const sources = await catalog.read();
    if (!current()) return null;
    const data = { origin, sources, desktopError: catalog.desktopError, canOpenSettings: catalog.canOpenSettings };
    const sheet = new ShareSheet(this.deps, page, data, current);
    const selected = await sheet.result;
    const video = current() ? catalog.resolve(selected) : null;
    return video ? { video } : null;
  }
}
