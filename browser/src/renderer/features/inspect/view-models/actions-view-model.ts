/**
 * The Actions pane: run a single browser command by hand (analyze, click,
 * type, …) and see its labelled result. Inputs are checked before anything is
 * sent, one action runs at a time, actions that touch the page wait for a
 * person to hold control, Enter runs a field's action, and an analysis lists
 * the page's elements so their numbers can be picked instead of guessed.
 */
import { ViewModel } from '../../../core/view-model.ts';
import { RendererConstants as C } from '../../../core/constants.ts';
import type { ControlState, Payload } from '../../../../shared/ipc.ts';
import { showResult, problemWith, type ActionResult, type ElementItem } from '../model/action-result.ts';
import {
  ACTION_FIELDS,
  ACTION_PARAMS,
  ACTIONS_TEXT,
  COPY_TEXT,
  FIELD_ACTION,
  UNGUARDED,
  type ActionField,
  type ActionFields,
} from '../model/constants.ts';
import type { InspectServices } from '../model/services.ts';

/** What the Actions pane shows. */
export interface ActionsState {
  /** What each field holds. */
  fields: ActionFields;
  /** The action in flight, or null. */
  running: string | null;
  /** A person holds control, so page actions may run. */
  interactive: boolean;
  /** The last result, or null. */
  result: ActionResult | null;
  /** The analyzed page's elements to pick from. */
  elements: readonly ElementItem[];
  /** The line above the elements. */
  hint: string;
  /** What the result's Copy button says. */
  copyLabel: string;
}

/** What an action's button looks like. */
export interface ActionLook {
  /** Off while another action runs, or while the agent holds the page and it touches the page. */
  disabled: boolean;
  /** Why it is off for control, or ''. */
  title: string;
}

/** How the button for `action` looks in `state`. */
export function actionLook(state: ActionsState, action: string): ActionLook {
  const held = !UNGUARDED.includes(action) && !state.interactive;
  return { disabled: !!state.running || held, title: held ? ACTIONS_TEXT.takeControl : '' };
}

/** Every field empty. */
const emptyFields = (): ActionFields =>
  Object.fromEntries(ACTION_FIELDS.map((id) => [id, ''])) as Record<ActionField, string>;

/** Nothing run yet, with a person in control until told otherwise. */
const initialState = (): ActionsState => ({
  fields: emptyFields(),
  running: null,
  interactive: true,
  result: null,
  elements: [],
  hint: ACTIONS_TEXT.hintStart,
  copyLabel: COPY_TEXT.idle,
});

/** The Actions pane. */
export class ActionsViewModel extends ViewModel<ActionsState> {
  /** The main process, the panel and the clipboard. */
  private readonly services: Pick<InspectServices, 'bridge' | 'clipboard'>;
  /** Everything the last result holds, for Copy. */
  private copyText = '';
  /** The timer that puts Copy back. */
  private copyTimer: ReturnType<typeof setTimeout> | undefined;

  /** Empty, following who holds control; registered as the pane's Clear. */
  constructor(services: Pick<InspectServices, 'bridge' | 'clipboard' | 'panel'>) {
    super(initialState());
    this.services = services;
    this.watchControl();
    this.own(services.panel.onClear('actions', () => this.clear()));
    this.own(() => clearTimeout(this.copyTimer));
  }

  /** Follows who holds control: page actions wait for a person. */
  private watchControl(): void {
    const { bridge } = this.services;
    this.own(bridge.onControlState((state) => this.controlChanged(state)));
    bridge.getControlState().then(
      (state) => this.controlChanged(state),
      () => {},
    );
  }

  /** Control changed hands. */
  controlChanged(state: ControlState | null | undefined): void {
    this.set({ interactive: !!state?.interactive });
  }

  /** A field was typed in. */
  setField(id: ActionField, value: string): void {
    this.set({ fields: { ...this.state.fields, [id]: value } });
  }

  /** Puts an element's number in the element field. */
  pickElement(id: string): void {
    this.setField('action-click-id', id);
  }

  /** Enter in a field: runs that field's action. */
  enter(id: ActionField): Promise<void> {
    return this.run(FIELD_ACTION[id]);
  }

  /** Runs `action` unless it is off or its input is wrong, then shows the result. */
  async run(action: string): Promise<void> {
    if (actionLook(this.state, action).disabled) return;
    const params = Object.hasOwn(ACTION_PARAMS, action) ? ACTION_PARAMS[action](this.state.fields) : {};
    const problem = problemWith(params);
    if (problem) return this.show(action, { ok: false, error: problem });
    this.set({ running: action });
    const answer = await this.perform(action, params);
    this.set({ running: null });
    this.show(action, answer);
  }

  /** Asks the main process to run the action; a failed call answers as a failure. */
  private perform(action: string, params: Record<string, string>): Promise<Payload> {
    return this.services.bridge.devAction(action, params).catch((e: Error) => ({ ok: false, error: e.message }));
  }

  /** Shows an answer, with the elements an analysis found. */
  private show(action: string, answer: Parameters<typeof showResult>[1]): void {
    const shown = showResult(action, answer, new Date());
    this.copyText = shown.copyText;
    this.set({ result: shown.result });
    if (!shown.elements) return;
    const hint = shown.elements.length ? ACTIONS_TEXT.hintPick : ACTIONS_TEXT.hintNone;
    this.set({ elements: shown.elements, hint });
  }

  /** Copies the whole last result, and says so on the button for a moment. */
  async copy(): Promise<void> {
    const copied = await this.services.clipboard.writeText(this.copyText).then(
      () => true,
      () => false,
    );
    this.set({ copyLabel: copied ? COPY_TEXT.copied : COPY_TEXT.failed });
    clearTimeout(this.copyTimer);
    this.copyTimer = setTimeout(() => this.set({ copyLabel: COPY_TEXT.idle }), C.COPIED_MS);
  }

  /** Empties the result and the element list. */
  clear(): void {
    this.copyText = '';
    this.set({ result: null, elements: [] });
  }
}
