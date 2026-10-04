/**
 * The dim over the page, with a soft window over each element the agent found.
 * It is one canvas, drawn only while a window is opening, gliding or closing,
 * so a hundred windows cost one layer and no frame at rest. Where the windows
 * are comes from VeilWindows; this class only paints them.
 */
import { RendererConstants as C } from '../core/constants.ts';
import { VEIL_CUT, VEIL_LIFT, VEIL_SMOKE } from './constants.ts';
import { middle, outset, scaled } from './geometry.ts';
import type { Rect, ShieldBox, Tone, ViewPort } from './types.ts';
import { VeilWindows, type VeilShape } from './veil-windows.ts';

/** What the veil draws on and when. */
export interface VeilDeps {
  /** The #veil canvas. */
  canvas: HTMLCanvasElement;
  /** The window's size and pixel ratio. */
  view: ViewPort;
  /** Asks for one animation frame (window.requestAnimationFrame). */
  requestFrame: (draw: (now: number) => void) => void;
}

/** The veil over the page. */
export class Veil {
  /** The page under the veil, as the main process measured it. */
  tone: Tone = 'light';
  /** Where each window is. */
  private readonly windows = new VeilWindows();
  /** Whether a frame is already asked for. */
  private pending = false;
  /** The canvas, the window and the frame clock. */
  private readonly deps: VeilDeps;

  /** `deps` is the canvas and the window it fills. */
  constructor(deps: VeilDeps) {
    this.deps = deps;
  }

  /** Opens a window over one element; it grows in from its middle on the next frames. */
  open(box: ShieldBox): void {
    this.windows.open(box);
    this.ask();
  }

  /** The elements moved: each window glides after its element, and closes if it has gone. */
  move(boxes: readonly ShieldBox[]): void {
    this.windows.move(boxes);
    this.ask();
  }

  /** Closes every window. */
  clear(): void {
    this.windows.clear();
    this.ask();
  }

  /** Asks for one frame, once. */
  private ask(): void {
    if (this.pending) return;
    this.pending = true;
    this.deps.requestFrame((now) => this.draw(now));
  }

  /** Draws the smoke, cuts every window out of it, lifts them on a dark page, and asks again while any moves. */
  private draw(now: number): void {
    this.pending = false;
    const ctx = this.context();
    if (!ctx) return;
    const frame = this.windows.frame(now);
    this.smoke(ctx);
    for (const shape of frame.shapes) this.window(ctx, shape);
    if (this.tone === 'dark') this.lift(ctx, frame.shapes);
    if (frame.moving) this.ask();
  }

  /** Fills the whole canvas with smoke, then sets the context to cut windows out of it. */
  private smoke(ctx: CanvasRenderingContext2D): void {
    const all = this.page();
    Object.assign(ctx, { globalCompositeOperation: 'copy', globalAlpha: 1, fillStyle: this.vignette(ctx, all) });
    ctx.fillRect(all.x, all.y, all.w, all.h);
    Object.assign(ctx, { globalCompositeOperation: 'destination-out', fillStyle: VEIL_CUT });
  }

  /** The smoke's fill: clear-ish around the middle of `all`, darkest at its corners. */
  private vignette(ctx: CanvasRenderingContext2D, all: Rect): CanvasGradient {
    const { x, y } = middle(all);
    const reach = Math.hypot(all.w, all.h) * C.SHIELD_HALF;
    const fill = ctx.createRadialGradient(x, y, reach * C.SHIELD_VEIL_CLEAR_SHARE, x, y, reach);
    const smoke = VEIL_SMOKE[this.tone];
    fill.addColorStop(0, smoke.middle);
    fill.addColorStop(1, smoke.edge);
    return fill;
  }

  /** The canvas's area in page pixels: the window, and the bleed past each edge. */
  private page(): Rect {
    const { innerWidth, innerHeight } = this.deps.view;
    return outset({ x: 0, y: 0, w: innerWidth || 0, h: innerHeight || 0 }, C.SHIELD_VEIL_BLEED_PX);
  }

  /** The canvas's context, sized to the window in device pixels, or null where there is none. */
  private context(): CanvasRenderingContext2D | null {
    const ctx = this.deps.canvas.getContext('2d');
    if (ctx) this.fit(ctx, this.deps.view.devicePixelRatio || 1);
    return ctx;
  }

  /** Sizes the canvas to the page in device pixels, and draws in page pixels with the bleed's corner at -bleed. */
  private fit(ctx: CanvasRenderingContext2D, ratio: number): void {
    const { canvas } = this.deps;
    const all = this.page();
    const size = { width: Math.round(all.w * ratio), height: Math.round(all.h * ratio) };
    if (canvas.width !== size.width || canvas.height !== size.height) Object.assign(canvas, size);
    ctx.setTransform(ratio, 0, 0, ratio, -all.x * ratio, -all.y * ratio);
  }

  /** One window, `open` of the way to full size and strength: a soft falloff of light around it, then the clear window itself. */
  private window(ctx: CanvasRenderingContext2D, { box, open }: VeilShape): void {
    for (const { px, share } of C.SHIELD_VEIL_FALLOFF) this.shape(ctx, box, px, open, open * share);
    this.shape(ctx, box, 0, open, open);
  }

  /** Fills a rounded box `px` past the window's edge, grown `open` of the way from its starting size, at `alpha`. */
  private shape(ctx: CanvasRenderingContext2D, box: Rect, px: number, open: number, alpha: number): void {
    const start = C.SHIELD_VEIL_START;
    const r = scaled(outset(box, C.SHIELD_VEIL_PAD_PX + px), start + (1 - start) * open);
    ctx.globalAlpha = alpha;
    ctx.beginPath();
    ctx.roundRect(r.x, r.y, r.w, r.h, C.SHIELD_VEIL_RADIUS_PX + px);
    ctx.fill();
  }

  /** On a dark page, lets a faint light into every window, so what the agent found stands out from the dark around it. */
  private lift(ctx: CanvasRenderingContext2D, shapes: readonly VeilShape[]): void {
    Object.assign(ctx, { globalCompositeOperation: 'source-over', fillStyle: VEIL_LIFT });
    for (const { box, open } of shapes) this.shape(ctx, box, 0, open, open);
  }
}
