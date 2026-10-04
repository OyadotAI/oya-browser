/**
 * The shapes a workflow is made of once normalized: a draft, its steps, their
 * locator candidates and recorded elements, and what generation returns.
 * Every side (the desktop, the worker, the server) reads drafts in this shape.
 */

/** One way to find a step's element: a locator kind and its value (a role also names the role). */
export interface Candidate {
  /** testId, role, label, text, placeholder or css. */
  kind: string;
  /** What the locator matches: the test id, the accessible name, the text or the CSS. */
  value: string;
  /** The ARIA role, for a role locator. */
  role?: string;
}

/** The recorded element's text fields, kept so a step can be repaired later. */
export interface RecordedElement {
  /** Any recorded text field (type, tag, text, domId, name, href, ...). */
  [field: string]: string | undefined;
}

/** One normalized step. */
export interface Step {
  /** The step's id, unique in its draft. */
  id: string;
  /** What the step does: 'click', 'navigate', 'assert_page', ... */
  action: string;
  /** False for a step kept in the list but turned off. */
  enabled: boolean;
  /** Pause here when validating. */
  breakpoint: boolean;
  /** The ways a replay can find the step's element, best first. */
  candidates: Candidate[];
  /** The recorded tab name ('main', 'tab-1', ...). */
  tab: string;
  /** Frame selectors from the page down to the step's frame. */
  frames: string[];
  /** The step's timeout in milliseconds. */
  timeout: number;
  /** The element as recorded, for repairs. */
  el?: RecordedElement;
  /** Where a navigate step goes. */
  url?: string;
  /** What a type step types. */
  text?: string;
  /** The option a select_option step picks. */
  option?: string;
  /** The file an upload_file step sends. */
  file?: string;
  /** The key a press_key step presses. */
  key?: string;
  /** Which way a scroll step scrolls. */
  direction?: string;
  /** What an assertion expects. */
  expected?: string;
  /** The query parameters an assert_page step holds to, comma separated. */
  params?: string;
  /** Why the step may not replay as recorded, for the person. */
  captureIssue?: string;
  /** How far a scroll step scrolls, in pixels. */
  amount?: number;
  /** When it was recorded, in epoch milliseconds. */
  t?: number;
  /** Set on the navigate step where the person began. */
  start?: boolean;
  /** Anything else a step carries through (the recorder and the studio add their own marks). */
  [field: string]: unknown;
}

/** One variable's settings; a secret never keeps a default. */
export interface Variable {
  /** Whether the value is a secret (never stored, redacted everywhere). */
  secret: boolean;
  /** The value used when none is given. */
  default?: string;
}

/** One normalized draft. */
export interface Draft {
  /** The draft schema it was saved under. */
  schemaVersion: number;
  /** The draft's id. */
  id: string;
  /** Bumped on every saved change. */
  revision: number;
  /** Its name, for the person. */
  name: string;
  /** What it does, for the person. */
  description: string;
  /** Its steps, in order. */
  steps: Step[];
  /** Its variables by name. */
  variables: Record<string, Variable>;
  /** The names of its secret variables. */
  secrets: string[];
  /** When it was created, in epoch milliseconds. */
  createdAt: number;
  /** When it last changed, in epoch milliseconds. */
  updatedAt: number;
  /** Whether it is still being recorded. */
  phase: 'recording' | 'paused';
  /** The draft this one repairs, when it is a repair. */
  repairedFrom?: unknown;
  /** When it was published to the server. */
  publishedAt?: unknown;
  /** The revision last published. */
  publishedRevision?: unknown;
  /** The run it came from. */
  run?: unknown;
  /** Anything else a draft carries through. */
  [field: string]: unknown;
}

/** One problem that blocks a run: which step, and what to tell the person. */
export interface Issue {
  /** The step with the problem. */
  stepId: string;
  /** What is wrong, for the person. */
  message: string;
}

/** What generate() returns: the Playwright module, where each step starts in it, and the problems found. */
export interface Generated {
  /** The module's source text. */
  code: string;
  /** Step id to the line it starts on. */
  mapping: Record<string, number>;
  /** The problems found (none block generation, or it throws). */
  issues: Issue[];
}

/**
 * What a recorded or live element says about itself, as far as finding it again
 * goes. Every field is optional: a page reports what it has, and values from a
 * page or a file are untrusted.
 */
export interface ElementFacts {
  /** The element's type as the analyzer names it: input, checkbox, link, ... */
  type?: string;
  /** Its tag name. */
  tag?: string;
  /** Its visible text, or a field's label. */
  text?: string;
  /** Its id attribute. */
  domId?: string;
  /** Its name attribute. */
  name?: string;
  /** Its placeholder. */
  placeholder?: string;
  /** Its aria-label. */
  ariaLabel?: string;
  /** Its data-testid. */
  testId?: string;
  /** Its ARIA role. */
  role?: string;
  /** A link's resolved target. */
  href?: string;
  /** A link's target as the page wrote it. */
  rawHref?: string;
  /** A checkbox's or radio's value within its group. */
  choice?: string;
  /** Its name without a live count, when the name had one. */
  stableText?: string;
  /** Set when the page repeats its name. */
  repeats?: unknown;
  /** A selector for its name within the nearest container where it is unique. */
  scoped?: string;
  /** A CSS path to it by position. */
  path?: string;
  /** The selector of the shadow host it sits in. */
  host?: string;
  /** Set when the page repeats its test id. */
  testIdRepeats?: unknown;
  /** Set when the page repeats its link target. */
  hrefRepeats?: unknown;
}
