/**
 * The selected step's editor intents: switch it on or off, set a breakpoint,
 * move, duplicate, delete, run to it, pick its target on the page, and edit
 * its target, value, frames, timeout and alternatives. Every change that
 * depends on the step reads it as it is when its command's turn comes, never
 * a copy from the last draw, so two quick edits both land.
 */
import { SAY } from '../model/constants.ts';
import { firstTarget, parseFrames, sameTarget } from '../model/step-format.ts';
import type { StudioViewModel } from './studio-view-model.ts';
import type { Candidate, Step, StepPatch } from '../model/types.ts';

/** A target field the editor changes. */
type TargetKey = 'kind' | 'value' | 'role';

/** The step editor. */
export class StepEditor {
  /** The studio it edits in. */
  private readonly studio: StudioViewModel;

  /** Edits the steps of `studio`'s draft. */
  constructor(studio: StudioViewModel) {
    this.studio = studio;
  }

  /** The step with `id` as it is now. */
  private current(id: string): Step | undefined {
    return this.studio.workspace?.draft.steps.find((s) => s.id === id);
  }

  /** Changes fields of step `id`. */
  patch(id: string, patch: StepPatch): Promise<unknown> {
    return this.studio.command({ type: 'update', id, patch });
  }

  /** Flips the step's `enabled` or `breakpoint`, as it is when the command's turn comes. */
  toggle(id: string, key: 'enabled' | 'breakpoint'): Promise<unknown> {
    return this.studio.command(() => ({ type: 'update', id, patch: { [key]: !this.current(id)?.[key] } }));
  }

  /** Moves the step up (-1) or down (1). */
  move(id: string, delta: number): Promise<unknown> {
    return this.studio.command({ type: 'move', id, delta });
  }

  /** Duplicates the step after itself. */
  duplicate(id: string): Promise<unknown> {
    return this.studio.command({ type: 'duplicate', id });
  }

  /** Deletes the step. */
  remove(id: string): Promise<unknown> {
    return this.studio.command({ type: 'delete', id });
  }

  /** Runs the workflow up to this step. */
  runTo(id: string): Promise<void> {
    return this.studio.actions.validate({ runTo: id });
  }

  /** Picks the target in the page inspector, saying how. */
  async pick(id: string): Promise<void> {
    if (await this.studio.command({ type: 'pick', id })) this.studio.say(SAY.pick);
  }

  /** Changes one key of the step's first target, applied to the target as it is now. */
  setTarget(id: string, key: TargetKey, value: string): Promise<unknown> {
    return this.studio.command(() => {
      const candidates = this.current(id)?.candidates || [];
      const target = { ...firstTarget(this.current(id)), [key]: value };
      return { type: 'update', id, patch: { candidates: [target, ...candidates.slice(1)] } };
    });
  }

  /** Sets the frames from "outer → inner" text. */
  setFrames(id: string, text: string): Promise<unknown> {
    return this.patch(id, { frames: parseFrames(text) });
  }

  /** Sets the timeout; an emptied field sends none. */
  setTimeout(id: string, timeout: number | undefined): Promise<unknown> {
    return this.patch(id, { timeout });
  }

  /** Makes another recorded target the first, keeping the others behind it. */
  chooseTarget(id: string, choice: Candidate): Promise<unknown> {
    return this.studio.command(() => {
      const candidates = this.current(id)?.candidates || [];
      const rest = candidates.filter((c) => !sameTarget(c, choice));
      return { type: 'update', id, patch: { candidates: [choice, ...rest] } };
    });
  }
}
