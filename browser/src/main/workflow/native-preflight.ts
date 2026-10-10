/** Refuse unsupported workflow semantics before opening tabs or changing browser ownership. */
import { generate, PLACEHOLDER, type Draft, type Step } from '../../workflow/index.ts';
import { workflowKey } from './native-keys.ts';
import type { RunOptions } from './validation.ts';

/** Actions whose native implementation preserves recorded step semantics. */
const ACTIONS = new Set([
  'navigate',
  'click',
  'type',
  'press_key',
  'scroll',
  'wait',
  'assert_visible',
  'assert_text',
  'assert_value',
  'assert_url',
  'assert_page',
  'checkpoint',
  'go_back',
  'go_forward',
  'double_click',
  'hover',
  'select_option',
  'upload_file',
]);
/** Locator engines implemented exactly by native DOM queries. */
const LOCATORS = new Set(['css', 'testId', 'placeholder', 'role', 'label', 'text']);

/** Resolve all variables once, including defaults, before any browser operation. */
export function nativeValues(draft: Draft, options: RunOptions): Record<string, unknown> {
  generate(draft);
  const defaults = Object.fromEntries(
    Object.entries(draft.variables)
      .filter(([, v]) => !v.secret)
      .map(([k, v]) => [k, v.default]),
  );
  return { ...defaults, ...options.vars };
}

/** Fill only explicitly provided variables; missing values never become empty input silently. */
export function nativeValue(value: unknown, vars: Record<string, unknown>): string {
  return String(value ?? '').replace(PLACEHOLDER, (_, key: string) => {
    if (!Object.hasOwn(vars, key) || vars[key] === undefined) throw new Error('Missing variable: ' + key);
    return String(vars[key]);
  });
}

/** Validate every enabled step, so a later unsupported action cannot leave a partially changed site. */
export function nativePreflight(draft: Draft, vars: Record<string, unknown>): void {
  for (const step of draft.steps.filter((s) => s.enabled)) {
    capability(step);
    for (const value of stepValues(step)) nativeValue(value, vars);
    validateKey(step, vars);
  }
}

/** Chords require a dedicated native parser; never type a chord's spelling as ordinary text. */
function validateKey(step: Step, vars: Record<string, unknown>): void {
  if (step.action === 'press_key') workflowKey(nativeValue(step.key, vars));
}

/** Missing capabilities are reported before any step reaches the browser. */
function capability(step: Step): void {
  const unsupported = !ACTIONS.has(step.action) || step.candidates.some((c) => !LOCATORS.has(c.kind));
  if (unsupported)
    throw new Error(
      `Unsupported native workflow capability at step ${step.id}: ${step.action}, frame or locator. Unknown action or locator kind.`,
    );
}

/** All user-substituted fields are validated before browser ownership is acquired. */
function stepValues(step: Step): unknown[] {
  const fields = [step.url, step.text, step.key, step.expected, step.file, step.option];
  return [...fields, ...step.frames, ...step.candidates.map((c) => c.value)];
}
