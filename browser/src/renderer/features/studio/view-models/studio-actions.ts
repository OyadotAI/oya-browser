/**
 * What the studio's buttons do: record (start, resume, stop), the record
 * shortcut, the test run, adding steps and variables, saving to Oya, export
 * and import, diagnostics, Expand, and the draft-level commands. Every action
 * clears and reports in the message slot under its own control.
 */
import { RendererConstants as C } from '../../../core/constants.ts';
import type { RendererServices } from '../../../app/services.ts';
import { SAY, UNTITLED, type Slot } from '../model/constants.ts';
import { cleanError, freeInput, isDisabled, isExpanded } from '../model/studio-model.ts';
import type { StudioViewModel } from './studio-view-model.ts';
import type { ValidateCommand, Variables } from '../model/types.ts';
import type { Failure } from '../../../../shared/ipc.ts';

/**
 * The control bar's guard on the record button. Under agent control the page
 * is watch-only, but Start recording takes control first: recording needs a
 * person's hands on the page. The control feature satisfies this.
 */
export interface RecordingGate {
  /** Whether the record button is refused now (watch-only, and control cannot be taken). */
  blocked(): boolean;
  /** Resolves true once a person may act on the page, taking control first when it is on offer; false when refused. */
  admit(): Promise<boolean>;
}

/** Where Copy code writes. */
export type StudioClipboard = Pick<Clipboard, 'writeText'>;

/** What the actions use besides the studio. */
type ActionDeps = Pick<RendererServices, 'bridge' | 'panel'> & {
  /** The record button's guard. */
  gate: RecordingGate;
  /** The clipboard. */
  clipboard: StudioClipboard;
};

/** A flag whileBusy raises. */
type BusyFlag = 'busy' | 'saving';

/** The studio's actions. */
export class StudioActions {
  /** The studio they act on. */
  private readonly studio: StudioViewModel;
  /** The main process, the panel, the gate and the clipboard. */
  private readonly deps: ActionDeps;

  /** Acts on `studio` through `deps`. */
  constructor(studio: StudioViewModel, deps: ActionDeps) {
    this.studio = studio;
    this.deps = deps;
  }

  /** The record button: through the control gate, then starts, resumes or stops recording and refreshes. */
  async toggleRecording(): Promise<void> {
    if (this.studio.state.busy || !(await this.deps.gate.admit())) return;
    await this.whileBusy(async () => {
      await this.switchRecording();
      await this.studio.command({ type: 'get' });
    });
  }

  /** The command palette's Record (⌘/Ctrl Alt R): opens the studio on Steps, then presses the record button if it could be pressed. */
  async recordShortcut(): Promise<void> {
    if (isDisabled(this.studio.state, 'record-toggle') || this.deps.gate.blocked()) return;
    await this.deps.panel.open('record');
    this.studio.selectTab('steps');
    await this.toggleRecording();
  }

  /** Runs `work` with `flag` raised, reporting a failure in `slot`. */
  private async whileBusy(work: () => Promise<void>, slot: Slot = 'record-result', flag: BusyFlag = 'busy') {
    this.studio.mark({ [flag]: true });
    this.studio.say('', false, slot);
    await work().catch((error: unknown) => this.studio.say(cleanError(error), true, slot));
    this.studio.mark({ [flag]: false });
  }

  /** Stops a recording, resumes a draft that has steps, or starts a new one. */
  private async switchRecording(): Promise<void> {
    const draft = this.studio.workspace?.draft;
    if (draft?.phase === 'recording') return this.stopRecording();
    if (!draft?.steps.length) return void (await this.deps.bridge.startRecording());
    const resumed = await this.studio.command({ type: 'resume-recording' });
    if (!resumed) throw new Error(this.studio.state.messages['record-result'].text || SAY.noResume);
  }

  /** Stops recording; the save card comes into view once it is drawn. */
  async stopRecording(): Promise<void> {
    await this.deps.bridge.stopRecording();
    this.studio.mark({ justFinished: true, revealFinish: true });
    this.studio.selectTab('steps');
  }

  /** Starts a test run with the run inputs' values (secret ones then cleared); shows the Run tab only if a run started. */
  async validate(extra: Pick<ValidateCommand, 'runTo'> = {}): Promise<void> {
    const { runInputs, runSpeed } = this.studio.state;
    const before = this.studio.workspace?.run?.id;
    const next = await this.studio.command({ type: 'validate', vars: runInputs, slowMo: runSpeed, ...extra });
    this.studio.mark({ runInputs: this.withoutSecrets(this.studio.state.runInputs) });
    if (next?.run && next.run.id !== before) this.studio.selectTab('run');
  }

  /** The run inputs with every secret one emptied. */
  private withoutSecrets(inputs: Record<string, string>): Record<string, string> {
    const variables = this.studio.workspace?.draft.variables ?? {};
    return Object.fromEntries(Object.entries(inputs).map(([k, v]) => [k, variables[k]?.secret ? '' : v]));
  }

  /** Adds a step of `action` after the selected one, then selects it; a refused add keeps the selection. */
  async addStep(action: string): Promise<void> {
    if (!action) return;
    const step = { action, candidates: [], expected: '' };
    const next = await this.studio.command({ type: 'add', id: this.studio.state.selected, step });
    if (next) this.studio.mark({ selected: this.added() });
  }

  /** The step just added: the one after the selection, or the last. */
  private added(): string | undefined {
    const { selected } = this.studio.state;
    const steps = this.studio.workspace?.draft.steps ?? [];
    return steps.find((_step, i) => i > 0 && steps[i - 1].id === selected)?.id || steps.at(-1)?.id;
  }

  /** Adds the next free input_n variable and says how to use it. */
  async addVariable(): Promise<void> {
    if (isDisabled(this.studio.state, 'variable-add')) return;
    let name = '';
    const add = (variables: Variables): Variables => ({
      ...variables,
      [(name = freeInput(variables))]: { default: '' },
    });
    const next = await this.studio.command(() => ({ type: 'variables', variables: add(this.variablesNow()) }));
    if (next) this.studio.say(`Use {{${name}}} in a step.`);
  }

  /** The variables as they are now. */
  private variablesNow(): Variables {
    return this.studio.workspace?.draft.variables ?? {};
  }

  /** Saves the playbook to the connected Oya workspace, under the name typed. */
  async save(): Promise<void> {
    if (this.studio.state.saving || isDisabled(this.studio.state, 'record-save')) return;
    const name = this.studio.state.name.trim();
    const description = this.studio.state.description || name;
    await this.whileBusy(() => this.publish(name, description), 'save-result', 'saving');
  }

  /** The save itself, its confirmation, and the refreshed state. */
  private async publish(name: string, description: string): Promise<void> {
    const result = (await this.deps.bridge.saveRecording(name, description)) as Failure | undefined;
    if (!result || result.error) throw new Error(result?.error || SAY.notConfirmed);
    this.studio.mark({ justFinished: false });
    await this.studio.command({ type: 'get' }, 'save-result');
    this.studio.say(`Saved “${name}” to Oya.`, false, 'save-result');
  }

  /** Sends the name and description. */
  metadata(): void {
    const { name, description } = this.studio.state;
    void this.studio.command({ type: 'metadata', name: name || UNTITLED, description });
  }

  /** Copies the generated module. */
  async copy(): Promise<void> {
    await this.report(async () => (await this.deps.clipboard.writeText(this.studio.workspace?.code ?? ''), SAY.copied));
  }

  /** Saves the generated module to a file. */
  async download(): Promise<void> {
    const { name = '', code = '' } = { name: this.studio.workspace?.draft.name, code: this.studio.workspace?.code };
    await this.report(async () =>
      (await this.deps.bridge.exportPlaywright({ name, code })).saved ? SAY.exported : '',
    );
  }

  /** Runs `work` and says what it answers under the code, or its error. */
  private async report(work: () => Promise<string>): Promise<void> {
    try {
      this.studio.say(await work(), false, 'code-result');
    } catch (error) {
      this.studio.say(cleanError(error), true, 'code-result');
    }
  }

  /** Saves the workflow as JSON, Oya's own or Chrome Recorder's, and says so. */
  async exportJson(format: 'oya' | 'chrome'): Promise<void> {
    const next = await this.studio.command({ type: 'export-json', format }, 'code-result');
    if (next?.exported) this.studio.say(format === 'chrome' ? SAY.chrome : SAY.json, false, 'code-result');
  }

  /** Opens a workflow file, from Oya or Chrome's Recorder, as a new draft on the Steps tab. */
  async importJson(): Promise<void> {
    const before = this.studio.workspace?.draft.id;
    const next = await this.studio.command({ type: 'import-json' }, 'code-result');
    if (next && next.draft.id !== before) this.studio.selectTab('steps');
  }

  /** Saves the diagnostics report and says so. */
  async support(): Promise<void> {
    const next = await this.studio.command({ type: 'support' }, 'code-result');
    if (next?.supportSaved) this.studio.say(SAY.diagnostics, false, 'code-result');
  }

  /** Expands or compacts the panel, from its real width. */
  async expand(): Promise<void> {
    const wide = isExpanded(this.studio.state.panelWidth);
    await this.deps.bridge.resizeDevPanel(wide ? C.PANEL_COMPACT_WIDTH : C.PANEL_MAX_WIDTH);
  }

  /** A fresh empty draft. */
  newDraft(): Promise<unknown> {
    return this.studio.command({ type: 'new' });
  }

  /** Opens a stored draft. */
  openDraft(id: string): Promise<unknown> {
    return this.studio.command({ type: 'open', id });
  }

  /** Undoes or redoes the last edit. */
  travel(type: 'undo' | 'redo'): Promise<unknown> {
    return this.studio.command({ type });
  }
}
