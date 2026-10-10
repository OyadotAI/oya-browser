/** Confirm only explicit header takeover requests; passive page input never interrupts automation. */
import type { AppServices } from '../app/services.ts';
import type { WebContents, MessageBoxOptions } from 'electron';
import { TAKEOVER_CHOICE } from './constants.ts';

/** Default keeps automation running until takeover is explicitly chosen. */
const TAKEOVER_OPTIONS: MessageBoxOptions = {
  type: 'question',
  message: 'Take control of this page?',
  detail: 'Oya is working here. Take control to interact, then use Return to agent when you are finished.',
  buttons: ['Keep agent working', 'Take control'],
  defaultId: 0,
  cancelId: 0,
  noLink: true,
};

/** Main-process services used for a native, window-parented confirmation. */
type Deps = Pick<AppServices, 'electron' | 'shell' | 'control'>;
/** Deduplicates explicit handoff requests and rechecks ownership after confirmation. */
export class TakeoverPrompt {
  /** Window, dialog, and control services. */
  private readonly deps: Deps;
  /** Only one confirmation may be open. */
  private pending = false;
  /** Keep all lifetime state on this collaborator. */
  constructor(deps: Deps) {
    this.deps = deps;
  }
  /** The shield consumes ordinary keys without opening dialogs; shell shortcuts remain available. */
  install(contents: WebContents): void {
    contents.on('before-input-event', (event, input) => {
      if (input.type !== 'keyDown' || input.control || input.meta || input.alt || input.key === 'Escape') return;
      event.preventDefault();
    });
  }
  /** Ask only for agent ownership; never steal a different person's hold. */
  async request(): Promise<void> {
    const state = this.deps.control.snapshot();
    if (this.pending || state.interactive || state.busy || state.mode !== 'agent') return;
    this.pending = true;
    await this.confirm()
      .catch(() => {})
      .finally(() => this.settled());
  }
  /** Reset the invitation after acceptance, dismissal, or a closed window. */
  private settled(): void {
    this.pending = false;
  }
  /** Show handoff errors without losing the current ownership state. */
  private async failed(error: Error): Promise<void> {
    const window = this.deps.shell.window;
    if (!window || window.isDestroyed()) return;
    await this.deps.electron.dialog.showMessageBox(window, {
      type: 'error',
      message: 'Could not take control',
      detail: error.message,
    });
  }
  /** A native modal leaves the automation gate intact until the user explicitly accepts. */
  private async confirm(): Promise<void> {
    const window = this.deps.shell.window;
    if (!window || window.isDestroyed()) return;
    const options = TAKEOVER_OPTIONS;
    const result = await this.deps.electron.dialog.showMessageBox(window, options);
    if (result.response !== TAKEOVER_CHOICE || this.deps.control.snapshot().mode !== 'agent') return;
    await this.deps.control.change('acquire').catch((error: Error) => this.failed(error));
  }
}
