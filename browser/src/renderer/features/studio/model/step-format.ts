/**
 * How a step reads in the studio, as pure functions: its row's number,
 * target and detail (a secret only as a secret), and the step editor's
 * heading, target and frames.
 */
import { RendererConstants as C } from '../../../core/constants.ts';
import { FRAME_ARROW, READABLE_KINDS, STRATEGIES, WHOLE_VARIABLE } from './constants.ts';
import { actionName } from './studio-model.ts';
import type { Candidate, RecordedElement, Step, Variables } from './types.ts';

/** A step's number, or a dot when it has a breakpoint. */
export const stepNumber = (step: Step, index: number): string =>
  step.breakpoint ? '●' : String(index + 1).padStart(C.STEP_NUMBER_DIGITS, '0');

/** A row's classes: selected, and disabled when the step is switched off. */
export const stepClass = (step: Step, selected: boolean): string =>
  'studio-step' + (selected ? ' selected' : '') + (!step.enabled ? ' disabled' : '');

/** Text cut to what a row has room for. */
export function clip(text: string): string {
  const max = C.ROW_TEXT_CHARS;
  return text.length > max ? text.slice(0, max - 1) + '…' : text;
}

/** A Fill's text as a row shows it: quoted, or marked secret when it is one. */
export function typed(text: string | undefined, variables: Variables): string {
  if (!text) return '(empty)';
  const variable = WHOLE_VARIABLE.exec(text)?.[1];
  if (variable && variables[variable]?.secret) return `secret {{${variable}}}`;
  return `“${clip(text.replace(/\s+/g, ' '))}”`;
}

/** The named query parameters of `url` as `k=v, …`. */
function held(url: URL, params: string | undefined): string {
  const names = String(params || '')
    .split(',')
    .filter(Boolean);
  return names.map((k) => `${k}=${url.searchParams.get(k) ?? ''}`).join(', ');
}

/** A checked page as a row shows it: host and path, and the parameters it holds to, not a page of tokens. */
export function checkedPage(expected: string | undefined, params: string | undefined): string {
  try {
    const url = new URL(expected ?? '');
    const kept = held(url, params);
    return clip(url.host + url.pathname + (kept ? ` (${kept})` : ''));
  } catch {
    return clip(String(expected || ''));
  }
}

/** Action → what its row says after the target. */
const DETAILS: Readonly<Record<string, (step: Step, variables: Variables) => string | undefined>> = {
  type: (step, variables) => typed(step.text, variables),
  select_option: (step) => (step.option ? `“${step.option}”` : undefined),
  scroll: (step) => `${step.direction || 'down'} ${step.amount || ''}`.trim(),
  go_back: () => 'to the previous page',
  go_forward: () => 'to the next page',
  assert_page: (step) => checkedPage(step.expected, step.params),
};

/** What the step enters, picks, presses, goes to or expects. */
export function stepDetail(step: Step, variables: Variables): string | undefined {
  if (Object.hasOwn(DETAILS, step.action)) return DETAILS[step.action](step, variables);
  return step.url || step.key || step.expected || step.captureIssue;
}

/** A CSS selector made readable: `[id="x"]` reads as `#x`, and only the last levels of a path show. */
export function shortSelector(selector: string | undefined): string {
  if (!selector) return '';
  const plain = selector.replace(/\[id="([^"]+)"\]/g, '#$1');
  const levels = plain.split(' > ');
  const shown = C.SELECTOR_LEVELS_SHOWN;
  return levels.length > shown ? '… > ' + levels.slice(-shown).join(' > ') : plain;
}

/** A name from the recorded element: its test id, name or id, with what kind of element it is. */
export function recordedName(el: RecordedElement | undefined): string {
  const name = el?.testId || el?.name || el?.domId;
  if (!name) return '';
  return el?.type && el.type !== 'button' ? `${name} ${el.type}` : name;
}

/**
 * What the step aims at, in the words a person sees on the page: its label,
 * role, text or placeholder when it has one, then what was recorded about the
 * element, and only then a shortened selector.
 */
export function stepTarget(step: Step): string {
  const candidates = step.candidates || [];
  const readable = candidates.find((c) => READABLE_KINDS.includes(c.kind));
  if (readable) return readable.value;
  const short = shortSelector(candidates[0]?.value);
  const kind = ['checkbox', 'radio'].includes(step.el?.type ?? '') ? step.el?.type + ' ' : '';
  return recordedName(step.el) || (short && kind + short);
}

/** A row's value: target and detail, or a nudge to configure it. */
export function stepValue(step: Step, variables: Variables): string {
  const parts = [stepTarget(step), stepDetail(step, variables)].filter(Boolean);
  return parts.join(' · ') || 'Select to configure';
}

/** "Step n · Action". */
export const editorTitle = (step: Step, steps: Step[]): string =>
  `Step ${steps.indexOf(step) + 1} · ${actionName(step.action)}`;

/** The step's first target, or an empty CSS one. */
export const firstTarget = (step: Step | undefined): Candidate => step?.candidates?.[0] || { kind: 'css', value: '' };

/** A recorded target as an alternatives button says it. */
export const candidateLabel = (c: Candidate): string =>
  `${(Object.hasOwn(STRATEGIES, c.kind) && STRATEGIES[c.kind]) || c.kind}: ${c.value}`;

/** The frames written joined by arrows. */
export const framesText = (frames: string[] | undefined): string => (frames || []).join(` ${FRAME_ARROW} `);

/** "a → b" as ['a', 'b'], without empty levels. */
export const parseFrames = (value: string): string[] =>
  value
    .split(FRAME_ARROW)
    .map((s) => s.trim())
    .filter(Boolean);

/** Whether two targets are the same locator. */
export const sameTarget = (a: Candidate | undefined, b: Candidate | undefined): boolean =>
  !!a && !!b && a.kind === b.kind && a.value === b.value && a.role === b.role;
