/**
 * The playbook editor's fixed words.
 */

/** The words the editor, its panes and the tester show. */
export const EDITOR_TEXT = {
  title: (name: string) => `Edit ${name}`,
  description: 'The steps and variables a replay uses. Every save is checked as an import is; nothing here runs code.',
  explorer: 'Explorer',
  explorerHint: 'Edits are checked as an import is when you save.',
  closeTab: (name: string) => `Close ${name}`,
  empty: 'Open a file from the explorer.',
  readOnly: 'Read-only',
  testPanel: 'Test',
  togglePanel: 'Toggle the test panel',
  toggleSidebar: 'Toggle the explorer',
  problems: (n: number) => (n === 1 ? '1 problem' : `${n} problems`),
  noProblems: 'No problems',
  saved: 'Saved',
  cursor: (line: number, col: number) => `Ln ${line}, Col ${col}`,
  spaces: 'Spaces: 2',
  loading: 'Loading the playbook…',
  definitionLabel: 'Playbook definition (JSON)',
  noVariables: 'The steps use no {{variables}} yet. Put {{name}} in a step’s text or URL to make one.',
  variable: 'Variable',
  defaultValue: 'Default',
  secret: 'Secret',
  secretHint: 'Secret values are never shown again once saved.',
  codeHint: 'The Playwright code for the saved steps. It updates when you save.',
  browser: 'Browser',
  noBrowsers: 'Start a browser to test on.',
  sample: (name: string) => `Sample value for ${name}`,
  heal: 'Let the agent finish a step that breaks',
  run: 'Run test',
  runningLabel: 'Running…',
  noSteps: 'No steps have played yet.',
  ok: 'played',
  broke: 'broke',
  discard: 'Discard',
  saveAs: 'Save as…',
  saveAsPrompt: 'Save as a new playbook named',
  save: 'Save',
  saving: 'Saving…',
  unsaved: 'Unsaved changes',
  confirmClose: 'Discard your unsaved changes?',
  viewLabel: 'How to show the definition',
  visual: 'Visual',
  json: 'JSON',
  stepsLabel: 'Steps',
  start: 'Start',
  end: 'End',
  emptySteps: 'No steps yet. Record a playbook to add some.',
  fixInJson: 'Fix it in JSON to see the steps.',
  showJson: 'Open JSON',
  stepTitle: (n: number, action: string) => `Step ${n} · ${action}`,
  recordedElement: 'Recorded element',
  cancel: 'Cancel',
  confirm: 'Confirm',
  deleteStep: 'Delete step',
} as const;

/** The step fields the panel lets a person change, each labelled as the panel shows it. */
export const STEP_FIELDS = {
  url: 'URL',
  text: 'Text',
  option: 'Option',
  file: 'File',
  key: 'Key',
  direction: 'Direction',
  amount: 'Amount',
  expected: 'Expected',
} as const;

/** A field the panel lets a person change. */
export type StepField = keyof typeof STEP_FIELDS;

/** The ways a scroll step can go, as replay reads them. */
export const DIRECTIONS = ['up', 'down', 'left', 'right'] as const;

/** How the definition tab shows the playbook. */
export type DefinitionView = 'json' | 'visual';

/** The files the explorer lists, in order: what each is called, its language and whether it can be typed in. */
export const FILES = {
  definition: { name: 'playbook.json', language: 'JSON', readOnly: false },
  variables: { name: 'variables', language: 'Variables', readOnly: false },
  code: { name: 'playwright.ts', language: 'TypeScript', readOnly: true },
} as const;

/** A file the workbench can open. */
export type FileId = keyof typeof FILES;

/** The run's state as the tester says it. */
export const RUN_STATUS: Readonly<Record<string, string>> = {
  running: 'Running',
  needs_attention: 'Waiting for a person',
  succeeded: 'Passed',
  failed: 'Failed',
};
