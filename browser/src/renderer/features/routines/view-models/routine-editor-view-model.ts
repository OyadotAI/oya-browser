/**
 * The Routines pane's editor: a card at the top of the pane for a new routine
 * or the one being edited, with its name, prompt and schedule ("every N minutes
 * or hours", or "daily at HH:MM"). Saving goes to the project on the server; a
 * refusal (a name too long, a time not written HH:MM) shows in the card, which
 * keeps what was typed.
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { Payload } from '../../../../shared/ipc.ts';
import { DEFAULT_SCHEDULE, REMOTE_ERROR_PREFIX } from '../model/constants.ts';
import type { Routine, Schedule } from '../model/types.ts';

/** The schedule kinds the editor offers. */
export type ScheduleKind = 'every' | 'daily';

/** The editor's fields and whether it shows. */
export interface RoutineEditorState {
  /** The card shows. */
  open: boolean;
  /** Bumped on every open, so the view focuses the name field again. */
  opened: number;
  /** The routine being edited, or null for a new one. */
  editing: Routine | null;
  /** The name field. */
  name: string;
  /** The prompt field. */
  prompt: string;
  /** Every or Daily at. */
  kind: ScheduleKind;
  /** How often, for Every (as typed). */
  n: string;
  /** minutes or hours, for Every. */
  unit: string;
  /** HH:MM, for Daily at. */
  at: string;
  /** Why the last save was refused, or ''. */
  error: string;
}

/** The editor's text fields. */
export type EditorField = 'name' | 'prompt' | 'n' | 'unit' | 'at';

/** The fields for `routine`'s settings, or a new routine's. */
function fieldsOf(routine: Routine | null): Omit<RoutineEditorState, 'open' | 'opened' | 'editing' | 'error'> {
  const s: Schedule = routine?.schedule ?? DEFAULT_SCHEDULE;
  const kind: ScheduleKind = s.kind === 'daily' ? 'daily' : 'every';
  const n = String(s.n ?? DEFAULT_SCHEDULE.n);
  const [unit, at] = [s.unit ?? DEFAULT_SCHEDULE.unit, s.at ?? DEFAULT_SCHEDULE.at];
  return { name: routine?.name ?? '', prompt: routine?.prompt ?? '', kind, n, unit, at };
}

/** A refusal's words, without what Electron puts before an error from the main process. */
export const refusalText = (message: unknown): string => String(message).replace(REMOTE_ERROR_PREFIX, '');

/** The routine editor. */
export class RoutineEditorViewModel extends ViewModel<RoutineEditorState> {
  /** The main process. */
  private readonly bridge: Pick<OyaBrowser, 'saveRoutine'>;
  /** Called with the main process's answer once a save went through. */
  private readonly saved: (result: Payload) => void;

  /** Closed; `saved` takes the new list after a save. */
  constructor(bridge: Pick<OyaBrowser, 'saveRoutine'>, saved: (result: Payload) => void) {
    super({ open: false, opened: 0, editing: null, error: '', ...fieldsOf(null) });
    this.bridge = bridge;
    this.saved = saved;
  }

  /** Opens the editor on `routine`, or empty for a new one. */
  open(routine: Routine | null): void {
    this.set({ open: true, opened: this.state.opened + 1, editing: routine, error: '', ...fieldsOf(routine) });
  }

  /** A field was typed in. */
  setField(field: EditorField, value: string): void {
    this.set({ [field]: value });
  }

  /** Picks the schedule kind, showing its fields. */
  setKind(kind: ScheduleKind): void {
    this.set({ kind });
  }

  /** The schedule the fields describe. */
  schedule(): Schedule {
    const { kind, at, n, unit } = this.state;
    return kind === 'daily' ? { kind, at } : { kind, n: Number(n), unit };
  }

  /** What the fields hold, as a routine to save; an edit keeps its id and whether it is on. */
  routine(): Payload {
    const { editing, name, prompt } = this.state;
    const base = { name, prompt, enabled: editing?.enabled ?? true, schedule: this.schedule() };
    return editing ? { ...base, id: editing.id } : base;
  }

  /** Saves to the project; a refusal stays in the card with what was typed. */
  async submit(): Promise<void> {
    const result = await this.bridge.saveRoutine(this.routine()).catch((err: Error) => ({ error: err.message }));
    if (result?.error) return void this.set({ error: refusalText(result.error) });
    this.close();
    this.saved(result);
  }

  /** Closes the editor. */
  close(): void {
    this.set({ open: false, editing: null });
  }
}
