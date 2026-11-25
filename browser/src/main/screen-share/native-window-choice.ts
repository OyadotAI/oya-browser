/** A native candidate is an exact window/process identity, resolved afresh after explicit confirmation. */
import { randomUUID } from 'node:crypto';
import type { DesktopCapturerSource } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { ShareOption } from '../../shared/share-picker.ts';
import { withinTime } from '../../shared/within-time.ts';
import { PICKER, WINDOW_SOURCES, WINDOW_LABEL_LIMIT } from './constants.ts';
import { previewPNG } from './tab-previews.ts';
import type { NativeWindowIdentity, NativeWindowSelector } from './native-window-selector.ts';

/** A single offered capability retains exact native ownership only in main. */
interface WindowCandidate {
  /** Process/window snapshot from the current native picker. */
  identity: NativeWindowIdentity;
  /** Literal UI presentation and unguessable request token. */
  option: ShareOption;
}
/** A native enumeration acknowledged after explicit confirmation. */
interface ResolvedWindow {
  /** Candidate token, scoped to this request. */
  id: string;
  /** The exact native object returned by the fresh enumeration. */
  source: DesktopCapturerSource;
}

/** Electron documents the first window source component as CGWindowID on macOS. */
export function nativeWindowId(id: string): number | null {
  const match = /^window:([1-9]\d*):\d+$/.exec(id);
  const value = match ? Number(match[1]) : NaN;
  return Number.isSafeInteger(value) ? value : null;
}

/** A title is bounded literal context, never authority for matching a source. */
function candidateOption(source: DesktopCapturerSource): ShareOption {
  return {
    id: randomUUID(),
    kind: 'window',
    detail: 'Application window',
    name: source.name.replace(/\p{Cc}/gu, ' ').slice(0, WINDOW_LABEL_LIMIT) || 'Application window',
    thumbnail: previewPNG(source.thumbnail),
  };
}

/** One request can own at most one direct-selection candidate; retiring it invalidates every old UI token. */
export class NativeWindowChoice {
  /** Enumeration is native and never supplied by the picker renderer. */
  private readonly electron: AppServices['electron'];
  /** Native process lifetime checks reject reopened apps and vanished windows. */
  private readonly selector: NativeWindowSelector;
  /** Candidate metadata remains main-process-only. */
  private choice: WindowCandidate | null = null;
  /** A late enumeration cannot recreate a retired candidate. */
  private generation = 0;
  /** Only a fresh lookup after explicit confirmation may supply the final native handle. */
  private resolved: ResolvedWindow | null = null;
  /** Every async stage rechecks the requesting document and human ownership. */
  private readonly current: () => boolean;
  /** Bind resolution to one exact capture request. */
  constructor(electron: AppServices['electron'], selector: NativeWindowSelector, current: () => boolean) {
    this.electron = electron;
    this.selector = selector;
    this.current = current;
  }
  /** A selected window must still exist before its title/preview is shown for final consent. */
  async offer(identity: NativeWindowIdentity): Promise<ShareOption | null> {
    this.retire();
    const generation = this.generation;
    const source = await this.lookup(identity);
    if (!source || generation !== this.generation) return null;
    const option = candidateOption(source);
    this.choice = { identity, option };
    return option;
  }
  /** Only the currently offered capability can request a fresh native source after confirmation. */
  async resolve(id: string | null): Promise<DesktopCapturerSource | null> {
    const choice = this.choice;
    if (!choice || choice.option.id !== id) return null;
    const source = await this.lookup(choice.identity);
    if (!source || this.choice !== choice) return null;
    this.resolved = { id: choice.option.id, source };
    return source;
  }
  /** The original Electron callback gets the fresh source only while both native and request identities remain valid. */
  take(id: string | null): DesktopCapturerSource | null {
    const choice = this.choice,
      resolved = this.resolved;
    this.resolved = null;
    return choice && resolved?.id === id && choice.option.id === id && this.valid(choice.identity)
      ? resolved.source
      : null;
  }
  /** Before and after native enumeration, require the same window and process lifetime. */
  private async lookup(identity: NativeWindowIdentity): Promise<DesktopCapturerSource | null> {
    if (!this.valid(identity)) return null;
    try {
      const sources = await this.enumerate();
      const matches = sources.filter((source) => nativeWindowId(source.id) === identity.windowId);
      return this.valid(identity) && matches.length === 1 ? matches[0] : null;
    } catch {
      return null;
    }
  }
  /** Enumeration has a bounded deadline and returns native objects without synthesizing source ids. */
  private enumerate(): Promise<DesktopCapturerSource[]> {
    return withinTime(this.electron.desktopCapturer.getSources(WINDOW_SOURCES), PICKER.SOURCES_TIMEOUT_MS);
  }
  /** A current source never overrides an expired requester or a revoked OS grant. */
  private valid(identity: NativeWindowIdentity): boolean {
    const status = this.electron.systemPreferences.getMediaAccessStatus('screen');
    return this.current() && status !== 'denied' && status !== 'restricted' && this.selector.current(identity);
  }
  /** Starting over or using the list makes a previous confirmation permanently unusable. */
  retire(): void {
    this.generation++;
    this.choice = null;
    this.resolved = null;
  }
}
