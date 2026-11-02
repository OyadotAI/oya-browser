/** Browser-owned capture picker facade: enumerate native previews, ask the person, then resolve one exact source. */
import type { WebContents, Streams } from 'electron';
import type { AppServices } from '../app/services.ts';
import { ShareSources } from './sources.ts';
import { ShareSheet } from './window.ts';
import { PICKER, DESKTOP_SOURCES } from './constants.ts';
import { withinTime } from '../../shared/within-time.ts';
import { NativeWindowSelector } from './native-window-selector.ts';
import { NativeWindowChoice } from './native-window-choice.ts';
import type { ShareOption, SharePresentation } from '../../shared/share-picker.ts';
import { ShareRequestLease, type ShareLeaseDeps } from './request-lease.ts';

/** Fixed macOS recovery destination, never supplied by a website. */
export const SCREEN_RECORDING_SETTINGS_URL = PICKER.SETTINGS_URL;

export { ShareIndicator } from './indicator.ts';

/** Native capture, tab policy and private UI services. */
type Deps = Pick<AppServices, 'appDir' | 'tabs'> & ShareLeaseDeps;
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
  private pending: ShareRequestLease | null = null;
  /** The native adapter owns process-wide cancellation quarantine across consecutive requests. */
  private readonly selector: NativeWindowSelector;

  /** Native APIs and tab ownership remain main-process-only. */
  constructor(deps: Deps) {
    this.deps = deps;
    this.selector = new NativeWindowSelector(deps.appDir);
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
    const lease = (this.pending = new ShareRequestLease(this.deps, page, current));
    const catalog = new ShareSources(this.deps, page);
    return this.choose(catalog, { page, origin, current: lease.current }).finally(() => this.release(catalog));
  }
  /** Agent and global control transitions revoke pending selection synchronously. */
  sync(): void {
    this.pending?.sync();
  }

  /** Release request capabilities and allow another picker, including after UI failures. */
  private release(catalog: ShareSources): void {
    catalog.dispose();
    this.pending?.dispose();
    this.pending = null;
  }

  /** Resolve opaque tokens only after rechecking the requester and selected native source. */
  private async choose(catalog: ShareSources, request: PickRequest): Promise<Streams | null> {
    const sources = await catalog.read();
    const { page, origin, current } = request;
    if (!current()) return null;
    const choice = new NativeWindowChoice(this.deps.electron, this.selector, current);
    const deps = { ...this.deps, native: { selector: this.selector, choice } };
    const sheet = new ShareSheet(deps, page, this.presentation(catalog, origin, sources), current);
    const selected = await sheet.result;
    return this.resolve(catalog, choice, selected, current);
  }
  /** Retire every candidate after the final synchronous request/source recheck. */
  private resolve(
    catalog: ShareSources,
    choice: NativeWindowChoice,
    selected: string | null,
    current: () => boolean,
  ): Streams | null {
    const video = current() ? (choice.take(selected) ?? catalog.resolve(selected)) : null;
    choice.retire();
    return video ? { video } : null;
  }
  /** Renderer metadata describes capability without exposing a native identity or handle. */
  private presentation(catalog: ShareSources, origin: string, sources: ShareOption[]): SharePresentation {
    return {
      origin,
      sources,
      desktopError: catalog.desktopError,
      canOpenSettings: catalog.canOpenSettings,
      native: this.selector.state(),
    };
  }
}
