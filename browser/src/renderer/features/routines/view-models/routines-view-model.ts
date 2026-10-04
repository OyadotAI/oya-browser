/**
 * The Routines pane: the project's routines, the prompts its Oya apps run on a
 * schedule. They live on the server, so every desktop on the project shows the
 * same list and history; the main process keeps the list in step with the
 * server (src/main/routines/routines.ts) and pushes every change here. This
 * holds which histories, runs and menus are open, the delete confirmation,
 * notes that expire, and the clock that redraws times while the pane shows.
 */
import { ViewModel } from '../../../core/view-model.ts';
import { RendererConstants as C } from '../../../core/constants.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import { RoutineEditorViewModel } from './routine-editor-view-model.ts';
import type { Routine, RoutinesSnapshot } from '../model/types.ts';

/** What the pane shows. */
export interface RoutinesState {
  /** What the main process last sent. */
  snapshot: RoutinesSnapshot;
  /** Ids of the routines whose history shows. */
  expanded: readonly string[];
  /** Ids of the runs whose details are open. */
  openRuns: readonly string[];
  /** The routine whose menu is open, or null. */
  menu: string | null;
  /** The routine asking to confirm its deletion, or null. */
  confirming: string | null;
  /** Notes under routines (why Run now could not start), by id. */
  notes: Readonly<Record<string, string>>;
  /** The time the times are told from; the clock moves it. */
  now: number;
}

/** The parts of the bridge the pane uses. */
export type RoutinesBridge = Pick<
  OyaBrowser,
  | 'listRoutines'
  | 'saveRoutine'
  | 'deleteRoutine'
  | 'runRoutineNow'
  | 'setRoutineEnabled'
  | 'clearRoutineHistory'
  | 'stopRoutine'
  | 'onRoutinesChanged'
>;

/** The part of the panel's state the clock reads. */
export interface PanelPane {
  /** The pane in view. */
  readonly pane: string;
}

/** The pane in view, which the clock follows (the workspace panel). */
export interface RoutinesPanel {
  /** The panel's state: the pane in view. */
  readonly state: PanelPane;
  /** Calls back when the panel changes. */
  subscribe(listener: () => void): () => void;
}

/** What the pane is built from. */
export interface RoutinesServices {
  /** The main process. */
  bridge: RoutinesBridge;
  /** The workspace panel. */
  panel: RoutinesPanel;
}

/** Whether `value` is a snapshot of the list (an answer may instead be a refusal). */
const isSnapshot = (value: unknown): value is RoutinesSnapshot =>
  Array.isArray((value as Partial<RoutinesSnapshot> | null)?.routines);

/** `ids` with `id` added, or taken out when `on` is false. */
const toggled = (ids: readonly string[], id: string, on: boolean): string[] =>
  on ? [...ids.filter((x) => x !== id), id] : ids.filter((x) => x !== id);

/** An answer that may say why it was refused. */
interface Refusal {
  /** Why, when it was refused. */
  error?: unknown;
}

/** An answer: a snapshot, a refusal, or nothing. */
type Answer = Refusal | null | undefined;

/** The Routines pane. */
export class RoutinesViewModel extends ViewModel<RoutinesState> {
  /** The editor card. */
  readonly editor: RoutineEditorViewModel;
  /** The main process. */
  private readonly bridge: RoutinesBridge;
  /** The clock that redraws times while the pane shows, or null. */
  private clock: ReturnType<typeof setInterval> | null = null;

  /** Online and empty until the main process answers; follows its changes and the pane in view. */
  constructor({ bridge, panel }: RoutinesServices) {
    const snapshot = { routines: [], running: null, online: true, busy: '', error: '' };
    super({ snapshot, expanded: [], openRuns: [], menu: null, confirming: null, notes: {}, now: Date.now() });
    this.bridge = bridge;
    this.editor = new RoutineEditorViewModel(bridge, (result) => this.show(result));
    this.own(bridge.onRoutinesChanged((state) => this.show(state)));
    this.watch(panel);
    void this.load();
  }

  /** Runs the clock while the panel shows this pane. */
  private watch(panel: RoutinesPanel): void {
    this.own(panel.subscribe(() => this.follow(panel.state.pane === 'routines')));
    this.own(() => this.follow(false));
    this.follow(panel.state.pane === 'routines');
  }

  /** Loads the list from the server, through the main process. */
  async load(): Promise<void> {
    this.apply(await this.bridge.listRoutines().catch((e: Error) => ({ error: e.message })));
  }

  /** Shows a snapshot from the main process, with fresh times; anything else keeps the last one. */
  show(state: unknown): void {
    this.set(isSnapshot(state) ? { snapshot: state, now: Date.now() } : { now: Date.now() });
  }

  /** Shows a new list, or a refusal as a note under routine `id` (or in the banner without one). */
  private apply(result: Answer, id?: string): void {
    if (result?.error) return this.note(id, String(result.error));
    this.show(result);
  }

  /** A note under a routine for a few seconds; without a routine, the banner says it. */
  note(id: string | undefined, message: string): void {
    if (!id) return this.set({ snapshot: { ...this.state.snapshot, error: message } });
    this.set({ notes: { ...this.state.notes, [id]: message } });
    const timer = setTimeout(() => this.dropNote(id, message), C.ROUTINE_NOTE_MS);
    this.own(() => clearTimeout(timer));
  }

  /** Takes the note off routine `id`, unless a newer one replaced it. */
  private dropNote(id: string, message: string): void {
    if (this.state.notes[id] !== message) return;
    const { [id]: _gone, ...notes } = this.state.notes;
    this.set({ notes });
  }

  /** Opens or closes a routine's menu. */
  toggleMenu(id: string): void {
    this.set({ menu: this.state.menu === id ? null : id });
  }

  /** A press outside an open menu, or Escape: closes it. */
  closeMenu(): void {
    this.set({ menu: null });
  }

  /** Shows or hides a routine's history, opening its latest run. */
  toggleHistory(routine: Routine): void {
    const open = !this.state.expanded.includes(routine.id);
    const latest = routine.runs?.[0]?.id;
    const openRuns = open && latest ? toggled(this.state.openRuns, latest, true) : this.state.openRuns;
    this.set({ expanded: toggled(this.state.expanded, routine.id, open), openRuns });
  }

  /** Keeps whether a run's details are open across redraws. */
  setRunOpen(id: string, open: boolean): void {
    if (this.state.openRuns.includes(id) !== open) this.set({ openRuns: toggled(this.state.openRuns, id, open) });
  }

  /** Asks to confirm deleting routine `id`, or cancels with null; closes the menu. */
  askDelete(id: string | null): void {
    this.set({ confirming: id, menu: null });
  }

  /** Opens the editor on a routine, or on a new one with null; closes the menu. */
  edit(routine: Routine | null): void {
    this.set({ menu: null });
    this.editor.open(routine);
  }

  /** Turns the routine's schedule on or off. */
  async setEnabled(routine: Routine): Promise<void> {
    this.apply(await this.bridge.setRoutineEnabled(routine.id, !routine.enabled), routine.id);
  }

  /** Runs the routine now; why it could not start shows under it. */
  async runNow(id: string): Promise<void> {
    this.apply(await this.bridge.runRoutineNow(id), id);
  }

  /** Stops this browser's run of the routine. */
  async stop(id: string): Promise<void> {
    await this.bridge.stopRoutine(id);
  }

  /** Clears the routine's finished runs; closes the menu. */
  async clearHistory(id: string): Promise<void> {
    this.set({ menu: null });
    this.apply(await this.bridge.clearRoutineHistory(id), id);
  }

  /** Deletes the routine; the editor stops editing it. */
  async remove(id: string): Promise<void> {
    this.set({ confirming: null });
    if (this.editor.state.editing?.id === id) this.editor.close();
    this.apply(await this.bridge.deleteRoutine(id), id);
  }

  /** Starts the clock (with fresh times) when the pane shows, and stops it when it hides. */
  private follow(visible: boolean): void {
    if (visible && !this.clock) {
      this.set({ now: Date.now() });
      this.clock = setInterval(() => this.set({ now: Date.now() }), C.ROUTINES_CLOCK_MS);
    }
    if (!visible && this.clock) {
      clearInterval(this.clock);
      this.clock = null;
    }
  }

  /** Stops the editor and everything the pane started. */
  override dispose(): void {
    this.editor.dispose();
    super.dispose();
  }
}
