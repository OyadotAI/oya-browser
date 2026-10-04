/**
 * The show of one reading: while the agent reads, the beam sweeps the page;
 * when the reading is back, a pass of light crosses it and each element it
 * found locks on as the pass reaches it, while Oya counts them; then the
 * outlines hold, fade, and the companion rests. A move glides them, and an
 * action that changes the page dismisses them at once.
 */
import { RendererConstants as C } from '../core/constants.ts';
import type { Companion } from './companion.ts';
import { WORDS } from './constants.ts';
import { beamReaches, counted, declutter, topDown } from './geometry.ts';
import type { ShowClock } from './show-clock.ts';
import type { Stage } from './stage.ts';
import type { FoundUpdate, MoveUpdate, ShieldBox, ViewPort } from './types.ts';
import type { Veil } from './veil.ts';

/** What the show plays on. */
export interface ShowParts {
  /** The page's body, whose classes (scanning, revealing, lit, dismissing) drive the CSS. */
  body: HTMLElement;
  /** The outlines. */
  stage: Stage;
  /** The dim with its windows. */
  veil: Veil;
  /** Oya's face and caption. */
  companion: Companion;
  /** The show's timing. */
  clock: ShowClock;
  /** The window's size. */
  view: ViewPort;
}

/** The scan and outlines of the current reading. */
export class Show {
  /** Where the found elements sit now: the result, then each move measured after it. */
  private latest: ShieldBox[] = [];
  /** The parts it plays on. */
  private readonly parts: ShowParts;

  /** `parts` is what the show plays on. */
  constructor(parts: ShowParts) {
    this.parts = parts;
  }

  /** A new read: every timer the last show left does nothing now. */
  restart(): void {
    this.parts.clock.bump();
  }

  /** The reading began: once any pass still crossing the page is done, the beam sweeps and Oya says so. */
  scan(): void {
    const wait = this.parts.clock.scanWait();
    if (wait <= 0) return this.read();
    this.parts.clock.later(wait, () => this.read());
  }

  /** The reading is back: once the scan has run its minimum and its beam has ended a sweep, outline what it found. */
  finish({ boxes, tone }: FoundUpdate): void {
    this.latest = boxes || [];
    this.parts.veil.tone = tone === 'dark' ? 'dark' : 'light';
    this.parts.clock.later(this.parts.clock.untilReveal(), () => this.found(this.latest));
  }

  /** The page moved under the outlines: each glides to where its element is now, and one whose element is gone fades. */
  move({ boxes }: MoveUpdate): void {
    this.latest = boxes || [];
    this.parts.veil.move(this.latest);
    this.parts.stage.move(this.latest);
  }

  /** The agent changed the page: what the show outlines is out of date, so it lets go now, and a waiting reveal never plays. */
  dismiss(): void {
    const { clock, body } = this.parts;
    clock.bump();
    clock.stop();
    this.parts.stage.leave();
    body.classList.remove('scanning', 'lit');
    this.dismissing();
  }

  /** The reading itself: the beam sweeps the page on a steady loop while the companion says so. */
  private read(): void {
    const { body } = this.parts;
    this.parts.clock.reading();
    this.letGo();
    body.style.setProperty('--loop', `${C.SHIELD_SCAN_LOOP_MS}ms`);
    body.classList.remove('revealing', 'lit');
    body.classList.add('scanning');
    this.parts.companion.say('scanning', WORDS.reading, true);
  }

  /** The pass crosses the page, locking on to each element as it reaches it, and Oya counts them; then the show fades. */
  private found(boxes: ShieldBox[]): void {
    this.clear();
    this.reveal();
    const shown = topDown(declutter(boxes, this.parts.view.innerWidth * this.parts.view.innerHeight || 0));
    this.parts.stage.busy(shown.length > C.SHIELD_BUSY_COUNT);
    this.parts.companion.say('found', shown.length ? counted(0) : WORDS.nothing);
    this.lockInTurn(shown, boxes.length);
    this.parts.clock.later(C.SHIELD_REVEAL_MS + C.SHIELD_HOLD_MS, () => this.fade());
  }

  /** Locks each shown outline as the pass reaches it, counting up to `total`; one in every few sends a spark. */
  private lockInTurn(shown: readonly ShieldBox[], total: number): void {
    const every = Math.max(1, Math.ceil(shown.length / C.SHIELD_SPARKS_MAX));
    shown.forEach((box, i) => {
      const words = counted(Math.round(((i + 1) * total) / shown.length));
      const at = beamReaches(box.y, this.parts.view.innerHeight);
      this.parts.clock.later(at, () => this.lock(box, words, i % every === 0));
    });
  }

  /** The edge light turns into the reveal: the wash passes once, then the light lets go. */
  private reveal(): void {
    const { body, clock } = this.parts;
    clock.revealing();
    // Set now, while the stage is empty: setting them as the fade begins would restyle every outline at once.
    body.style.setProperty('--reveal', `${C.SHIELD_REVEAL_MS}ms`);
    body.style.setProperty('--fade', `${C.SHIELD_FADE_MS}ms`);
    body.classList.remove('scanning');
    body.classList.add('revealing', 'lit');
    clock.later(C.SHIELD_REVEAL_MS, () => body.classList.remove('revealing'));
  }

  /** The pass reached one element: its outline locks on where it is now, the count goes up, and maybe a spark flies. */
  private lock(box: ShieldBox, words: string, spark: boolean): void {
    this.parts.companion.count(words);
    const now = this.latest.find((b) => b.id === box.id);
    // Its element has gone since it was measured: it is counted, but nothing is drawn where it was.
    if (!now) return;
    this.parts.stage.outline(now);
    this.parts.veil.open(now);
    if (spark) this.parts.stage.spark(now);
  }

  /** Fades out whatever the last show left up, rather than cutting it: its outlines fade and its windows close. */
  private letGo(): void {
    if (!this.parts.stage.count) return this.clear();
    this.parts.stage.leave();
    this.parts.veil.move([]);
    this.dismissing();
  }

  /** Plays the quick let-go, then clears what is left. */
  private dismissing(): void {
    const { body } = this.parts;
    body.classList.add('dismissing');
    this.parts.clock.later(C.SHIELD_DISMISS_MS, () => {
      this.clear();
      body.classList.remove('revealing', 'dismissing');
    });
  }

  /** Fades the outlines and the veil out, then puts the companion back to rest. */
  private fade(): void {
    this.parts.stage.leave();
    this.parts.body.classList.remove('lit');
    this.parts.companion.rest();
    this.parts.clock.later(C.SHIELD_FADE_MS, () => {
      this.clear();
      this.parts.body.classList.remove('revealing');
      this.parts.companion.rest();
    });
  }

  /** No outlines, sparks or windows. */
  private clear(): void {
    this.parts.stage.clear();
    this.parts.veil.clear();
  }
}
