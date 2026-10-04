/** IPC: recording, saving as a playbook, and exporting Playwright code. */
import type { SaveDialogOptions } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { Payload, SaveAnswer } from '../../shared/ipc.ts';
import type { HandlersOf } from './handle.ts';
import { RecordingPublisher } from '../recording/publish.ts';
import { WorkspaceCommands } from './workspace.ts';
import { writePrivateFile } from './files.ts';
import { ShellDialogs } from './shell-dialogs.ts';
import { MAX_EXPORT_CHARS, SAFE_FILE_NAME } from './constants.ts';

/** The services the recording handlers use. */
type Deps = Pick<
  AppServices,
  'workspace' | 'shield' | 'recorder' | 'tabs' | 'electron' | 'shell' | 'control' | 'socket' | 'config'
>;

/** The channels this group answers. */
type Channel = 'workspace' | 'start-recording' | 'stop-recording' | 'save-recording' | 'export-playwright';

/** The save dialog for an export, named after the playbook when its name is safe. */
function playwrightSaveOptions(payload: Payload): SaveDialogOptions {
  const name = typeof payload.name === 'string' && SAFE_FILE_NAME.test(payload.name) ? payload.name : 'playbook';
  const filters = [{ name: 'JavaScript', extensions: ['mjs'] }];
  return { title: 'Save Playwright script', defaultPath: name + '.mjs', filters };
}

/** Recording and its exports, as the shell asks for them. */
export class RecordingHandlers {
  /** Channel → handler. */
  readonly handlers: HandlersOf<Channel> = {
    workspace: (_e, command) => this.workspace.run(command),
    'start-recording': () => {
      this.deps.shield.requireHumanControl();
      return this.deps.recorder.queueRecording(() => this.deps.recorder.startRecording());
    },
    'stop-recording': () => this.deps.recorder.queueRecording(() => this.deps.recorder.stopRecording()),
    'save-recording': (_e, name, description) =>
      this.deps.recorder.queueRecording(() => new RecordingPublisher(this.deps).save(name, description)),
    'export-playwright': (_e, payload) => this.exportPlaywright(payload),
  };
  /** The main-process services. */
  private readonly deps: Deps;
  /** The workflow studio's commands. */
  private readonly workspace: WorkspaceCommands;

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
    this.workspace = new WorkspaceCommands(deps);
  }

  /** Saves Playwright code the shell generated where the person chooses. */
  private async exportPlaywright(payload: Payload | undefined): Promise<SaveAnswer> {
    if (typeof payload?.code !== 'string' || payload.code.length > MAX_EXPORT_CHARS)
      throw new Error('Invalid Playwright export');
    const result = await new ShellDialogs(this.deps).save(playwrightSaveOptions(payload));
    if (result.canceled) return { canceled: true };
    await writePrivateFile(result.filePath, payload.code);
    return { saved: true };
  }
}
