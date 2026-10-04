/**
 * What the shield draws as elements: the outlines with their number badges,
 * the sparks that fly into the orb, and an action's target ring with its
 * flight of light. Each is placed exactly over its element, snapped to device
 * pixels, and coloured by the element's kind.
 */
import { RendererConstants as C } from '../core/constants.ts';
import { DEFAULT_COLOR, TYPE_COLORS } from './constants.ts';
import { middle, tucked } from './geometry.ts';
import { TagLayout } from './tag-layout.ts';
import type { Point, Rect, ShieldBox, ViewPort } from './types.ts';

/** The stage's parts of the page. */
export interface StageParts {
  /** The outlines (#stage). */
  stage: HTMLElement;
  /** The sparks in flight (#sparks). */
  sparks: HTMLElement;
  /** An action's ring and flight (#targets). */
  targets: HTMLElement;
  /** The companion's orb (.orb), where sparks land and flights leave from. */
  orb: HTMLElement;
  /** The document, to make elements. */
  document: Document;
  /** The window's pixel ratio. */
  view: ViewPort;
}

/** The colour of an element of `type`: its kind's, or a click's. */
export function colorOf(type: string): string {
  return Object.hasOwn(TYPE_COLORS, type) ? TYPE_COLORS[type] : DEFAULT_COLOR;
}

/** The outlines, sparks and targets. */
export class Stage {
  /** Where this show's badges sit. */
  private readonly tags = new TagLayout();
  /** The elements it draws in. */
  private readonly parts: StageParts;

  /** `parts` is the stage's elements. */
  constructor(parts: StageParts) {
    this.parts = parts;
  }

  /** How many outlines are up. */
  get count(): number {
    return this.parts.stage.children.length;
  }

  /** Marks the stage busy (many outlines), so their numbers step back once locked on. */
  busy(on: boolean): void {
    this.parts.stage.classList.toggle('busy', on);
  }

  /** Fades every outline out. */
  leave(): void {
    this.parts.stage.classList.add('leaving');
  }

  /** Removes every outline and spark, and makes the stage visible for the next ones. */
  clear(): void {
    const { stage, sparks } = this.parts;
    stage.replaceChildren();
    stage.classList.remove('leaving');
    this.tags.reset();
    sparks.replaceChildren();
  }

  /** Adds one outline with its number (tucked in at the window's edge, left out where it would land on another). */
  outline(box: ShieldBox): void {
    const el = this.make('div', ['box', tucked(box) && 'tucked', !this.tags.claim(box) && 'quiet']);
    el.dataset.id = String(box.id);
    const badge = this.parts.document.createElement('b');
    badge.textContent = String(box.id);
    el.append(badge);
    this.place(el, box);
    el.style.setProperty('--c', colorOf(box.type));
    this.parts.stage.append(el);
  }

  /** The page moved: each outline glides to its element, and one whose element is gone fades (and comes back if it returns). */
  move(boxes: readonly ShieldBox[]): void {
    const at = new Map(boxes.map((box) => [String(box.id), box]));
    for (const el of Array.from(this.parts.stage.children) as HTMLElement[]) {
      const box = at.get(el.dataset.id ?? '');
      el.classList.toggle('gone', !box);
      if (box) this.place(el, box);
    }
  }

  /** A spark leaves the middle of an element just after its outline locks on, and flies to the orb. */
  spark(box: ShieldBox): void {
    const orb = this.orbCentre();
    if (orb) this.parts.sparks.append(this.flight(middle(box), orb, box.type, C.SHIELD_SPARK_LAG_MS));
  }

  /** Light flies from the orb to the element an action is aimed at, then a ring locks on to it; both go once played. */
  target(box: ShieldBox): void {
    const ring = this.make('div', ['target']);
    this.place(ring, box);
    ring.style.setProperty('--c', colorOf(box.type));
    const orb = this.orbCentre();
    const shown = orb ? [ring, this.flight(orb, middle(box), box.type, 0)] : [ring];
    this.parts.targets.append(...shown);
    setTimeout(() => shown.forEach((el) => el.remove()), C.SHIELD_TARGET_MS);
  }

  /** A point of light flying on an arc from `from` to `to`, in the colour of a `type` element, leaving after `lag` ms. */
  private flight(from: Point, to: Point, type: string, lag: number): HTMLElement {
    const el = this.make('div', ['spark']);
    el.append(this.parts.document.createElement('i'));
    const vars = { '--fx': from.x, '--fy': from.y, '--dx': to.x - from.x, '--dy': to.y - from.y };
    for (const [name, value] of Object.entries(vars)) el.style.setProperty(name, `${Math.round(value)}px`);
    el.style.setProperty('--c', colorOf(type));
    el.style.setProperty('--d', `${lag}ms`);
    return el;
  }

  /** Puts an element exactly over its box, snapped to device pixels so its hairline stays crisp, with its lock-on reach. */
  private place(el: HTMLElement, box: Rect): void {
    const ratio = this.parts.view.devicePixelRatio || 1;
    const px = (v: number): string => `${Math.round(v * ratio) / ratio}px`;
    el.style.transform = `translate3d(${px(box.x)}, ${px(box.y)}, 0)`;
    Object.assign(el.style, { width: px(box.w), height: px(box.h) });
    el.style.setProperty('--sx', String(1 + C.SHIELD_LOCK_REACH_PX / Math.max(1, box.w)));
    el.style.setProperty('--sy', String(1 + C.SHIELD_LOCK_REACH_PX / Math.max(1, box.h)));
  }

  /** The middle of the orb, or null before it is laid out. */
  private orbCentre(): Point | null {
    const r = this.parts.orb.getBoundingClientRect();
    const centre = middle({ x: r.left, y: r.top, w: r.width, h: r.height });
    return Number.isFinite(centre.x) && Number.isFinite(centre.y) ? centre : null;
  }

  /** A `tag` element with the truthy ones of `classes`. */
  private make(tag: string, classes: (string | false)[]): HTMLElement {
    const el = this.parts.document.createElement(tag);
    el.className = classes.filter(Boolean).join(' ');
    return el;
  }
}
