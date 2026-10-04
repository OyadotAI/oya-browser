/**
 * The questions and file pickers the shell's commands open, over the shell
 * window when there is one (Electron refuses a null parent).
 */
import type {
  MessageBoxOptions,
  MessageBoxReturnValue,
  OpenDialogOptions,
  OpenDialogReturnValue,
  SaveDialogOptions,
  SaveDialogReturnValue,
} from 'electron';
import type { AppServices } from '../app/services.ts';

/** The services the dialogs use. */
type Deps = Pick<AppServices, 'electron' | 'shell'>;

/** Electron's dialogs, parented to the shell window. */
export class ShellDialogs {
  /** The main-process services. */
  private readonly deps: Deps;

  /** `deps` gives Electron's dialog and the shell window. */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** Asks where to save a file. */
  save(options: SaveDialogOptions): Promise<SaveDialogReturnValue> {
    const { dialog } = this.deps.electron;
    const window = this.deps.shell.window;
    return window ? dialog.showSaveDialog(window, options) : dialog.showSaveDialog(options);
  }

  /** Asks which file to open. */
  open(options: OpenDialogOptions): Promise<OpenDialogReturnValue> {
    const { dialog } = this.deps.electron;
    const window = this.deps.shell.window;
    return window ? dialog.showOpenDialog(window, options) : dialog.showOpenDialog(options);
  }

  /** Asks a question. */
  ask(options: MessageBoxOptions): Promise<MessageBoxReturnValue> {
    const { dialog } = this.deps.electron;
    const window = this.deps.shell.window;
    return window ? dialog.showMessageBox(window, options) : dialog.showMessageBox(options);
  }
}
