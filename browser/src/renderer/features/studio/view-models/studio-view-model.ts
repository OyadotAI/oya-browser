/**
 * The workflow studio (the Record pane): the last workspace snapshot, the
 * studio tab, the selected step, the messages under each action, and what is
 * typed into the name, description and run inputs. Workspace commands run one
 * at a time, in order; a command may be a function, built when its turn comes,
 * so an edit made before the last one's answer arrived builds on that answer.
 * The intents for recording and saving, the step editor, the variables and the
 * run live on collaborators (`actions`, `editor`, `variables`, `run`).
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { RendererServices } from '../../../app/services.ts';
import { STUDIO_TABS, type Slot, type StudioTab } from '../model/constants.ts';
import {
  INITIAL_STATE,
  NO_MESSAGES,
  cleanError,
  draftName,
  runInputsFor,
  type StudioState,
} from '../model/studio-model.ts';
import type { WorkspaceCommand, WorkspaceSnapshot } from '../model/types.ts';
import { StudioActions, type RecordingGate, type StudioClipboard } from './studio-actions.ts';
import { StepEditor } from './step-editor.ts';
import { StudioVariables } from './studio-variables.ts';
import { RunTimeline } from './run-timeline.ts';

/** What the studio is built from: the shared services, the control bar's gate and the clipboard. */
export type StudioDeps = Pick<RendererServices, 'bridge' | 'shell' | 'panel'> & {
  /** The control bar's guard on Start recording (wired by the root from the control feature). */
  gate: RecordingGate;
  /** Where Copy code writes (navigator.clipboard in the page). */
  clipboard: StudioClipboard;
};

/** A command, or a function that builds it when its turn comes. */
export type CommandSource = WorkspaceCommand | (() => WorkspaceCommand);

/** The fields of the state collaborators may change directly. */
export type StudioFlags = Partial<
  Pick<StudioState, 'busy' | 'saving' | 'justFinished' | 'revealFinish' | 'selected' | 'runInputs' | 'runSpeed'>
>;

/** A workspace answer as a snapshot, or undefined when it carries no draft. */
function asSnapshot(value: unknown): WorkspaceSnapshot | undefined {
  const draft = (value as Partial<WorkspaceSnapshot> | null | undefined)?.draft;
  return draft && typeof draft === 'object' ? (value as WorkspaceSnapshot) : undefined;
}

/** The workflow studio. */
export class StudioViewModel extends ViewModel<StudioState> {
  /** Record, test run, save, export and the other buttons. */
  readonly actions: StudioActions;
  /** The selected step's editor. */
  readonly editor: StepEditor;
  /** Variables and run inputs. */
  readonly variables: StudioVariables;
  /** The Run tab. */
  readonly run: RunTimeline;
  /** The main process. */
  private readonly deps: StudioDeps;
  /** The last command in flight. */
  private queue: Promise<unknown> = Promise.resolve();

  /** Follows the workspace, the connection and the panel's width, and asks for the workspace. */
  constructor(deps: StudioDeps) {
    super({ ...INITIAL_STATE, connected: deps.shell.state.connected });
    this.deps = deps;
    this.actions = new StudioActions(this, deps);
    this.editor = new StepEditor(this);
    this.variables = new StudioVariables(this);
    this.run = new RunTimeline(this);
    this.follow(deps);
    void this.command({ type: 'get' });
  }

  /** Follows workspace pushes, the shell's connection and the panel's width. */
  private follow({ bridge, shell, panel }: StudioDeps): void {
    this.own(bridge.onWorkspace((snapshot) => void this.receive(snapshot)));
    this.own(shell.subscribe(() => this.set({ connected: shell.state.connected })));
    this.own(panel.subscribe(() => this.set({ panelWidth: panel.state.layout?.panelWidth ?? 0 })));
  }

  /** The snapshot; collaborators call this only once the workspace has answered. */
  get workspace(): WorkspaceSnapshot | undefined {
    return this.state.snapshot;
  }

  /** Sends a workspace command after the ones before it; resolves to the new snapshot, or undefined (its error shown in `slot`). */
  command(source: CommandSource, slot: Slot = 'record-result'): Promise<WorkspaceSnapshot | undefined> {
    const sent = this.queue.then(() => this.send(source, slot));
    this.queue = sent.catch(() => undefined);
    return sent;
  }

  /** Sends one command now and takes in what came back. */
  private async send(source: CommandSource, slot: Slot): Promise<WorkspaceSnapshot | undefined> {
    this.say('', false, slot);
    try {
      const command = typeof source === 'function' ? source() : source;
      return this.receive(await this.deps.bridge.workspace(command));
    } catch (error) {
      this.say(cleanError(error), true, slot);
      return undefined;
    }
  }

  /** Takes in a snapshot (ignored without a draft); the shell learns whether a workflow is recorded. */
  private receive(value: unknown): WorkspaceSnapshot | undefined {
    const next = asSnapshot(value);
    if (!next) return undefined;
    this.set({ ...this.forgetOtherDraft(next), ...this.selection(next), ...this.fields(next), ...this.inputs(next) });
    this.deps.shell.setRecording(next.draft.phase === 'recording');
    return next;
  }

  /** The new snapshot, with the run inputs it asks for. */
  private inputs(next: WorkspaceSnapshot): Partial<StudioState> {
    return { runInputs: runInputsFor(next.draft, this.state.runInputs, this.state.snapshot?.draft), snapshot: next };
  }

  /** Another draft came on screen: its messages and the "just recorded" note belong to the old one. */
  private forgetOtherDraft(next: WorkspaceSnapshot): Partial<StudioState> {
    const shown = this.state.snapshot?.draft.id;
    if (shown === next.draft.id) return {};
    return shown === undefined ? { justFinished: false } : { justFinished: false, messages: NO_MESSAGES };
  }

  /** While recording, the newest step is in focus and the list follows it; otherwise the person's pick stays. */
  private selection(next: WorkspaceSnapshot): Partial<StudioState> {
    const steps = next.draft.steps;
    const recording = next.draft.phase === 'recording';
    const before = this.state.snapshot?.draft.steps.length ?? steps.length;
    const followSteps = recording && steps.length > before;
    if (recording) return { selected: steps.at(-1)?.id, followSteps };
    const kept = steps.some((s) => s.id === this.state.selected);
    return { selected: kept ? this.state.selected : steps[0]?.id, followSteps };
  }

  /** The name and description, left alone while the person is typing in them. */
  private fields(next: WorkspaceSnapshot): Partial<StudioState> {
    const { editing } = this.state;
    const name = editing === 'name' ? {} : { name: draftName(next.draft) };
    return editing === 'description' ? name : { ...name, description: next.draft.description };
  }

  /** Shows a message in `slot`, under the action it is about ('' clears it). */
  say(text = '', error = false, slot: Slot = 'record-result'): void {
    this.set({ messages: { ...this.state.messages, [slot]: { text, error } } });
  }

  /** Changes the flags the collaborators own (busy, saving, the finish card, the selection, the run inputs and speed). */
  mark(flags: StudioFlags): void {
    this.set(flags);
  }

  /** Shows one studio tab. */
  selectTab(tab: StudioTab): void {
    this.set({ tab });
  }

  /** Left and right arrows move between the studio tabs; answers the tab now shown, or undefined for another key. */
  tabKey(key: string): StudioTab | undefined {
    if (key !== 'ArrowLeft' && key !== 'ArrowRight') return undefined;
    const step = key === 'ArrowRight' ? 1 : STUDIO_TABS.length - 1;
    this.selectTab(STUDIO_TABS[(STUDIO_TABS.indexOf(this.state.tab) + step) % STUDIO_TABS.length]);
    return this.state.tab;
  }

  /** Selects a step. */
  select(id: string): void {
    this.set({ selected: id });
  }

  /** The person started typing in the name or description. */
  focusField(field: 'name' | 'description'): void {
    this.set({ editing: field });
  }

  /** The name or description as typed (the save button and hint follow the name). */
  typeField(field: 'name' | 'description', value: string): void {
    this.set(field === 'name' ? { name: value } : { description: value });
  }

  /** The person left the field: a changed name or description is sent. */
  blurField(): void {
    const draft = this.workspace?.draft;
    const changed = draft && (this.state.name !== draftName(draft) || this.state.description !== draft.description);
    this.set({ editing: undefined });
    if (changed) this.actions.metadata();
  }

  /** The save card came into view. */
  finishRevealed(): void {
    this.set({ revealFinish: false });
  }

  /** The steps list followed the newest step. */
  followed(): void {
    this.set({ followSteps: false });
  }
}
