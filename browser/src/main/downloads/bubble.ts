/**
 * The toolbar's download chip, fed from main: each window hears only the
 * downloads that started from its own pages, in the profile on screen, so a
 * person sees "Saved" in the window they clicked in and nowhere else. It knows
 * only downloads it saw start in this run, so nothing kept from an earlier
 * launch ever shows, and it never sends where a file was saved.
 */
import type { WebContents } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { DownloadEntry } from '../../shared/library.ts';
import type { DownloadBubbleState } from '../../shared/ipc.ts';

/** The services the chip reads: the windows, the download list and the profile on screen. */
type Deps = Pick<AppServices, 'windows' | 'downloads'>;

/** Which downloads each window shows, and tells it when they change. */
export class DownloadBubble {
  /** The services the chip reads. */
  private readonly deps: Deps;
  /** Each window's downloads by id: those still running, and its latest. */
  private readonly shown = new Map<AppServices, Set<string>>();
  /** Windows already re-told on focus changes, so a window forgotten and shown again is not watched twice. */
  private readonly watched = new WeakSet<object>();

  /** `deps` gives the windows and the download list. */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** A download was listed from `contents`: the window that page is in shows it from now on. */
  began(id: string, contents: WebContents | undefined): void {
    const scope = this.windowOf(contents);
    this.watchFocus(scope);
    this.shown.set(scope, (this.shown.get(scope) ?? new Set()).add(id));
    this.tell(scope);
  }

  /** The list changed (progress, an end, a profile switch): tells every window that shows a download. */
  announce(): void {
    for (const scope of [...this.shown.keys()]) this.tell(scope);
  }

  /** The window a page is in; a page in no tab (a popup window) counts as the window in front. */
  private windowOf(contents: WebContents | undefined): AppServices {
    const windows = this.deps.windows;
    const owner = contents && windows?.ownerOfContents(contents);
    return owner || windows?.current || (this.deps as AppServices);
  }

  /** Re-tells a window when it gains or loses focus, while it shows a download: a finished chip waits for the person to look. */
  private watchFocus(scope: AppServices): void {
    const win = scope.shell.window;
    if (!win || this.watched.has(win)) return;
    this.watched.add(win);
    const retell = (): void => void (this.shown.has(scope) && this.tell(scope));
    win.on('focus', retell);
    win.on('blur', retell);
  }

  /** Sends `scope` its chip, and forgets it once it shows nothing (a closed window, another profile's downloads). */
  private tell(scope: AppServices): void {
    const ids = this.shown.get(scope);
    const mine = this.deps.downloads.list().filter((d) => ids?.has(d.id));
    const alive = scope.shell.alive?.() ?? !!scope.shell.window;
    if (alive) scope.shell.send('download-bubble', stateOf(mine, !!scope.shell.window?.isFocused()));
    if (!alive || !mine.length) return void this.shown.delete(scope);
    this.shown.set(scope, new Set(mine.filter((d, i) => i === 0 || d.state === 'progressing').map((d) => d.id)));
  }
}

/** What a window's chip shows: its latest download, and the totals of those still running. */
export function stateOf(mine: DownloadEntry[], focused: boolean): DownloadBubbleState {
  const running = mine.filter((d) => d.state === 'progressing');
  const unknown = running.some((d) => !d.total);
  const sum = (key: 'received' | 'total'): number => running.reduce((n, d) => n + d[key], 0);
  const total = unknown ? null : sum('total');
  return { latest: mine[0] ?? null, running: running.length, received: sum('received'), total, focused };
}
