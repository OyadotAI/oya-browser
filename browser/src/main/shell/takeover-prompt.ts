/** Ask before a person's input interrupts automation; never replay the blocked input. */
import type { AppServices } from '../app/services.ts';
import type { WebContents, MouseInputEvent, Point, MessageBoxOptions } from 'electron';
import { TAKEOVER_COOLDOWN_MS, TAKEOVER_MOVE_DISTANCE, TAKEOVER_CHOICE } from './constants.ts';

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
/** Deduplicates input-triggered handoff requests and rechecks ownership after confirmation. */
export class TakeoverPrompt {
  /** Window, dialog, and control services. */
  private readonly deps: Deps;
  /** Only one confirmation may be open. */
  private pending = false;
  /** Pointer motion is quiet for a while after declining. */
  private quietUntil = 0;
  /** Last pointer position used to recognize deliberate motion. */
  private point: Point | null = null;
  /** Distance moved over the shield since its last prompt. */
  private distance = 0;
  /** Keep all lifetime state on this collaborator. */
  constructor(deps: Deps) {
    this.deps = deps;
  }
  /** Watch only the Oya shield, never visited pages or agent-generated page input. */
  install(contents: WebContents): void {
    contents.on('before-mouse-event', (_event, mouse) => this.mouse(mouse));
    contents.on('before-input-event', (event, input) => {
      if (input.type !== 'keyDown' || input.control || input.meta || input.alt || input.key === 'Escape') return;
      event.preventDefault();
      void this.request();
    });
  }
  /** A click asks immediately; substantial pointer movement asks with a cooldown. */
  private mouse(mouse: MouseInputEvent): void {
    if (mouse.type === 'mouseDown') return void this.request();
    if (mouse.type !== 'mouseMove') return;
    if (this.point) this.distance += Math.hypot(mouse.x - this.point.x, mouse.y - this.point.y);
    this.point = { x: mouse.x, y: mouse.y };
    if (this.distance < TAKEOVER_MOVE_DISTANCE || Date.now() < this.quietUntil) return;
    this.distance = 0;
    void this.request();
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
    this.distance = 0;
    this.quietUntil = Date.now() + TAKEOVER_COOLDOWN_MS;
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
