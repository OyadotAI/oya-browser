/**
 * The strip a torn-off tab is carried over, as in Firefox and Chrome: that strip
 * opens a gap where the tab would land and draws it there, and the carrying window
 * hides meanwhile, so the tab looks back on a strip before it is let go. Moving off
 * the strip closes the gap and shows the window again.
 */
import type { Point } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { Tab } from '../tabs/types.ts';
import type { IncomingTab } from '../../shared/ipc.ts';

/** The fully hidden and fully shown opacity of the carrying window. */
const HIDDEN = 0;
/** The carrying window seen again. */
const SHOWN = 1;

/** What the strip is told about the tab carried over it, with the pointer at `x`. */
const incoming = (tab: Tab, x: number): IncomingTab => ({
  id: tab.id,
  title: tab.title,
  favicon: tab.favicon ?? null,
  home: !!tab.home,
  x,
});

/** Which strip shows the gap, for which carrying window. */
export class StripHover {
  /** The window whose strip shows the gap, if any. */
  private over?: AppServices;
  /** The window carrying the tab, hidden while a strip shows it. */
  private carrier?: AppServices;
  /** The last pointer x sent, so a still pointer sends nothing. */
  private lastX = Number.NaN;

  /** One frame: `target`'s strip (or none) shows `tab` at the pointer, and `carrier` hides while it does. */
  show(carrier: AppServices, tab: Tab, target: AppServices | undefined, point: Point): void {
    if (target !== this.over) this.leave();
    if (!target) return;
    Object.assign(this, { carrier, over: target });
    carrier.shell.window?.setOpacity?.(HIDDEN);
    this.send(target, tab, Math.round(point.x - target.shell.window!.getContentBounds().x));
  }

  /** The tab landed on `target`: its strip places the tab in the gap itself; any other strip showing one closes it. */
  landed(target: AppServices): void {
    if (this.over !== target) return this.leave();
    Object.assign(this, { over: undefined, carrier: undefined, lastX: Number.NaN });
  }

  /** The gap closes and the carrying window shows again. */
  leave(): void {
    if (this.over?.shell.alive()) this.over.shell.send('tab-incoming', null);
    const win = this.carrier?.shell.window;
    if (win && !win.isDestroyed()) win.setOpacity?.(SHOWN);
    Object.assign(this, { over: undefined, carrier: undefined, lastX: Number.NaN });
  }

  /** Tells `target`'s strip where the tab is, when the pointer moved. */
  private send(target: AppServices, tab: Tab, x: number): void {
    if (x === this.lastX) return;
    this.lastX = x;
    target.shell.send('tab-incoming', incoming(tab, x));
  }
}
