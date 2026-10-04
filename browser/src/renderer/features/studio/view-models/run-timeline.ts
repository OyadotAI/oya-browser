/**
 * The Run tab's intents: the run controls, what the next run is given (its
 * inputs and replay speed), opening a previous run, a timeline event opening
 * its step, and applying or reviewing a repair the run found.
 */
import type { RunCommand } from '../model/constants.ts';
import { canApply } from '../model/run-format.ts';
import { sameTarget } from '../model/step-format.ts';
import type { StudioViewModel } from './studio-view-model.ts';
import type { Repair } from '../model/types.ts';

/** The Run tab. */
export class RunTimeline {
  /** The studio whose run it shows. */
  private readonly studio: StudioViewModel;

  /** Acts on `studio`'s run. */
  constructor(studio: StudioViewModel) {
    this.studio = studio;
  }

  /** Pauses, resumes, steps or stops the test run. */
  control(command: RunCommand): Promise<unknown> {
    return this.studio.command({ type: 'control', command });
  }

  /** Changes what run input `name` gives the next run. */
  setInput(name: string, value: string): void {
    this.studio.mark({ runInputs: { ...this.studio.state.runInputs, [name]: value } });
  }

  /** Sets the replay speed: milliseconds of pause per step. */
  setSpeed(runSpeed: number): void {
    this.studio.mark({ runSpeed });
  }

  /** Shows a previous run ('' is the list's placeholder, which does nothing). */
  async openRun(id: string): Promise<void> {
    if (id) await this.studio.command({ type: 'open-run', id });
  }

  /** A timeline event opens its step on the Steps tab. */
  openStep(stepId: string): void {
    this.studio.select(stepId);
    this.studio.selectTab('steps');
  }

  /** Makes the target that worked the step's first, keeping the others behind it; Undo takes it back. */
  async applyRepair(repair: Repair): Promise<void> {
    const s = this.studio.workspace;
    const step = s?.draft.steps.find((item) => item.id === repair.stepId);
    if (!s || !step || !canApply(s, repair)) return;
    const rest = (step.candidates || []).filter((c) => !sameTarget(c, repair.replacement));
    await this.studio.command({ type: 'update', id: step.id, patch: { candidates: [repair.replacement, ...rest] } });
  }

  /** Opens the repaired copy on the Steps tab. */
  async reviewRepair(repair: Repair): Promise<void> {
    await this.studio.command({ type: 'open', id: repair.draftId });
    this.studio.selectTab('steps');
  }
}
