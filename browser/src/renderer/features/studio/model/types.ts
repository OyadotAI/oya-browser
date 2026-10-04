/**
 * The workflow workspace as the main process sends it (src/main/workflow/workspace.ts
 * `snapshot()`), and the commands the studio sends back on the `workspace`
 * channel (src/main/ipc/workspace.ts; any type it does not handle itself is an
 * edit, src/main/workflow/edits.ts).
 */

/** One way to find a step's element. */
export interface Candidate {
  /** The strategy: testId, role, label, text, placeholder or css. */
  kind: string;
  /** What the strategy looks for. */
  value: string;
  /** The ARIA role, for a role locator. */
  role?: string;
}

/** What the recorder noted about a step's element. */
export interface RecordedElement {
  /** The input type (checkbox, radio, button...). */
  type?: string;
  /** Its test id. */
  testId?: string;
  /** Its name attribute. */
  name?: string;
  /** Its id attribute. */
  domId?: string;
}

/** One step of a workflow. */
export interface Step {
  /** The step's id, stable across edits. */
  id: string;
  /** What it does (navigate, click, type...). */
  action: string;
  /** Whether a run performs it. */
  enabled: boolean;
  /** Whether a run pauses before it. */
  breakpoint: boolean;
  /** Its locators, the one replay tries first first. */
  candidates?: Candidate[];
  /** What was recorded about its element. */
  el?: RecordedElement;
  /** The frame selectors from the page down to its frame. */
  frames?: string[];
  /** How long it may wait, in milliseconds. */
  timeout?: number;
  /** The address it opens. */
  url?: string;
  /** The text it types. */
  text?: string;
  /** The option it picks. */
  option?: string;
  /** The file it uploads. */
  file?: string;
  /** The key it presses. */
  key?: string;
  /** Which way it scrolls. */
  direction?: string;
  /** How far it scrolls. */
  amount?: number;
  /** What it expects to find. */
  expected?: string;
  /** The query parameters a page check holds to, comma separated. */
  params?: string;
  /** Why the recorder could not capture it fully. */
  captureIssue?: string;
}

/** A variable's settings. */
export interface VariableConfig {
  /** Its default value (none for a secret). */
  default?: string;
  /** Whether it is asked for on each run and never saved. */
  secret?: boolean;
}

/** Variables by name. */
export type Variables = Record<string, VariableConfig>;

/** The workflow being edited. */
export interface Draft {
  /** Its id. */
  id: string;
  /** Its name ('Untitled workflow' until named). */
  name: string;
  /** What it does. */
  description: string;
  /** Its steps, in order. */
  steps: Step[];
  /** Its variables. */
  variables: Variables;
  /** The names of its secret variables. */
  secrets: string[];
  /** 'recording' while recorded, otherwise 'paused'. */
  phase: string;
  /** Bumped on every change. */
  revision: number;
  /** When it was last saved to Oya, if ever. */
  publishedAt?: number;
}

/** Something that stops the workflow running. */
export interface Issue {
  /** What is wrong, in words for the person. */
  message: string;
  /** The step it is about, if one. */
  stepId?: string;
}

/** A stored draft in the workflow picker. */
export interface LibraryItem {
  /** The draft's id. */
  id: string;
  /** Its name. */
  name: string;
  /** How many steps it has (absent when unknown). */
  steps?: number;
  /** Set when it could not be read. */
  error?: boolean;
}

/** One event of a test run. */
export interface RunEvent {
  /** step, attention, target, or a kind the timeline skips. */
  kind: string;
  /** The step it is about. */
  stepId?: string;
  /** running, passed, failed, paused... */
  status?: string;
  /** What happened, in words. */
  message?: string;
  /** How long the step took, in milliseconds. */
  duration?: number;
}

/** A target that worked when the recorded one failed. */
export interface Repair {
  /** The step repaired. */
  stepId: string;
  /** The target that failed. */
  original: Candidate;
  /** The target that worked. */
  replacement: Candidate;
  /** The repaired copy saved as a new draft. */
  draftId: string;
}

/** A test run. */
export interface Run {
  /** Its id. */
  id: string;
  /** The draft it ran. */
  draftId: string;
  /** starting, running, paused, stopping, succeeded, failed... */
  status: string;
  /** Its events, oldest first. */
  events: RunEvent[];
  /** The repairs it found. */
  repairs: Repair[];
  /** Why it failed. */
  error?: string;
  /** How many assertions passed. */
  assertions?: number;
}

/** A stored run in the previous-runs list. */
export interface RunHistoryItem {
  /** The run's id. */
  id: string;
  /** "draft name · status". */
  name: string;
  /** When it was last saved, in ms since the epoch. */
  updatedAt: number;
}

/** Everything the studio shows, in one object. */
export interface WorkspaceSnapshot {
  /** The draft being edited. */
  draft: Draft;
  /** Its generated Playwright module ('' while issues stop it). */
  code: string;
  /** Step id to the code line it generated. */
  mapping: Record<string, number>;
  /** What stops a test run. */
  issues: Issue[];
  /** The stored drafts. */
  library: LibraryItem[];
  /** Whether the draft is what was last saved to Oya. */
  saved: boolean;
  /** Why the draft could not be stored, or null. */
  storageError: string | null;
  /** Whether Undo has something to undo. */
  canUndo: boolean;
  /** Whether Redo has something to redo. */
  canRedo: boolean;
  /** Previous runs. */
  runHistory: RunHistoryItem[];
  /** The run shown, or null. */
  run: Run | null;
  /** The redacted diagnostics bundle. */
  support: unknown;
  /** Answer to export-json: the file was written. */
  exported?: boolean;
  /** Answer to support: the report was written. */
  supportSaved?: boolean;
}

/** The changes an update may make to a step. */
export type StepPatch = Partial<Omit<Step, 'id'>>;

/** A step as Add step sends it. */
export type NewStep = {
  /** Its action. */
  action: string;
  /** No targets yet. */
  candidates: Candidate[];
  /** Nothing expected yet. */
  expected: string;
};

/** A command that carries only its type. */
export type PlainCommand = {
  /** get, resume-recording, support, import-json, undo, redo or new. */
  type: 'get' | 'resume-recording' | 'support' | 'import-json' | 'undo' | 'redo' | 'new';
};

/** A command about one step, draft or run, by id. */
export type IdCommand = {
  /** pick (a target), open-run, open (a draft), delete or duplicate (a step). */
  type: 'pick' | 'open-run' | 'open' | 'delete' | 'duplicate';
  /** The step, run or draft. */
  id: string;
};

/** Starts a test run. */
export type ValidateCommand = {
  /** validate. */
  type: 'validate';
  /** The run inputs, by variable. */
  vars: Record<string, string>;
  /** Milliseconds of pause per step. */
  slowMo: number;
  /** Stop after this step. */
  runTo?: string;
};

/** Pauses, resumes, steps or stops the test run. */
export type ControlCommand = {
  /** control. */
  type: 'control';
  /** pause, resume, step or stop. */
  command: string;
};

/** Saves the workflow as JSON. */
export type ExportCommand = {
  /** export-json. */
  type: 'export-json';
  /** Oya's own JSON or Chrome Recorder's. */
  format: 'oya' | 'chrome';
};

/** Sets the name and description. */
export type MetadataCommand = {
  /** metadata. */
  type: 'metadata';
  /** The name. */
  name: string;
  /** The description. */
  description: string;
};

/** Renames a variable everywhere. */
export type RenameCommand = {
  /** rename-variable. */
  type: 'rename-variable';
  /** Its name now. */
  name: string;
  /** Its new name. */
  nextName: string;
};

/** Replaces every variable. */
export type VariablesCommand = {
  /** variables. */
  type: 'variables';
  /** The variables. */
  variables: Variables;
};

/** Adds a step after another (or at the end). */
export type AddCommand = {
  /** add. */
  type: 'add';
  /** The step it goes after; none for the end. */
  id: string | undefined;
  /** The new step. */
  step: NewStep;
};

/** Changes fields of a step. */
export type UpdateCommand = {
  /** update. */
  type: 'update';
  /** The step. */
  id: string;
  /** Its changed fields. */
  patch: StepPatch;
};

/** Moves a step. */
export type MoveCommand = {
  /** move. */
  type: 'move';
  /** The step. */
  id: string;
  /** Places to move by: -1 up, 1 down. */
  delta: number;
};

/** Every command the studio sends on the `workspace` channel. */
export type WorkspaceCommand =
  | PlainCommand
  | IdCommand
  | ValidateCommand
  | ControlCommand
  | ExportCommand
  | MetadataCommand
  | RenameCommand
  | VariablesCommand
  | AddCommand
  | UpdateCommand
  | MoveCommand;
