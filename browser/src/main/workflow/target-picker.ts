/** Native workflow target picking: intercept selection input before the website receives it. */
import type { WebContents, Event, MouseInputEvent, Input } from 'electron';
import { candidates, type Candidate } from '../../workflow/index.ts';
import { PICKER, PICKER_WORLD_ID } from './constants.ts';
import { PICKER_START, PICKER_STOP } from './picker-scripts.ts';
/** Only browser-owned native execution and input/lifecycle events are needed. */
export interface PickerView {
  /** Surface selected by the user's authorized workflow action. */
  webContents: Pick<
    WebContents,
    'on' | 'off' | 'isDestroyed' | 'executeJavaScriptInIsolatedWorld' | 'getZoomFactor'
  > & {
    /** One picker owns input on a surface at a time. */
    oyaTargetPicking?: boolean;
  };
}
/** A descriptor can be refused without inventing a fragile target. */
function choicesFor(element: Record<string, unknown>): Candidate[] {
  if (element.unsupported)
    throw new Error('This picker supports top-level targets. Enter the frame selector for embedded targets.');
  const choices = candidates(element);
  if (!choices.length) throw new Error('This element has no stable target. Add a test ID or enter a CSS selector.');
  return choices;
}
/** One native selection owns its event listeners and always releases them when it settles. */
class TargetPick {
  /** Native surface, never a debugger session. */
  private readonly view: PickerView;
  /** One result promise, settled by selection or cancellation. */
  private readonly result = Promise.withResolvers<Candidate[]>();
  /** Selection and teardown are single-use. */
  private settled = false;
  /** Suppress additional selections while descriptor evaluation is in flight. */
  private selecting = false;
  /** Avoid accumulating renderer work on high-frequency mouse movement. */
  private hovering = false;
  /** Coalesce movement to the newest point instead of dropping the final hover. */
  private hoverArgs: string | null = null;
  /** A picker cannot monopolize input indefinitely. */
  private timer?: ReturnType<typeof setTimeout>;
  /** Native input callbacks are stable so they can be removed exactly. */
  private readonly mouse = (event: Event, input: MouseInputEvent): void => this.onMouse(event, input);
  /** Escape cancels without reaching the page. */
  private readonly key = (event: Event, input: Input): void => {
    event.preventDefault();
    if (input.key === 'Escape') this.finish(new Error('Target selection canceled'));
  };
  /** A navigation invalidates the document being inspected. */
  private readonly navigated = (): void => this.finish(new Error('The page navigated while picking a target'));
  /** Destruction must not leave the caller waiting until timeout. */
  private readonly destroyed = (): void => this.finish(new Error('The page closed while picking a target'));
  /** The selected view is supplied by the existing authorized workflow handler. */
  constructor(view: PickerView) {
    this.view = view;
  }
  /** Install input interception before awaiting the renderer overlay. */
  start(): Promise<Candidate[]> {
    this.wire();
    this.timer = setTimeout(() => this.finish(new Error('Target selection timed out')), PICKER.TIMEOUT_MS);
    void this.run(PICKER_START).catch((error) => this.finish(error));
    return this.result.promise;
  }
  /** Native event listeners prevent clicks from activating the underlying website. */
  private wire(): void {
    const wc = this.view.webContents;
    wc.on('before-mouse-event', this.mouse);
    wc.on('before-input-event', this.key);
    wc.on('did-start-loading', this.navigated);
    wc.on('destroyed', this.destroyed);
  }
  /** Release every interception path, including Escape and closed-page failures. */
  private unwire(): void {
    const wc = this.view.webContents;
    wc.off('before-mouse-event', this.mouse);
    wc.off('before-input-event', this.key);
    wc.off('did-start-loading', this.navigated);
    wc.off('destroyed', this.destroyed);
  }
  /** Prevent both halves of selection; only left-button release selects a target. */
  private onMouse(event: Event, input: MouseInputEvent): void {
    event.preventDefault();
    if (this.settled || this.selecting || !Number.isFinite(input.x) || !Number.isFinite(input.y)) return;
    const zoom = this.view.webContents.getZoomFactor();
    const args = `${input.x / zoom},${input.y / zoom}`;
    if (input.type === 'mouseMove') this.hover(args);
    if (input.type === 'mouseUp' && input.button === 'left') void this.select(args);
  }
  /** Keep the latest point while at most one hover evaluation is in flight. */
  private hover(args: string): void {
    this.hoverArgs = args;
    if (!this.hovering) this.flushHover();
  }
  /** A stationary pointer still receives the last coalesced highlight. */
  private flushHover(): void {
    if (this.settled || !this.hoverArgs) return;
    const args = this.hoverArgs;
    this.hoverArgs = null;
    this.hovering = true;
    void this.run(`globalThis.__oyaPicker?.hover(${args});`)
      .catch((error) => this.finish(error))
      .finally(() => this.hovered());
  }
  /** Continue only when a newer pointer point arrived during evaluation. */
  private hovered(): void {
    this.hovering = false;
    if (this.hoverArgs) this.flushHover();
  }
  /** Describe through the isolated world, never a remote object handle. */
  private async select(args: string): Promise<void> {
    this.selecting = true;
    try {
      const element = (await this.run(`globalThis.__oyaPicker?.pick(${args})`)) as Record<string, unknown> | undefined;
      if (!element) throw new Error('The picker document is no longer available');
      this.finish(null, choicesFor(element));
    } catch (error) {
      this.finish(error as Error);
    }
  }
  /** Teardown is queued after prior native evaluations; no late hover can reinstall the overlay. */
  private finish(error: Error | null, value?: Candidate[]): void {
    if (this.settled) return;
    this.settled = true;
    clearTimeout(this.timer);
    this.unwire();
    this.cleanup();
    if (error) this.result.reject(error);
    else this.result.resolve(value ?? []);
  }
  /** Release ownership after the native overlay cleanup has been dispatched. */
  private cleanup(): void {
    void this.run(PICKER_STOP)
      .catch(() => {})
      .finally(() => {
        this.view.webContents.oyaTargetPicking = false;
      });
  }
  /** Refuse dead targets without attempting a debugger fallback. */
  private async run(code: string): Promise<unknown> {
    const wc = this.view.webContents;
    if (wc.isDestroyed()) throw new Error('The page closed while picking a target');
    return wc.executeJavaScriptInIsolatedWorld(PICKER_WORLD_ID, [{ code }]);
  }
}
/** Start one native picker; overlapping requests never steal its input ownership. */
export async function pickTarget(view: PickerView): Promise<Candidate[]> {
  if (view.webContents.oyaTargetPicking) throw new Error('Target selection is already active');
  view.webContents.oyaTargetPicking = true;
  return new TargetPick(view).start();
}
