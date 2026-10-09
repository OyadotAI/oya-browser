/**
 * Dragging a tab along the strip to reorder it, as in Chrome. A press shows
 * the tab at once; moving a few pixels lifts it, it follows the pointer, its
 * neighbours slide out of its way, and the strip scrolls when the pointer
 * nears an edge (once a frame). Letting go answers where it landed; the strip
 * reorders and tells the main process. Escape puts it back. The view measures
 * the strip at the press and hands it over as a `StripPort`.
 */
import { outsideStrip } from '../model/tear-off.ts';
import { ViewModel } from '../../../core/view-model.ts';
import { RendererConstants as C } from '../../../core/constants.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { RendererServices } from '../../../app/services.ts';
import { clamp, dropIndex, edgeSpeed, landingLeft, shifts, type Slot } from '../model/tab-math.ts';
import type { TabCardViewModel } from './tab-card-view-model.ts';

/** The strip as a drag sees it, measured live by the view. */
export interface StripPort {
  /** How far the strip is scrolled. */
  scrollLeft(): number;
  /** The strip's left and right edges in the window. */
  edges(): Pick<DOMRectReadOnly, 'left' | 'right'> & Partial<Pick<DOMRectReadOnly, 'top' | 'bottom'>>;
  /** Scrolls the strip by `px`. */
  scrollBy(px: number): void;
}

/** A press on a tab, as the view measured it. */
export interface Press {
  /** The tab pressed. */
  id: number;
  /** The pointer that pressed it. */
  pointerId: number;
  /** The pointer's x in the window. */
  x: number;
  /** Vertical position detects a tear-off even without horizontal reordering. */
  y?: number;
  /** Where every open tab sits at the press, in order. */
  slots: Slot[];
  /** The pressed tab's index among them. */
  index: number;
}

/** A finished drag: the tab, where it lands, and how far off its new place it was let go (for the glide home). */
export interface Drop {
  /** Leaving the strip moves the live tab to another native window. */
  detached?: boolean;
  /** The tab moved. */
  id: number;
  /** Its new index among the open tabs. */
  to: number;
  /** Pixels from its new place to where it was shown when let go. */
  offset: number;
}

/** What the strip draws during a drag. */
export interface TabDragState {
  /** The pressed tab, while a press lasts. */
  id: number | null;
  /** The press has become a drag. */
  lifted: boolean;
  /** How far the dragged tab is moved. */
  dx: number;
  /** How far each open tab slides, by index. */
  shifts: number[];
}

/** What a drag uses. */
export interface TabDragDeps extends Pick<RendererServices, 'frames'> {
  /** Shows a pressed tab at once. */
  bridge: Pick<OyaBrowser, 'activateTab'> & Partial<Pick<OyaBrowser, 'beginTabDrag' | 'endTabDrag'>>;
  /** Hidden when a drag lifts. */
  card: Pick<TabCardViewModel, 'hide'>;
}

/** The state at rest. */
const REST: TabDragState = { id: null, lifted: false, dx: 0, shifts: [] };

/** A tab drag. */
export class TabDragViewModel extends ViewModel<TabDragState> {
  /** What it uses. */
  private readonly deps: TabDragDeps;
  /** The press under way, if any. */
  private press: Press | null = null;
  /** The strip, during a press. */
  private strip: StripPort | null = null;
  /** The pointer's latest x. */
  private x = 0;
  /** Latest vertical pointer position, for a drag out of the strip. */
  private y = 0;
  /** The strip's scroll at the press. */
  private startScroll = 0;
  /** Where the tab would land now. */
  private to = 0;
  /** The frame that scrolls the strip under a lifted tab. */
  private frame: number | null = null;

  /** At rest; a running frame is cancelled on dispose. */
  constructor(deps: TabDragDeps) {
    super(REST);
    this.deps = deps;
    this.own(() => this.end());
  }

  /** A primary-button press on a tab (not its close button): it shows now, and may become a drag. */
  start(press: Press, strip: StripPort): void {
    void this.deps.bridge.activateTab(press.id);
    Object.assign(this, { press, strip, x: press.x, y: press.y ?? 0 });
    this.to = press.index;
    this.startScroll = strip.scrollLeft();
    this.set({ ...REST, id: press.id });
  }

  /** The pointer moved: past the threshold the tab lifts, then follows. */
  move(pointerId: number, x: number, y?: number): void {
    if (!this.press || pointerId !== this.press.pointerId) return;
    this.x = x;
    this.y = y ?? this.y;
    if (!this.state.lifted && Math.hypot(x - this.press.x, this.y - (this.press.y ?? 0)) < C.TAB_DRAG_THRESHOLD) return;
    if (!this.state.lifted) this.lift();
    this.follow();
  }

  /** The button came up: a drag answers where the tab lands (null for a plain click, or no move). */
  release(pointerId: number): Drop | null {
    if (!this.press || pointerId !== this.press.pointerId) return null;
    const outside = this.state.lifted && this.outside();
    const drop = this.state.lifted && (outside || this.to !== this.press.index) ? this.drop(this.press) : null;
    if (drop && outside) drop.detached = true;
    this.end(Boolean(drop?.detached));
    return drop;
  }

  /** A small margin prevents an accidental tear-off while reordering near strip edges. */
  private outside(): boolean {
    return outsideStrip(this.x, this.y, this.strip?.edges());
  }

  /** Escape during a drag puts every tab back; answers whether it took the key. */
  cancel(): boolean {
    if (!this.state.lifted) return false;
    this.end();
    return true;
  }

  /** Drops the press without moving anything (the system took the pointer away). */
  end(transferring = false): void {
    if (!transferring) void this.deps.bridge.endTabDrag?.().catch(() => {});
    this.press = null;
    this.strip = null;
    this.stopFrames();
    this.set(REST);
  }

  /** The press becomes a drag: the card goes and the strip starts following the pointer near its edges. */
  private lift(): void {
    void this.deps.bridge.beginTabDrag?.(this.press!.id).catch(() => this.end());
    this.deps.card.hide();
    this.set({ lifted: true });
    this.frame = this.deps.frames.request(() => this.scroll());
  }

  /** Puts the dragged tab under the pointer and slides the others to make room where it would land. */
  private follow(): void {
    if (!this.press || !this.strip) return;
    const { slots, index, x } = this.press;
    const dx = clamp(slots, index, this.x - x + this.strip.scrollLeft() - this.startScroll);
    this.to = dropIndex(slots, index, dx);
    this.set({ dx, shifts: shifts(slots, index, this.to) });
  }

  /** Each frame of a drag: scrolls the strip near its edge, and keeps the tab under the pointer. */
  private scroll(): void {
    if (!this.strip) return;
    const { left, right } = this.strip.edges();
    const speed = edgeSpeed(this.x, left, right);
    if (speed) this.strip.scrollBy(speed);
    if (speed) this.follow();
    this.frame = this.deps.frames.request(() => this.scroll());
  }

  /** Where the tab lands and how far it was let go from there. */
  private drop({ id, slots, index }: Press): Drop {
    const shown = slots[index].left + this.state.dx;
    return { id, to: this.to, offset: shown - landingLeft(slots, index, this.to) };
  }

  /** Stops scrolling the strip. */
  private stopFrames(): void {
    if (this.frame !== null) this.deps.frames.cancel(this.frame);
    this.frame = null;
  }
}

/** How far the open tab at `index` (tab `id`) slides during a drag; undefined at rest. */
export function slideFor(state: TabDragState, id: number, index: number): number | undefined {
  if (!state.lifted) return undefined;
  return state.id === id ? state.dx : (state.shifts[index] ?? 0);
}
