/**
 * The control shield's composition root: finds the page's parts, builds the
 * show, the veil, the companion and the actor, and routes each update from the
 * main process to the one that plays it.
 */
import { Actor } from './actor.ts';
import { Companion, type CompanionParts } from './companion.ts';
import { Show } from './show.ts';
import { ShowClock } from './show-clock.ts';
import { Stage, type StageParts } from './stage.ts';
import type { Phase, ShieldUpdate, ViewPort } from './types.ts';
import { Veil } from './veil.ts';

/** What the shield needs from its page. */
export interface ShieldHost {
  /** The shield's document (index.html). */
  document: Document;
  /** The window's size and pixel ratio. */
  view: ViewPort;
  /** Asks for one animation frame (window.requestAnimationFrame). */
  requestFrame: (draw: (now: number) => void) => void;
}

/** Plays one update of phase P. */
type Play<P extends Phase> = (update: Extract<ShieldUpdate, Record<'phase', P>>) => void;

/** One handler per update, each taking its own payload. */
type Handlers = { [P in Phase]: Play<P> };

/** The element `selector` finds in `document`; the page is ours, so a missing one is a broken build. */
function part<T extends Element = HTMLElement>(document: Document, selector: string): T {
  const el = document.querySelector<T>(selector);
  if (!el) throw new Error(`The control shield page has no ${selector}`);
  return el;
}

/** The shield over the page. */
export class Shield {
  /** Each update's handler, by phase. */
  private readonly phases: Handlers;
  /** The show of the current reading. */
  private readonly show: Show;

  /** `host` is the page the shield draws on. */
  constructor({ document, view, requestFrame }: ShieldHost) {
    const at = (selector: string): HTMLElement => part(document, selector);
    const companion = new Companion({ ...companionParts(at), document });
    const stage = new Stage({ ...stageParts(at), document, view });
    const veil = new Veil({ canvas: part<HTMLCanvasElement>(document, '#veil'), view, requestFrame });
    this.show = new Show({ body: document.body, stage, veil, companion, clock: new ShowClock(), view });
    this.phases = handlers(this.show, new Actor({ show: this.show, companion, stage }));
  }

  /** One update from the main process; anything that is not a known update is ignored. */
  update(update: unknown): void {
    const phase = phaseOf(update);
    if (phase === null || !Object.hasOwn(this.phases, phase)) return;
    // Only a new read restarts the show; a move or an action plays over it.
    if (phase === 'scan' || phase === 'found') this.show.restart();
    (this.phases[phase] as (update: ShieldUpdate) => void)(update as ShieldUpdate);
  }
}

/** Finds one of the page's elements by selector. */
type Find = (selector: string) => HTMLElement;

/** The companion's elements. */
function companionParts(at: Find): Omit<CompanionParts, 'document'> {
  return { root: at('#companion'), words: at('#bubble-text'), pulse: at('.orb .pulse') };
}

/** The stage's elements. */
function stageParts(at: Find): Omit<StageParts, 'document' | 'view'> {
  return { stage: at('#stage'), sparks: at('#sparks'), targets: at('#targets'), orb: at('.orb') };
}

/** The handler of each update. */
function handlers(show: Show, actor: Actor): Handlers {
  return {
    scan: () => show.scan(),
    found: (update) => show.finish(update),
    move: (update) => show.move(update),
    act: (update) => actor.act(update),
    end: () => actor.end(),
  };
}

/** The phase an update names, or null when it names none. */
function phaseOf(update: unknown): Phase | null {
  if (typeof update !== 'object' || update === null || !('phase' in update)) return null;
  return typeof update.phase === 'string' ? (update.phase as Phase) : null;
}
