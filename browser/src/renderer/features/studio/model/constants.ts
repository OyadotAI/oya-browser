/**
 * The workflow studio's fixed words and tables: action names, the studio tabs,
 * message slots, run statuses and their titles, the step editor's fields, and
 * the add-step and replay-speed choices. Numbers come from core/constants.ts.
 */

/** The name people see for each action. */
export const ACTION_NAMES: Readonly<Record<string, string>> = {
  navigate: 'Navigate',
  go_back: 'Back',
  go_forward: 'Forward',
  type: 'Fill',
  click: 'Click',
  double_click: 'Double-click',
  hover: 'Hover',
  select_option: 'Select',
  press_key: 'Press key',
  scroll: 'Scroll',
  upload_file: 'Upload',
  wait: 'Wait for element',
  assert_visible: 'Assert visible',
  assert_text: 'Assert text',
  assert_value: 'Assert value',
  assert_url: 'Assert URL',
  assert_page: 'Check page',
  unsupported_frame: 'Not recorded: embedded frame',
  unsupported_drop: 'Not recorded: drag and drop',
  unsupported_click: 'Not recorded: canvas click',
  checkpoint: 'Human checkpoint',
};

/** The actions the Add step picker offers, in its order. */
export const ADDABLE_ACTIONS = [
  'assert_visible',
  'assert_text',
  'assert_value',
  'assert_url',
  'assert_page',
  'wait',
  'checkpoint',
  'navigate',
  'go_back',
  'go_forward',
  'click',
  'double_click',
  'hover',
  'type',
  'select_option',
  'press_key',
  'scroll',
] as const;

/** The studio tabs, in order. */
export const STUDIO_TABS = ['steps', 'run', 'code'] as const;

/** A studio tab. */
export type StudioTab = (typeof STUDIO_TABS)[number];

/** The words on each studio tab. */
export const TAB_LABELS: Readonly<Record<StudioTab, string>> = { steps: 'Steps', run: 'Run', code: 'Code' };

/** Where each message shows: under the action that caused it. */
export const SLOTS = ['record-result', 'save-result', 'code-result'] as const;

/** A message slot's id. */
export type Slot = (typeof SLOTS)[number];

/** Run statuses during which the draft is locked. */
export const ACTIVE_STATUSES: readonly string[] = ['starting', 'running', 'paused', 'stopping'];

/** What a playbook may be called when it is saved to Oya. */
export const NAME_RULE = /^[\w-]{1,64}$/;

/** The longest playbook name (the server's limit, as NAME_RULE says). */
export const NAME_MAX_LENGTH = 64;

/** The name a draft has before the person names it. */
export const UNTITLED = 'Untitled workflow';

/** The record button's label in each stage. */
export const TOGGLE_LABEL = {
  empty: 'Start recording',
  recording: 'Stop recording',
  captured: 'Resume recording',
  running: 'Resume recording',
} as const;

/** The studio's stage, which the stylesheet reads from #pane-record[data-stage]. */
export type Stage = keyof typeof TOGGLE_LABEL;

/** The record shortcut as each platform writes it (the command palette binds it). */
export const RECORD_SHORTCUT = { mac: '⌘⌥R', other: 'Ctrl+Alt+R' } as const;

/** The heading for each run status. */
export const RUN_TITLES: Readonly<Record<string, string>> = {
  succeeded: 'Steps completed.',
  failed: 'Needs your attention.',
  'outcome-unknown': 'Check the website before retrying.',
  paused: 'Paused at a step.',
  running: 'Running…',
  starting: 'Preparing a fresh tab…',
  interrupted: 'Run interrupted.',
  stopping: 'Stopping…',
  stopped: 'Run stopped.',
};

/** Event kinds the run timeline shows. */
export const SHOWN_EVENT_KINDS: readonly string[] = ['step', 'attention', 'target'];

/** The run controls, in order, with their labels and the run statuses each works in. */
export const RUN_CONTROLS = [
  { command: 'pause', label: 'Pause', statuses: ['starting', 'running'] },
  { command: 'resume', label: 'Continue', statuses: ['paused'] },
  { command: 'step', label: 'Step', statuses: ['paused'] },
  { command: 'stop', label: 'Stop', statuses: ['starting', 'running', 'paused'] },
] as const;

/** A run control's command. */
export type RunCommand = (typeof RUN_CONTROLS)[number]['command'];

/** The replay speeds: milliseconds of pause per step, and what the picker says. */
export const RUN_SPEEDS = [
  { value: 0, label: 'Normal' },
  { value: 1000, label: 'Slow (1 s per step)' },
  { value: 3000, label: 'Very slow (3 s per step)' },
] as const;

/** Locator kinds that read as words on the page, in the order a step row prefers them. */
export const READABLE_KINDS: readonly string[] = ['label', 'role', 'text', 'placeholder'];

/** Actions that act on an element, so have a target to edit. */
export const TARGETED_ACTIONS: readonly string[] = [
  'click',
  'double_click',
  'hover',
  'type',
  'select_option',
  'upload_file',
  'wait',
  'assert_visible',
  'assert_text',
  'assert_value',
];

/** Locator strategies, in the order the picker lists them, with the names people see. */
export const STRATEGIES: Readonly<Record<string, string>> = {
  testId: 'Test id',
  role: 'Role',
  label: 'Label',
  text: 'Text',
  placeholder: 'Placeholder',
  css: 'CSS',
};

/** A step's text fields that an action's value field edits. */
export type ValueKey = 'url' | 'text' | 'option' | 'file' | 'key' | 'direction' | 'expected';

/** The field that holds each action's value, and its label. */
export const VALUE_FIELDS: Readonly<Record<string, readonly [ValueKey, string]>> = {
  navigate: ['url', 'URL'],
  type: ['text', 'Text or {{variable}}'],
  select_option: ['option', 'Option'],
  upload_file: ['file', 'File'],
  press_key: ['key', 'Key'],
  scroll: ['direction', 'Direction (up or down)'],
  assert_url: ['expected', 'Expected URL'],
  assert_page: ['expected', 'Page (the query is ignored)'],
  assert_text: ['expected', 'Expected text'],
  assert_value: ['expected', 'Expected value'],
};

/** A `{{variable}}` placeholder. */
export const PLACEHOLDER = /\{\{([A-Za-z_]\w*)\}\}/g;

/** A Fill whose whole text is one variable. */
export const WHOLE_VARIABLE = /^\{\{(\w+)\}\}$/;

/** The prefix of the names Add variable gives. */
export const INPUT_PREFIX = 'input_';

/** The arrow between frame levels in the Frames field. */
export const FRAME_ARROW = '→';

/** Electron's wrapper around an error thrown in the main process. */
export const IPC_ERROR_PREFIX = /^Error invoking remote method '[^']+': (?:Error: )?/;

/** Fixed sentences the studio says. */
export const SAY = {
  noCode: 'Fix the steps marked ! to see the code.',
  notConfirmed: 'The server did not confirm the save',
  noResume: 'Could not resume recording',
  pick: 'Click an element in the page. Escape cancels.',
  copied: 'Code copied.',
  exported: 'Playwright module exported.',
  json: 'Workflow saved as JSON.',
  chrome: 'Saved for Chrome Recorder.',
  diagnostics: 'Diagnostics saved.',
  runIdle: 'Test this workflow',
  runWarning: 'A test run uses your current login and can change real data.',
  noAssertions: 'Actions completed. Add assertions to verify the outcome.',
  saveOnline: 'Save this playbook to Oya',
  saveOffline: 'Connect to a server to save to Oya',
  offlineHint: 'Connect this browser to save to Oya. The draft and the code work offline.',
  issuesHint: 'Fix the steps marked ! before saving.',
  nameHint: 'Use letters, numbers, hyphens or underscores, up to 64.',
} as const;
