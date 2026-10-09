/**
 * The workspace panel: whether it is open, the pane in view (and the Inspect
 * pane last shown), the layout the main process computed, and resizing by
 * drag or keyboard. Panes register what Clear does for them, so the panel
 * depends on none of them.
 */
import { ViewModel } from '../../core/view-model.ts';
import { RendererConstants as C } from '../../core/constants.ts';
import type { OyaBrowser } from '../../core/bridge.ts';
import type { ShellLayout } from '../../../shared/ipc.ts';
import { INSPECT_PANES, type Pane } from './constants.ts';

/** What the panel shows. */
export interface PanelState {
  /** The panel is open. */
  open: boolean;
  /** The pane in view. */
  pane: Pane;
  /** The Inspect pane last shown, which the Inspect tab reopens. */
  lastInspect: Pane;
  /** The layout from the main process, once one has come. */
  layout: ShellLayout | null;
  /** The panel's edge is being dragged. */
  dragging: boolean;
}

/** The parts of the bridge the panel uses. */
export type PanelBridge = Pick<OyaBrowser, 'toggleDevPanel' | 'resizeDevPanel' | 'onDevPanelState' | 'onShellLayout'>;

/** Schedules a callback for the next animation frame; returns a handle to cancel it. */
export interface FrameClock {
  /** Like requestAnimationFrame. */
  request(callback: () => void): number;
  /** Like cancelAnimationFrame. */
  cancel(handle: number): void;
}

/** The width for a pointer at `x` in a window `windowWidth` wide: within the panel's limits, leaving the page its minimum. */
export function widthAt(x: number, windowWidth: number): number {
  const width = Math.min(windowWidth - x, C.PANEL_MAX_WIDTH, windowWidth - C.PAGE_MIN_WIDTH);
  return Math.round(Math.max(C.PANEL_MIN_WIDTH, width));
}

/** The width an arrow, Home or End key on the handle asks for, from the panel's `current` width. */
export function keyWidth(key: string, current: number): number {
  if (key === 'Home') return C.PANEL_MIN_WIDTH;
  if (key === 'End') return C.PANEL_MAX_WIDTH;
  return current + (key === 'ArrowLeft' ? C.PANEL_KEY_STEP : -C.PANEL_KEY_STEP);
}

/** The workspace panel. */
export class PanelViewModel extends ViewModel<PanelState> {
  /** The main process. */
  private readonly bridge: PanelBridge;
  /** Batches drag widths into animation frames. */
  private readonly frames: FrameClock;
  /** What Clear does in each pane that has a clear action. */
  private readonly clears = new Map<Pane, () => void>();
  /** The latest dragged width not yet sent. */
  private pendingWidth: number | undefined;
  /** The frame that will send it. */
  private frame: number | null = null;

  /** Closed on Ask, following the main process's panel state and layout. */
  constructor(bridge: PanelBridge, frames: FrameClock) {
    super({ open: false, pane: 'chat', lastInspect: 'actions', layout: null, dragging: false });
    this.bridge = bridge;
    this.frames = frames;
    this.own(bridge.onDevPanelState((open) => this.set({ open })));
    this.own(bridge.onShellLayout((layout) => this.set({ layout, open: layout.panelOpen ?? this.state.open })));
  }

  /** Shows `pane`; an Inspect pane is remembered for the Inspect tab. */
  show(pane: Pane): void {
    this.set(INSPECT_PANES.includes(pane) ? { pane, lastInspect: pane } : { pane });
  }

  /** The Inspect tab: reopens the Inspect pane last shown. */
  showInspect(): void {
    this.show(this.state.lastInspect);
  }

  /** Opens the panel showing `pane`, or just shows it when the panel is open. */
  async open(pane: Pane): Promise<void> {
    this.show(pane);
    if (!this.state.open) await this.bridge.toggleDevPanel();
  }

  /** The Oya Agent button: opens the panel where it was left (on Record while `recording`), or closes it. */
  async toggle(recording = false): Promise<void> {
    const opening = !this.state.open;
    await this.bridge.toggleDevPanel();
    if (opening && recording) this.show('record');
  }

  /** Registers what Clear does while `pane` is in view. */
  onClear(pane: Pane, clear: () => void): () => void {
    this.clears.set(pane, clear);
    return () => void this.clears.delete(pane);
  }

  /** Clears the pane in view; panes without a clear action are left alone. */
  clear(): void {
    this.clears.get(this.state.pane)?.();
  }

  /** A drag on the panel's edge started. */
  startDrag(): void {
    this.set({ dragging: true });
  }

  /** The pointer is at `x` during a drag: the width goes out at most once a frame. */
  dragTo(x: number, windowWidth: number): void {
    if (!this.state.dragging) return;
    this.pendingWidth = widthAt(x, windowWidth);
    this.frame ??= this.frames.request(() => this.flush());
  }

  /** The drag ended: the last width goes out now. */
  endDrag(): void {
    if (!this.state.dragging) return;
    if (this.frame !== null) this.frames.cancel(this.frame);
    this.flush();
    this.set({ dragging: false });
  }

  /** Resizes from the keyboard, from the panel's `current` width. */
  resizeByKey(key: string, current: number): void {
    void this.bridge.resizeDevPanel(keyWidth(key, current));
  }

  /** Sends the pending width, if any. */
  private flush(): void {
    this.frame = null;
    if (this.pendingWidth !== undefined) void this.bridge.resizeDevPanel(this.pendingWidth);
    this.pendingWidth = undefined;
  }
}
