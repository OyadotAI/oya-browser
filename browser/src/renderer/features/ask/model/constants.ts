/**
 * The Ask pane's fixed text, codes and lookup tables: the words it shows, the
 * server's refusal codes, the tools a playbook can replay, and the names the
 * step line gives the browser's commands.
 */

/** The words the Ask pane shows. */
export const ASK_TEXT = {
  /** The pill on a reply where the agent stopped to ask the person something. */
  needsInput: 'Needs your answer',
  /** The same pill once the person replied. */
  answered: 'Answered',
  /** The composer's placeholder while the agent waits for an answer. */
  answerPlaceholder: 'Type your answer…',
  /** Explicit permission for another bounded run without repeating completed work. */
  continueTask:
    'Continue the unfinished task from the current page. Check what is already done and do not repeat completed actions.',
  /** A reply with no text. */
  noResponse: '(no response)',
  /** What a stopped run says. */
  stopped: 'Stopped.',
  /** The error the main process answers a stopped chat with. */
  stoppedError: 'Stopped',
  /** Put before an error shown as the agent's reply. */
  errorPrefix: 'Error: ',
  /** The live line before any step is known. */
  thinking: 'Thinking…',
  /** The live line while a run's dropped connection is being picked up again. */
  reconnecting: 'Reconnecting…',
  /** The verdicts above a report. */
  done: 'Done',
  failed: 'Could not finish',
  /** The Copy button, before and after copying. */
  copy: 'Copy',
  copied: 'Copied',
  /** The attachments' notes and labels. */
  tooBig: 'Attachments can be up to 10 MB in all.',
  unreadable: 'Could not read that file.',
  remove: 'Remove',
  attached: 'Attached',
  /** The persona picker's default entry. */
  defaultProfile: 'Default profile',
  /** The model card's titles and key help. */
  modelTitle: 'AI model',
  connectTitle: 'Connect an AI model',
  keyKept: 'Your saved key is kept. Paste a new one only to replace it.',
  keySaved: '••••••••  saved',
  keyPlaceholder: 'API key',
  /** When the free trial is used up. */
  claimNeeded: 'Your free trial is used up. Create your free Oya account to keep going.',
  /** The card shown when today's free steps are used up. */
  trialTitle: 'You’ve used today’s free steps',
  trialBody: 'Create a free Oya account to keep going. This browser, your logins and ChatGPT stay as they are.',
  trialPerks: ['500 agent steps a month', 'Cloud browsers that run while you’re away', 'Routines on a schedule'],
  trialCreate: 'Create free account',
  trialLater: 'Not now',
  /** The sign-in provider's words (ChatGPT). */
  signInHelp: 'Runs on your ChatGPT Plus or Pro plan, no API key. Sign in on this computer.',
  signInWaiting: 'Finish signing in in your browser. This card updates when you are done.',
  signInButton: 'Sign in with ChatGPT',
  /** The model picker's words. */
  chooseModel: 'Choose a model',
  customModel: 'Custom model id',
  asCustom: 'as a custom model id',
  typeModel: 'Type a model id to use it.',
  /** Save as playbook. */
  saveAsPlaybook: 'Save as playbook',
  saveExplainer: 'Save this run as a playbook to replay it without the model.',
  nothingToReplay: 'Nothing to replay: this run only read pages.',
  badName: 'Use 1–64 letters, numbers, hyphens or underscores.',
  playbookName: 'Playbook name',
  defaultPlaybook: 'agent-run',
} as const;

/** The codes the server refuses a chat with when the project has no model key, or the provider refused it. */
export const KEY_CODES: readonly string[] = ['llm_unconfigured', 'llm_rejected'];

/** The code for a key the provider refused; its error is shown on the model card. */
export const REJECTED_CODE = 'llm_rejected';

/** The server's refusal when a browser's free trial is used up and it needs an Oya account. */
export const CLAIM_CODE = 'claim_required';

/** How the agent opens its report: DONE: or FAILED:, shown as a verdict instead of text. */
export const VERDICT = /^(DONE|FAILED):\s*/i;

/** How a reply begins when the agent stopped to ask the person (the server's NEEDS_INPUT, agent/loop.ts). */
export const NEEDS_INPUT = /^NEEDS INPUT:\s*/i;

/** A reply that reports a failed run. */
export const FAILED_REPLY = /^FAILED:/i;

/** A playbook name the server accepts. */
export const PLAYBOOK_NAME = /^[\w-]{1,64}$/;

/** The longest playbook name the field takes. */
export const PLAYBOOK_NAME_MAX = 64;

/** An activity entry for a command from the server: `cmd: <action>`. */
export const COMMAND_ENTRY = /^cmd:\s*(\S+)$/;

/** The example tasks the empty state offers: what is sent, and the button's words. */
export const EXAMPLE_TASKS = [
  { task: 'Summarize this page in three bullet points', label: 'Summarize this page' },
  { task: 'Find the contact or support email on this site', label: 'Find a contact email' },
] as const;

/**
 * Tools the server records as replayable steps (server/src/modules/agent/recorder.ts,
 * RECORDED; a unit test keeps the two lists equal).
 */
export const REPLAYABLE_TOOLS: ReadonlySet<string> = new Set([
  'navigate',
  'click',
  'double_click',
  'click_coordinates',
  'type',
  'keyboard_type',
  'select_option',
  'upload_file',
  'press_key',
  'key_chord',
  'edit_text',
  'scroll_at',
  'scroll',
  'wait',
  'handle_dialog',
  'open_tab',
  'switch_tab',
  'close_tab',
  'hover',
  'go_back',
  'go_forward',
  'reload',
]);

/** What people are told an action is doing (the studio's step names, then the step line's own). */
export const STEP_NAMES: Readonly<Record<string, string>> = {
  navigate: 'Navigate',
  go_back: 'Back',
  go_forward: 'Forward',
  type: 'Fill',
  click: 'Click',
  select_option: 'Select',
  press_key: 'Press key',
  key_chord: 'Press shortcut',
  edit_text: 'Edit text',
  scroll_at: 'Scroll at point',
  start_trace: 'Start tracing',
  read_trace: 'Read trace',
  stop_trace: 'Stop tracing',
  scroll: 'Scroll',
  upload_file: 'Upload',
  wait: 'Wait for element',
  analyze: 'Reading the page',
  screenshot: 'Taking a screenshot',
  list_tabs: 'Listing the tabs',
  open_tab: 'Opening a tab',
  switch_tab: 'Switching tab',
  close_tab: 'Closing a tab',
  handle_dialog: 'Answering a dialog',
  click_coordinates: 'Clicking',
  double_click: 'Double-clicking',
  keyboard_type: 'Typing',
  mouse_move: 'Moving the mouse',
  drag: 'Dragging',
  hover: 'Hovering',
  back: 'Going back',
  forward: 'Going forward',
  reload: 'Reloading the page',
  run_script: 'Reading the page',
};

/** The parameter that says what an action acted on, for actions where it reads well. */
export const STEP_DETAILS: Readonly<Record<string, string>> = {
  navigate: 'url',
  open_tab: 'url',
  type: 'text',
  keyboard_type: 'text',
  wait: 'selector',
  press_key: 'key',
  key_chord: 'key',
  edit_text: 'operation',
  scroll: 'direction',
};
