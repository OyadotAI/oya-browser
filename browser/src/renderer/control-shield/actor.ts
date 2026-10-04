/**
 * What Oya shows between reads: each action the agent takes is said in the
 * caption, the orb pulses, and light reaches out to the element it acts on.
 * The caption goes quiet once the agent has been still a while.
 */
import { RendererConstants as C } from '../core/constants.ts';
import type { Companion } from './companion.ts';
import { WORDS } from './constants.ts';
import type { Show } from './show.ts';
import type { Stage } from './stage.ts';
import type { ActUpdate } from './types.ts';

/** What the actor plays on. */
export interface ActorParts {
  /** The show, dismissed when an action changes the page. */
  show: Show;
  /** Oya's face and caption. */
  companion: Companion;
  /** Where the target ring is drawn. */
  stage: Stage;
}

/** The agent's actions, as Oya tells them. */
export class Actor {
  /** The timer that quiets the caption. */
  private timer: ReturnType<typeof setTimeout> | undefined;
  /** The parts it plays on. */
  private readonly parts: ActorParts;

  /** `parts` is what the actor plays on. */
  constructor(parts: ActorParts) {
    this.parts = parts;
  }

  /** One action: a change to the page dismisses the show; Oya says it, pulses, and targets the element when it was measured. */
  act({ text, box, changes }: ActUpdate): void {
    if (changes) this.parts.show.dismiss();
    this.parts.companion.say('acting', String(text));
    this.parts.companion.pulse();
    if (box) this.parts.stage.target(box);
    this.quietIn(C.SHIELD_ACT_HOLD_MS);
  }

  /** The run ended: whatever the show still drew lets go, and Oya says it is done before going quiet. */
  end(): void {
    this.parts.show.dismiss();
    this.parts.companion.say('acting', WORDS.done);
    this.quietIn(C.SHIELD_DONE_MS);
  }

  /** Quiets the caption `ms` from now, unless another action comes first. */
  private quietIn(ms: number): void {
    clearTimeout(this.timer);
    this.timer = setTimeout(() => this.parts.companion.quiet(), ms);
  }
}
