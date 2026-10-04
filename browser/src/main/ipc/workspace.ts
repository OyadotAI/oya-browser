/**
 * IPC: the workflow workspace (drafts, editing, target picking, validation,
 * diagnostics). Command type → handler; any other type is an edit.
 */
import type { AppServices } from '../app/services.ts';
import type { Payload } from '../../shared/ipc.ts';
import { pickTarget } from '../workflow/target-picker.ts';
import { JSON_INDENT } from '../app/constants.ts';
import { writePrivateFileSync } from './files.ts';
import { ShellDialogs } from './shell-dialogs.ts';
import { WorkflowFiles } from './workflow-files.ts';
import { CONFIRM_BUTTON } from './constants.ts';

/** The services the workspace commands use. */
type Deps = Pick<AppServices, 'workspace' | 'shield' | 'recorder' | 'tabs' | 'electron' | 'shell' | 'control'>;

/** The workspace, once it exists. */
type Studio = NonNullable<AppServices['workspace']>;

/** One workspace command's handler. */
type Command = (command: Payload) => Payload | Promise<Payload>;

/** The question asked before a workflow runs against the real site. */
const VALIDATE_PROMPT = {
  type: 'question' as const,
  title: 'Validate workflow',
  message: 'Run on the real website?',
  detail:
    'Oya opens a fresh tab using your current login. This can submit forms, send messages, upload files, or change data. Steps run exactly as shown in the exported Playwright module.',
  buttons: ['Cancel', 'Run workflow'],
  defaultId: 0,
  cancelId: 0,
};

/** Where the diagnostics report is offered to be saved. */
const SUPPORT_SAVE_OPTIONS = {
  defaultPath: 'oya-diagnostics.json',
  filters: [{ name: 'JSON diagnostics', extensions: ['json'] }],
};

/** The workflow studio's commands, as the shell sends them on the `workspace` channel. */
export class WorkspaceCommands {
  /** Command type → handler. */
  readonly commands: Readonly<Record<string, Command>> = {
    get: () => this.studio.snapshot(),
    'resume-recording': () => this.resumeRecording(),
    pick: (command) => this.pickTarget(command),
    validate: (command) => this.validate(command),
    control: (command) => this.controlRun(command),
    support: () => this.saveSupportReport(),
    'export-json': (command) => this.files.exportJson(command),
    'import-json': () => this.files.importJson((command) => this.editDraft(command)),
  };
  /** The main-process services. */
  private readonly deps: Deps;
  /** Workflow files, saved and opened. */
  private readonly files: WorkflowFiles;
  /** The questions and file pickers. */
  private readonly dialogs: ShellDialogs;

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
    this.files = new WorkflowFiles(deps);
    this.dialogs = new ShellDialogs(deps);
  }

  /** The `workspace` channel: a known command, or an edit. */
  async run(command: Payload = {}): Promise<Payload> {
    if (!this.deps.workspace) throw new Error('Workspace is starting');
    const type = command.type;
    if (typeof type === 'string' && Object.hasOwn(this.commands, type)) return this.commands[type](command);
    return this.editDraft(command);
  }

  /** The workspace, which run() has checked exists. */
  private get studio(): Studio {
    return this.deps.workspace as Studio;
  }

  /** Resumes a paused recording. */
  private async resumeRecording(): Promise<Payload> {
    this.deps.shield.requireHumanControl();
    await this.deps.recorder.queueRecording(() => this.deps.recorder.startRecording(true));
    return this.studio.snapshot();
  }

  /** The page to pick on, once nothing else is using it. */
  private pickableView(): NonNullable<ReturnType<Deps['tabs']['getActiveView']>> {
    this.deps.shield.requireHumanControl();
    if (this.studio.busy() || this.deps.recorder.recording) {
      throw new Error('Pause recording and stop validation before picking a target');
    }
    const view = this.deps.tabs.getActiveView();
    if (!view) throw new Error('Open a page first');
    return view;
  }

  /** Picks a target on the page for one step. */
  private async pickTarget(command: Payload): Promise<Payload> {
    const view = this.pickableView();
    const draftId = this.studio.draft.id;
    const candidates = await pickTarget(view);
    if (this.studio.draft.id !== draftId) throw new Error('Draft changed during target selection');
    return this.applyTarget(command, candidates);
  }

  /** Saves the picked candidates on the step. */
  private applyTarget(command: Payload, candidates: unknown): Payload {
    const workspace = this.studio;
    const id = typeof command.id === 'string' ? command.id : undefined;
    const state = workspace.edit({ type: 'update', id, patch: { candidates, captureIssue: undefined } });
    this.deps.recorder.recordedSteps = structuredClone(workspace.draft.steps);
    return state;
  }

  /** Runs the draft against the real site, once the person confirms. */
  private async validate(command: Payload): Promise<Payload> {
    const answer = await this.dialogs.ask(VALIDATE_PROMPT);
    if (answer.response !== CONFIRM_BUTTON) return this.studio.snapshot();
    return this.studio.start(command);
  }

  /** Resumes, steps or stops a validation; resuming hands control back to the run. */
  private async controlRun(command: Payload): Promise<Payload> {
    const state = this.deps.control.snapshot();
    const resuming = ['resume', 'step'].includes(String(command.command));
    if (resuming && state.mode === 'human' && state.mine) await this.deps.control.change('return');
    return this.studio.control(String(command.command));
  }

  /** Saves a diagnostics report where the person chooses; the snapshot says whether it was saved. */
  private async saveSupportReport(): Promise<Payload> {
    const report = this.studio.support();
    const result = await this.dialogs.save(SUPPORT_SAVE_OPTIONS);
    const supportSaved = !result.canceled && !!result.filePath;
    if (supportSaved) writePrivateFileSync(result.filePath, JSON.stringify(report, null, JSON_INDENT));
    return { ...this.studio.snapshot(), supportSaved };
  }

  /** An edit to the draft; the recording follows it. */
  private editDraft(command: Payload): Payload {
    const workspace = this.studio;
    const state = workspace.edit({ ...command, type: String(command.type) });
    this.deps.recorder.adopt(structuredClone(workspace.draft.steps), workspace.draft.secrets);
    return state;
  }
}
