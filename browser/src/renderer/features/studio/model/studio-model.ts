/**
 * The studio's state and what follows from it, as pure functions: what the
 * studio may do now (its mode), every control's disabled rule from one
 * command map, the save card's words, the workflow picker, the code lines and
 * the variables the steps use. Views and tests read these; nothing here
 * changes anything.
 */
import { RendererConstants as C } from '../../../core/constants.ts';
import {
  ACTION_NAMES,
  ACTIVE_STATUSES,
  INPUT_PREFIX,
  IPC_ERROR_PREFIX,
  NAME_RULE,
  PLACEHOLDER,
  RECORD_SHORTCUT,
  SAY,
  UNTITLED,
  type Slot,
  type Stage,
  type StudioTab,
} from './constants.ts';
import type { Draft, LibraryItem, Run, Step, Variables, WorkspaceSnapshot } from './types.ts';

/** A message under an action. */
export interface Message {
  /** What it says ('' for nothing). */
  text: string;
  /** Whether it is an error. */
  error: boolean;
}

/** The studio's whole state. */
export interface StudioState {
  /** The last workspace snapshot, once the workspace has answered. */
  snapshot: WorkspaceSnapshot | undefined;
  /** The studio tab in view. */
  tab: StudioTab;
  /** The selected step's id. */
  selected: string | undefined;
  /** A recording start or stop is in flight. */
  busy: boolean;
  /** A save to Oya is in flight. */
  saving: boolean;
  /** The recording just finished in this session, so the save card says so. */
  justFinished: boolean;
  /** Bring the save card into view on the next draw. */
  revealFinish: boolean;
  /** A recording just added a step, so the list follows it. */
  followSteps: boolean;
  /** The message in each slot. */
  messages: Record<Slot, Message>;
  /** The playbook name as typed. */
  name: string;
  /** The description as typed. */
  description: string;
  /** The field being typed in, which a snapshot leaves alone. */
  editing: 'name' | 'description' | undefined;
  /** What is typed into each run input, by variable. */
  runInputs: Record<string, string>;
  /** Milliseconds of pause per step in a test run. */
  runSpeed: number;
  /** Connected to the server (from the shell). */
  connected: boolean;
  /** The panel's width, for Expand or Compact. */
  panelWidth: number;
}

/** What the studio may do now. */
export interface StudioMode {
  /** A workflow is being recorded. */
  recording: boolean;
  /** A test run is going. */
  running: boolean;
  /** The draft may not be edited. */
  locked: boolean;
  /** A test run may start. */
  runnable: boolean;
  /** The stage the stylesheet shows. */
  stage: Stage;
  /** The draft can be stored. */
  stored: boolean;
}

/** No message. */
const SILENT: Message = { text: '', error: false };

/** Every slot empty. */
export const NO_MESSAGES: Record<Slot, Message> = {
  'record-result': SILENT,
  'save-result': SILENT,
  'code-result': SILENT,
};

/** Before the workspace answers: nothing to edit or run. */
const WAITING: StudioMode = {
  recording: false,
  running: false,
  locked: true,
  runnable: false,
  stage: 'empty',
  stored: true,
};

/** The studio before the workspace has answered. */
export const INITIAL_STATE: StudioState = {
  ...{ snapshot: undefined, tab: 'steps', selected: undefined, busy: false, saving: false },
  ...{ justFinished: false, revealFinish: false, followSteps: false, messages: NO_MESSAGES },
  ...{ name: '', description: '', editing: undefined, runInputs: {}, runSpeed: 0, connected: false, panelWidth: 0 },
};

/** An action's display name (the raw action when it has none). */
export const actionName = (action: string): string =>
  (Object.hasOwn(ACTION_NAMES, action) && ACTION_NAMES[action]) || action;

/** Whether a run is still going. */
export const isActive = (run: Run | null | undefined): boolean => ACTIVE_STATUSES.includes(run?.status ?? '');

/** "1 step", "3 steps". */
export const plural = (count: number, word: string): string => `${count} ${word}${count === 1 ? '' : 's'}`;

/** An error's message without Electron's IPC wrapper. */
export const cleanError = (error: unknown): string =>
  String(error instanceof Error ? error.message : error).replace(IPC_ERROR_PREFIX, '');

/** Whether `name` can be saved to Oya. */
export const validName = (name: string): boolean => NAME_RULE.test(name.trim());

/** The stage from the draft and run. */
function stageOf(recording: boolean, running: boolean, draft: Draft): Stage {
  if (recording) return 'recording';
  if (running) return 'running';
  return draft.steps.length ? 'captured' : 'empty';
}

/** What the studio may do now. */
export function studioMode(state: StudioState): StudioMode {
  const s = state.snapshot;
  if (!s) return WAITING;
  const recording = s.draft.phase === 'recording';
  const running = isActive(s.run);
  const locked = recording || running || state.busy || state.saving;
  const runnable = !locked && s.draft.steps.some((step) => step.enabled) && !s.issues.length;
  return { recording, running, locked, runnable, stage: stageOf(recording, running, s.draft), stored: !s.storageError };
}

/** A control's disabled rule, given the mode, the state and the snapshot. */
type Rule = (m: StudioMode, state: StudioState, s: WorkspaceSnapshot) => boolean;

/** Each control's disabled rule, by element id. */
const DISABLED = {
  'record-toggle': (m, state) => state.busy || state.saving || m.running,
  'record-validate': (m) => !m.runnable,
  'record-clear': (m) => m.locked || !m.stored,
  'draft-library': (m) => m.locked || !m.stored,
  'record-save': (m, state, s) => !m.runnable || !state.connected || s.saved || !validName(state.name),
  'record-name': (m) => m.locked,
  'record-desc': (m) => m.locked,
  'step-undo': (m, _state, s) => m.locked || !s.canUndo,
  'step-redo': (m, _state, s) => m.locked || !s.canRedo,
  'step-add': (m) => m.locked,
  'variable-add': (m) => m.locked,
  'run-history': (m) => m.locked,
  'record-copy': (_m, _state, s) => !s.code,
  'record-download': (_m, _state, s) => !s.code,
  'record-json': (_m, _state, s) => !s.draft.steps.length,
  'record-chrome': (_m, _state, s) => !s.draft.steps.length,
  'record-import': (m) => m.locked,
  'record-open': (m) => m.locked,
} satisfies Record<string, Rule>;

/** A control the disabled map covers. */
export type ControlId = keyof typeof DISABLED;

/** Whether control `id` is off now; before the workspace answers, everything but the record button is. */
export function isDisabled(state: StudioState, id: ControlId): boolean {
  const s = state.snapshot;
  if (!s) return id !== 'record-toggle';
  const rule: Rule = DISABLED[id];
  return rule(studioMode(state), state, s);
}

/** The save card's title: saved, just recorded, or ready to save. */
export function finishTitle(state: StudioState): string {
  if (state.snapshot?.saved) return 'Saved to Oya';
  return state.justFinished ? 'Recording captured' : 'Save to Oya';
}

/** What the save card says about where the workflow is. */
export function finishCopy(state: StudioState, draft: Draft): string {
  if (state.snapshot?.storageError) return 'This workflow is only in memory. Save or export it before closing Oya.';
  if (state.snapshot?.saved) return 'Your local draft is kept too. Edits need saving again.';
  const steps = plural(draft.steps.length, 'step');
  if (!state.justFinished) return `${steps} on this device.`;
  // A playbook already in Oya has its name; it only needs the changes saved.
  const next = draft.publishedAt ? 'Save the changes to update it in Oya.' : 'Name it to reuse it in Oya.';
  return `${steps} saved on this device. ${next}`;
}

/** The save button's label. */
export function saveLabel(state: StudioState, draft: Draft): string {
  if (state.saving) return 'Saving…';
  if (state.snapshot?.saved) return 'Saved';
  return draft.publishedAt ? 'Save changes' : 'Save playbook';
}

/** What stands between this draft and saving it. */
export function saveHint(state: StudioState): string {
  if (!state.connected) return SAY.offlineHint;
  if (state.snapshot?.issues.length) return SAY.issuesHint;
  const name = state.name.trim();
  return name && !validName(name) ? SAY.nameHint : '';
}

/** The save button's tooltip: publishing needs a connection, local work does not. */
export const saveTitle = (connected: boolean): string => (connected ? SAY.saveOnline : SAY.saveOffline);

/** The name field's text for a draft: empty until it is named. */
export const draftName = (draft: Draft): string => (draft.name === UNTITLED ? '' : draft.name);

/**
 * The library as the picker shows it: the draft being edited always (first when
 * it is not stored yet, so the list is never blank), and no other empty drafts,
 * which older versions left behind as extra "Untitled workflow" entries.
 */
export function withCurrent(library: LibraryItem[], draft: Draft): LibraryItem[] {
  const shown = library.filter((item) => item.id === draft.id || item.steps || item.error);
  return shown.some((item) => item.id === draft.id) ? shown : [{ id: draft.id, name: draft.name }, ...shown];
}

/** The step-limit note while recording near or at the limit, or '' when it stays hidden. */
export function stepLimit(count: number, recording: boolean): string {
  const { MAX_STEPS, STEPS_WARNING } = C;
  if (count < STEPS_WARNING || (count < MAX_STEPS && !recording)) return '';
  if (count >= MAX_STEPS)
    return `Recording stopped at the ${MAX_STEPS}-step limit. Save this workflow, then record the rest as a second one.`;
  return `${count} of ${MAX_STEPS} steps. Recording stops at ${MAX_STEPS}: finish this part, save it, and record the rest as a second workflow.`;
}

/** Whether the panel is at (or past) its expanded width. */
export const isExpanded = (panelWidth: number): boolean => panelWidth >= C.PANEL_MAX_WIDTH;

/** The Expand control's name, from the panel's real width. */
export const expandLabel = (panelWidth: number): string =>
  isExpanded(panelWidth) ? 'Compact workspace' : 'Expand workspace';

/** The generated code's lines, or why there is none. */
export const codeLines = (code: string): string[] => (code || SAY.noCode).split('\n');

/** Every variable the steps use, in first-use order. */
export const usedVariables = (steps: Step[]): string[] => [
  ...new Set([...JSON.stringify(steps).matchAll(PLACEHOLDER)].map((m) => m[1])),
];

/** The first input_n name the variables do not use. */
export function freeInput(variables: Variables): string {
  let n = 1;
  while (variables[INPUT_PREFIX + n]) n++;
  return INPUT_PREFIX + n;
}

/** The variables the steps use, with their secret flags: the run inputs are rebuilt only when this changes. */
const inputKey = (draft: Draft): string =>
  JSON.stringify(usedVariables(draft.steps).map((n) => [n, !!draft.variables[n]?.secret]));

/** The run inputs for `draft`: what was typed survives while the inputs stay the same, else each starts at its default. */
export function runInputsFor(draft: Draft, typed: Record<string, string>, before: Draft | undefined) {
  if (before && inputKey(before) === inputKey(draft)) return typed;
  return Object.fromEntries(usedVariables(draft.steps).map((n) => [n, draft.variables[n]?.default ?? '']));
}

/** The record button's tooltip, naming the shortcut: it moved off ⌘⇧R (Chrome's hard reload), so people no longer guess it. */
export const toggleTitle = (platform: string): string =>
  `Start or stop recording (${platform.includes('Mac') ? RECORD_SHORTCUT.mac : RECORD_SHORTCUT.other})`;
