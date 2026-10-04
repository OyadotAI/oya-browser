/**
 * Chrome DevTools Recorder's JSON, both ways: a recording made in Chrome opens
 * as a workflow here, and a workflow saves in Chrome's shape for Chrome's
 * Recorder panel and @puppeteer/replay.
 *
 * ponytail: a Chrome frame is a path of indexes, ours a path of selectors; an
 * index becomes `iframe >> nth=i`, which holds while the page keeps its frames
 * in the same order. A Chrome "change" on a select carries its option's value,
 * not its label, so it opens as a Fill for the person to check.
 */
import type { Candidate, Draft, Step } from './types.ts';

/** A Chrome Recorder step, as read from a file: any field may be missing or malformed. */
export interface ChromeStep {
  /** The step type: navigate, click, change, keyDown, waitForExpression, ... */
  type?: string;
  /** Selectors, each a list of parts (one per shadow root) or a single string. */
  selectors?: (string | string[])[];
  /** A change's value. */
  value?: unknown;
  /** A key press's key. */
  key?: unknown;
  /** A scroll's vertical distance. */
  y?: number;
  /** A navigation's address. */
  url?: unknown;
  /** The frame path, as indexes. */
  frame?: unknown;
  /** The tab it ran in. */
  target?: unknown;
  /** Events Chrome checked after the step. */
  assertedEvents?: AssertedEvent[];
  /** A waitForExpression's expression. */
  expression?: string;
}

/** An event Chrome checked after a step. */
interface AssertedEvent {
  /** The event: navigation, ... */
  type?: string;
  /** Where a navigation landed. */
  url?: unknown;
}

/** A Chrome Recorder recording. */
export interface ChromeRecording {
  /** Its title. */
  title?: unknown;
  /** Its steps. */
  steps: ChromeStep[];
}

/** One of our steps as an import makes it, or as an export reads it: loose until normalized. */
type LooseStep = Partial<Step>;

/** A step to export: loose, but it names its action. */
type ExportStep = LooseStep & Pick<Step, 'action'>;

/** A draft to export: its name and its steps. */
interface ExportDraft extends Pick<Draft, 'name'> {
  /** Its steps, in our words. */
  steps: ExportStep[];
}

/** A Chrome step as an export writes it. */
type ChromeOut = Record<string, unknown>;

/** A draft as fromChromeRecording makes it, for normalizeDraft. */
export interface ImportedDraft {
  /** The recording's title. */
  name: string;
  /** Its steps, in our words. */
  steps: LooseStep[];
}

/** A draft as toChromeRecording writes it. */
export interface ExportedRecording {
  /** The draft's name. */
  title: string;
  /** Its steps, in Chrome's words. */
  steps: ChromeOut[];
}

/** Chrome step type → our action, for the types that have one. */
const FROM_CHROME: Record<string, string> = {
  navigate: 'navigate',
  click: 'click',
  doubleClick: 'double_click',
  hover: 'hover',
  change: 'type',
  keyDown: 'press_key',
  scroll: 'scroll',
  waitForElement: 'wait',
};

/** Our action → Chrome step type, for the actions Chrome has. */
const TO_CHROME: Record<string, string> = {
  navigate: 'navigate',
  click: 'click',
  double_click: 'doubleClick',
  hover: 'hover',
  type: 'change',
  select_option: 'change',
  press_key: 'keyDown',
  scroll: 'scroll',
  wait: 'waitForElement',
};

/** Chrome selector prefix → how it becomes one of our targets. */
const SELECTOR_KINDS: [string, (v: string, field?: boolean) => Candidate][] = [
  ['aria/', (v, field) => ({ kind: field ? 'label' : 'text', value: v })],
  ['text/', (v) => ({ kind: 'text', value: v })],
  ['xpath/', (v) => ({ kind: 'css', value: 'xpath=' + v })],
  ['pierce/', (v) => ({ kind: 'css', value: v })],
];

/** One Chrome selector (a list of parts, one per shadow root) as our target. */
function fromSelector(parts: string | string[], field: boolean): Candidate {
  const selector = (Array.isArray(parts) ? parts : [parts]).join(' ');
  const [prefix, make] = SELECTOR_KINDS.find(([p]) => selector.startsWith(p)) || ['', null];
  return make ? make(selector.slice(prefix.length), field) : { kind: 'css', value: selector };
}

/** How many `>` steps make a CSS selector a path by position rather than a name. */
const PATH_STEPS = 3;

/** A CSS path by position, several levels deep: what is left when nothing names the element. */
const positional = (c: Candidate): boolean =>
  c.kind === 'css' && (c.value.split('>').length > PATH_STEPS || /:nth-|^xpath=/.test(c.value));

/** A Chrome step's selectors as our targets, in Chrome's order, a deep positional path after the names. */
function fromSelectors(step: ChromeStep, field: boolean): Candidate[] {
  const found = (step.selectors || []).map((parts) => fromSelector(parts, field));
  return [...found.filter((c) => !positional(c)), ...found.filter(positional)];
}

/** Our action → our fields from its Chrome step's own value. */
const OWN_FIELDS: Record<string, (step: ChromeStep) => LooseStep> = {
  type: (step) => ({ text: String(step.value ?? '') }),
  press_key: (step) => ({ key: String(step.key ?? '') }),
  scroll: (step) => ({ direction: (step.y ?? 0) < 0 ? 'up' : 'down', amount: Math.abs(step.y ?? 0) }),
  navigate: (step) => ({ url: String(step.url ?? '') }),
};

/** A Chrome step's fields as ours: its value, key, scroll, frame and tab. */
function fromFields(step: ChromeStep, action: string): LooseStep {
  const out: LooseStep = Object.hasOwn(OWN_FIELDS, action) ? OWN_FIELDS[action](step) : {};
  if (Array.isArray(step.frame)) out.frames = step.frame.map((i: unknown) => `iframe >> nth=${Number(i)}`);
  if (step.target && step.target !== 'main') out.tab = String(step.target);
  return out;
}

/** The page a Chrome step's navigation lands on, as a page check, when it asserted one. */
function landing(step: ChromeStep): LooseStep[] {
  const navigation = (step.assertedEvents || []).find((e) => e.type === 'navigation' && e.url);
  return navigation ? [{ action: 'assert_page', expected: String(navigation.url) }] : [];
}

/** The address a page check we saved as `waitForExpression` waits for, or null for any other expression. */
function checkedPage(step: ChromeStep): string | null {
  const match = /^location\.origin \+ location\.pathname === ("(?:[^"\\]|\\.)*")$/.exec(step.expression || '');
  return match ? JSON.parse(match[1]) : null;
}

/** One Chrome step as our steps; a type we lack becomes nothing (setViewport, keyUp, close). */
function fromChromeStep(step: ChromeStep | null | undefined): LooseStep[] {
  if (step?.type === 'waitForExpression' && checkedPage(step))
    return [{ action: 'assert_page', expected: checkedPage(step) as string }];
  if (!step || typeof step.type !== 'string' || !Object.hasOwn(FROM_CHROME, step.type)) return [];
  const action = FROM_CHROME[step.type];
  const candidates = fromSelectors(step, action === 'type');
  const ours = { action, ...fromFields(step, action), ...(candidates.length ? { candidates } : {}) };
  return [ours, ...(action === 'navigate' ? [] : landing(step))];
}

/** Whether `json` is a Chrome Recorder recording. */
export const isChromeRecording = (json: unknown): json is ChromeRecording =>
  !!json &&
  Array.isArray((json as ChromeRecording).steps) &&
  (json as ChromeRecording).steps.some((s) => typeof s?.type === 'string');

/** A Chrome Recorder recording as a draft for normalizeDraft. */
export function fromChromeRecording(json: ChromeRecording): ImportedDraft {
  return { name: String(json.title || 'Imported recording'), steps: json.steps.flatMap(fromChromeStep) };
}

/** Selector syntax only Playwright reads (`:text-is`, `>>`, `nth=`): Chrome's querySelector refuses it. */
const PLAYWRIGHT_ONLY = /:text-is\(|:has-text\(|>>|\bnth=|^internal:/;

/** Our targets that Chrome can read, as Chrome selectors. */
const toSelectors = (candidates: Candidate[]): string[][] =>
  candidates.filter((c) => !(c.kind === 'css' && PLAYWRIGHT_ONLY.test(c.value))).map(toSelector);

/** Our target as a Chrome selector. */
function toSelector(c: Candidate): string[] {
  if (c.kind === 'css') return [c.value.startsWith('xpath=') ? 'xpath/' + c.value.slice('xpath='.length) : c.value];
  if (c.kind === 'text') return ['text/' + c.value];
  if (c.kind === 'testId') return [`[data-testid=${JSON.stringify(c.value)}]`];
  if (c.kind === 'placeholder') return [`[placeholder=${JSON.stringify(c.value)}]`];
  return ['aria/' + c.value];
}

/** Action → the Chrome fields its own value becomes. */
const VALUE_FIELDS: Record<string, (step: LooseStep) => Record<string, unknown>> = {
  navigate: (step) => ({ url: step.url }),
  type: (step) => ({ value: step.text ?? '' }),
  select_option: (step) => ({ value: step.option ?? '' }),
  press_key: (step) => ({ key: step.key }),
  scroll: (step) => ({ y: (step.direction === 'up' ? -1 : 1) * (step.amount || 0) }),
};

/** Our step's fields in Chrome's words. */
function toFields(step: ExportStep): Record<string, unknown> {
  const out: Record<string, unknown> = Object.hasOwn(VALUE_FIELDS, step.action) ? VALUE_FIELDS[step.action](step) : {};
  const selectors = toSelectors(step.candidates || []);
  if (selectors.length) out.selectors = selectors;
  if (step.tab && step.tab !== 'main') out.target = step.tab;
  return out;
}

/**
 * Chrome has no Back or Forward: one becomes a navigation to the page it landed
 * on, which the page check recorded after it names; without one it is left out.
 */
function historyStep(next: LooseStep | undefined): ChromeOut[] {
  return next?.action === 'assert_page' ? [{ type: 'navigate', url: next.expected }] : [];
}

/** Our step as Chrome steps: a key is pressed and let go, and a page check waits on the address. */
function toChromeSteps(step: ExportStep, next: LooseStep | undefined): ChromeOut[] {
  if (step.action === 'go_back' || step.action === 'go_forward') return historyStep(next);
  if (step.action === 'assert_page') {
    const expression = `location.origin + location.pathname === ${JSON.stringify(pageOf(step.expected))}`;
    return [{ type: 'waitForExpression', expression }];
  }
  if (!Object.hasOwn(TO_CHROME, step.action)) return [];
  const chrome = { type: TO_CHROME[step.action], ...toFields(step) };
  return step.action === 'press_key' ? [chrome, { ...chrome, type: 'keyUp' }] : [chrome];
}

/** An address without its query or fragment. */
function pageOf(url: unknown): string {
  try {
    const u = new URL(String(url));
    return u.origin + u.pathname;
  } catch {
    return String(url);
  }
}

/** A draft as a Chrome Recorder recording; steps Chrome has no type for are left out, and so are frames. */
export function toChromeRecording(draft: ExportDraft): ExportedRecording {
  const enabled = draft.steps.filter((step) => step.enabled !== false);
  return { title: draft.name, steps: enabled.flatMap((step, i) => toChromeSteps(step, enabled[i + 1])) };
}
