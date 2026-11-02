/**
 * The Routines pane's editor: a card at the top of the pane for a new routine
 * or the one being edited, with its name, what it does (asks the agent a prompt,
 * or plays a saved playbook with the values it sets), the profile it runs as,
 * and its schedule ("every N minutes or hours", or "daily at HH:MM"). Saving goes to the project on the server; a
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

/** A saved playbook as the editor offers it: its name and the variables a routine can set. */
export interface PlaybookChoice {
  /** Its name. */
  name: string;
  /** The variables its steps use. */
  variables: string[];
  /** Their saved values (never a secret's), shown as each field's placeholder. */
  defaults: Record<string, string>;
  /** The variables holding secrets, which a routine leaves to the playbook's saved value. */
  secrets: string[];
}

/** A profile the routine can run as; '' is the project's default. */
export interface ProfileChoice {
  /** The persona's id, or '' for the default. */
  value: string;
  /** Its name. */
  label: string;
}

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
  /** The saved playbook it plays, or '' to ask the prompt. */
  playbook: string;
  /** The project's saved playbooks, to choose from. */
  playbooks: PlaybookChoice[];
  /** What the routine sets the playbook's variables to; a blank one uses the playbook's own. */
  values: Record<string, string>;
  /** The persona it runs as, or '' for the default. */
  persona: string;
  /** The project's named personas, to choose from. */
  personas: ProfileChoice[];
  /** Every or Daily at. */
  kind: ScheduleKind;
  /** How often, for Every (as typed). */
  n: string;
  /** minutes or hours, for Every. */
  unit: string;
  /** HH:MM, for Daily at. */
  at: string;
  /** Run in the cloud, by the server, rather than on a desktop. */
  cloud: boolean;
  /** Why the last save was refused, or ''. */
  error: string;
}

/** The editor's text fields. */
export type EditorField = 'name' | 'prompt' | 'playbook' | 'n' | 'unit' | 'at' | 'persona';

/** The editor's settings fields, without its open state and the choices it offers. */
type EditorFields = Omit<RoutineEditorState, 'open' | 'opened' | 'editing' | 'error' | 'playbooks' | 'personas'>;

/** The fields for `routine`'s settings, or a new routine's. */
function fieldsOf(routine: Routine | null): EditorFields {
  const s: Schedule = routine?.schedule ?? DEFAULT_SCHEDULE;
  const kind: ScheduleKind = s.kind === 'daily' ? 'daily' : 'every';
  const n = String(s.n ?? DEFAULT_SCHEDULE.n);
  const [unit, at] = [s.unit ?? DEFAULT_SCHEDULE.unit, s.at ?? DEFAULT_SCHEDULE.at];
  const task = { prompt: routine?.prompt ?? '', playbook: routine?.playbook ?? '', values: { ...routine?.values } };
  const where = { cloud: routine?.target === 'cloud', persona: routine?.persona ?? '' };
  return { name: routine?.name ?? '', ...task, kind, n, unit, at, ...where };
}

/** A list of strings from an answer's field, or none. */
const strings = (value: unknown): string[] => (Array.isArray(value) ? value.map(String) : []);

/** A playbook as the list describes it, as the editor offers it; null without a name. */
function choiceOf(raw: unknown): PlaybookChoice | null {
  const p = (raw ?? {}) as Record<string, unknown>;
  if (!p.name) return null;
  const defaults = p.defaults && typeof p.defaults === 'object' ? (p.defaults as Record<string, string>) : {};
  return { name: String(p.name), variables: strings(p.variables), defaults, secrets: strings(p.secrets) };
}

/**
 * The values to save: only those `playbook` can use, not its secrets, and not
 * blank (a blank one plays the playbook's own value). Unknown playbooks (the
 * list could not be read) keep what the routine had.
 */
export function valuesFor(playbook: PlaybookChoice | undefined, values: Record<string, string>) {
  const kept = Object.entries(values).filter(([name, value]) => value !== '' && !playbook?.secrets.includes(name));
  return Object.fromEntries(playbook ? kept.filter(([name]) => playbook.variables.includes(name)) : kept);
}

/** The parts of the main process the editor calls. */
export type EditorBridge = Pick<OyaBrowser, 'saveRoutine' | 'playbooks' | 'listPersonas'>;

/** A refusal's words, without what Electron puts before an error from the main process. */
export const refusalText = (message: unknown): string => String(message).replace(REMOTE_ERROR_PREFIX, '');

/** The routine editor. */
export class RoutineEditorViewModel extends ViewModel<RoutineEditorState> {
  /** The main process. */
  private readonly bridge: EditorBridge;
  /** Called with the main process's answer once a save went through. */
  private readonly saved: (result: Payload) => void;

  /** Closed; `saved` takes the new list after a save. */
  constructor(bridge: EditorBridge, saved: (result: Payload) => void) {
    super({ open: false, opened: 0, editing: null, error: '', playbooks: [], personas: [], ...fieldsOf(null) });
    this.bridge = bridge;
    this.saved = saved;
  }

  /** Opens the editor on `routine`, or empty for a new one. */
  open(routine: Routine | null): void {
    this.set({ open: true, opened: this.state.opened + 1, editing: routine, error: '', ...fieldsOf(routine) });
    void this.loadPlaybooks();
    void this.loadPersonas();
  }

  /** The project's saved playbooks, to choose one to play; none when they cannot be read. */
  async loadPlaybooks(): Promise<void> {
    const answer = await this.bridge.playbooks({ action: 'list' }).catch(() => null);
    const list = Array.isArray(answer?.playbooks) ? answer.playbooks : [];
    this.set({ playbooks: list.map(choiceOf).filter((p): p is PlaybookChoice => !!p) });
  }

  /** The project's named personas, to choose the one it runs as; none offline. */
  async loadPersonas(): Promise<void> {
    const answer = await this.bridge.listPersonas().catch(() => null);
    const named = (answer?.personas ?? []).filter((p) => !p.isDefault);
    this.set({ personas: named.map((p) => ({ value: p.id, label: p.name })) });
  }

  /** The playbook chosen, as the list describes it, or undefined. */
  chosen(): PlaybookChoice | undefined {
    return this.state.playbooks.find((p) => p.name === this.state.playbook);
  }

  /** A playbook variable's field was typed in. */
  setValue(name: string, value: string): void {
    this.set({ values: { ...this.state.values, [name]: value } });
  }

  /** A field was typed in. */
  setField(field: EditorField, value: string): void {
    this.set({ [field]: value });
  }

  /** Runs the routine in the cloud, or on a desktop. */
  setCloud(cloud: boolean): void {
    this.set({ cloud });
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
    const { editing, name, prompt, playbook, cloud, persona } = this.state;
    const target = cloud ? 'cloud' : 'desktop';
    const played = playbook ? { values: valuesFor(this.chosen(), this.state.values) } : {};
    const task = { prompt, playbook: playbook || null, ...played, persona: persona || null };
    const base = { name, ...task, enabled: editing?.enabled ?? true, schedule: this.schedule(), target };
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
