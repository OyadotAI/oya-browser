/**
 * The main window's last position, size and maximized state, so a relaunch
 * opens it where the person left it rather than at the default size. Saved
 * when the window closes; read back as untrusted input and dropped when it is
 * malformed or no longer lands on a connected display, since a window
 * restored onto an unplugged monitor is invisible.
 */
import type { BrowserWindow, Rectangle } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { JsonFile } from '../app/json-file.ts';
import { WINDOW_SIZE, WINDOW_SIZE_MAX, WINDOW_VISIBLE_MIN } from './constants.ts';

/** What the file holds: the window's normal (un-maximized) bounds and whether it was maximized. */
export interface SavedWindow extends Rectangle {
  /** Whether the window was maximized when it closed. */
  maximized: boolean;
}

/** A whole number a window coordinate can be; anything else in the file is rejected. */
const windowCoordinate = (value: unknown): value is number => Number.isSafeInteger(value);

/** A positive size no larger than any real display. */
const windowSide = (value: unknown): value is number =>
  windowCoordinate(value) && value > 0 && value <= WINDOW_SIZE_MAX;

/** Whether the saved position and size are numbers a real window could have. */
function plausible(s: Partial<SavedWindow>): boolean {
  return windowCoordinate(s.x) && windowCoordinate(s.y) && windowSide(s.width) && windowSide(s.height);
}

/** Whether the file's value has the saved window's shape. */
function wellFormed(saved: unknown): saved is SavedWindow {
  const s = saved as Partial<SavedWindow> | null;
  return !!s && plausible(s) && typeof s.maximized === 'boolean';
}

/** How far two spans overlap along one axis. */
function overlap(start: number, length: number, areaStart: number, areaLength: number): number {
  return Math.min(start + length, areaStart + areaLength) - Math.max(start, areaStart);
}

/** Whether the top edge, where the title bar is dragged from, is on `area`: a window whose top is above the screen cannot be moved back. */
function topOn(bounds: Rectangle, area: Rectangle): boolean {
  return bounds.y >= area.y && bounds.y <= area.y + area.height - WINDOW_VISIBLE_MIN;
}

/** Whether enough of `bounds` is on `area` to be seen and dragged: a sliver of overlap is still unreachable. */
function onDisplay(bounds: Rectangle, area: Rectangle): boolean {
  const across = overlap(bounds.x, bounds.width, area.x, area.width);
  return across >= WINDOW_VISIBLE_MIN && topOn(bounds, area);
}

/** Where to open the window from what was saved, or null for the default: malformed, or on no display connected now. Sizes below the minimum are raised to it. */
export function placement(saved: unknown, areas: Rectangle[]): SavedWindow | null {
  if (!wellFormed(saved)) return null;
  const width = Math.max(saved.width, WINDOW_SIZE.minWidth);
  const height = Math.max(saved.height, WINDOW_SIZE.minHeight);
  const bounds = { x: saved.x, y: saved.y, width, height, maximized: saved.maximized };
  return areas.some((area) => onDisplay(bounds, area)) ? bounds : null;
}

/** The saved main-window bounds: read once at launch, written when the window closes. */
export class WindowBounds {
  /** Electron, for the displays connected now. */
  private readonly deps: Pick<AppServices, 'electron'>;
  /** Where the bounds are kept. */
  private readonly file: JsonFile<SavedWindow>;

  /** `deps` is the main-process services (see src/main/main.ts); `file` is the bounds' file. */
  constructor(deps: Pick<AppServices, 'electron'>, file: JsonFile<SavedWindow>) {
    this.deps = deps;
    this.file = file;
  }

  /** The bounds to open at, or null for the default. Displays are asked here, not at construction: the screen module needs a ready app. */
  saved(): SavedWindow | null {
    const areas = this.deps.electron.screen.getAllDisplays().map((display) => display.workArea);
    return placement(this.file.read(), areas);
  }

  /**
   * Saves `win`'s bounds when it closes. The normal bounds, so un-maximizing
   * returns to the person's size; never while fullscreen, which is not
   * restored, or minimized, where macOS reports a maximized window as not
   * maximized at the maximized size. The last good bounds are kept instead.
   */
  track(win: BrowserWindow): void {
    win.on('close', () => {
      if (win.isFullScreen() || win.isMinimized()) return;
      this.file.write({ ...win.getNormalBounds(), maximized: win.isMaximized() });
    });
  }
}
