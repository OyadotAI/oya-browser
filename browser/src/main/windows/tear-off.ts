/**
 * A tab pulled off its strip, the way Chrome and Firefox do it: the tab goes into
 * a real window at once (a window with no other tab simply is that window), the
 * window follows the pointer until the button comes up, and then it joins the
 * strip it was let go over, or stays where it is. A card standing in for the tab
 * left the person waiting for the real window until they let go.
 */
import type { Point } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { Tab } from '../tabs/types.ts';
import type { BrowserWindows } from './windows.ts';
import { StripHover } from './strip-hover.ts';
import { DRAG_FRAME_MS, DRAG_MAX_MS } from './constants.ts';

/** One gesture: the window carrying the tab, and where on it the pointer holds it. */
interface Carry {
  /** The window that follows the pointer. */
  scope: AppServices;
  /** The pointer's offset from the window's top-left corner. */
  grip: Point;
}

/** The tab being torn off, and the window that carries it. */
export class TabTearOff {
  /** Window ownership, transfer and drop targets. */
  private readonly windows: BrowserWindows;
  /** The tab being dragged; 0 when no gesture is under way. */
  private id = 0;
  /** Resolves to the carrying window once it exists (a new window paints first). */
  private carry: Promise<Carry | undefined> = Promise.resolve(undefined);
  /** The pointer-following timer. */
  private timer?: ReturnType<typeof setInterval>;
  /** A lost release cannot leave a window glued to the pointer. */
  private deadline = 0;
  /** The strip the tab is carried over, which shows it in a gap. */
  private readonly hover = new StripHover();

  /** `windows` owns the windows the tab moves between. */
  constructor(windows: BrowserWindows) {
    this.windows = windows;
  }

  /** The tab left `source`'s strip: it gets its own window, which follows the pointer from now on. */
  begin(source: AppServices, id: number): Promise<void> {
    source.agentTab.requireFor(id);
    const tab = source.tabs.find(id);
    if (!tab || tab.window) throw new Error('Tab is not in this window');
    this.end();
    Object.assign(this, { id, deadline: Date.now() + DRAG_MAX_MS });
    this.carry = this.carrier(source, id).then((scope) => (scope && this.id === id ? this.follow(scope) : undefined));
    return this.carry.then(() => {});
  }

  /** Whether tab `id` is being torn off right now. */
  carrying(id: number): boolean {
    return this.id === id;
  }

  /** The button came up: the window joins the strip under the pointer, else stays where it was let go. */
  async drop(): Promise<void> {
    const id = this.id;
    const carry = await this.carry;
    this.stop();
    const tab = carry?.scope.tabs.find(id);
    const target = carry && tab && this.windows.dropTarget(carry.scope, this.cursor(carry.scope));
    if (!target) return this.hover.leave();
    this.land(carry.scope, target, tab);
  }

  /** Moves `tab` onto `target`'s strip, whose gap takes it; a refused move closes the gap rather than leaving it open. */
  private land(source: AppServices, target: AppServices, tab: Tab): void {
    try {
      this.windows.transfer(source, target, tab);
    } catch (error) {
      this.hover.leave();
      throw error;
    }
    this.hover.landed(target);
  }

  /** Stops following the pointer; the window stays where it is, shown, and no strip keeps a gap. */
  end(): void {
    this.stop();
    this.hover.leave();
  }

  /** Stops following the pointer and forgets the gesture. */
  private stop(): void {
    clearInterval(this.timer);
    Object.assign(this, { timer: undefined, id: 0, carry: Promise.resolve(undefined) });
  }

  /** The window that carries the tab: the source itself when the tab is all it has, else a new one under the pointer. */
  private carrier(source: AppServices, id: number): Promise<AppServices | undefined> {
    if (source.tabs.list.length === 1) return Promise.resolve(source);
    return this.windows.tearOff(id, this.cursor(source));
  }

  /** Starts moving `scope`'s window with the pointer, keeping the grip it has now. */
  private follow(scope: AppServices): Carry {
    const at = scope.shell.window!.getBounds();
    const point = this.cursor(scope);
    const carry = { scope, grip: { x: point.x - at.x, y: point.y - at.y } };
    this.timer = setInterval(() => this.track(carry), DRAG_FRAME_MS);
    return carry;
  }

  /** One frame: the window moves under the pointer, until its tab is no longer the person's or the gesture ran too long. */
  private track({ scope, grip }: Carry): void {
    const win = scope.shell.window;
    if (!scope.shell.alive() || !win || !scope.agentTab.humanMay(this.id) || Date.now() > this.deadline)
      return this.end();
    const point = this.cursor(scope);
    win.setPosition(Math.round(point.x - grip.x), Math.round(point.y - grip.y), false);
    const tab = scope.tabs.find(this.id);
    if (tab) this.hover.show(scope, tab, this.windows.dropTarget(scope, point), point);
  }

  /** The pointer on screen. */
  private cursor(scope: AppServices): Point {
    return scope.electron.screen.getCursorScreenPoint();
  }
}
