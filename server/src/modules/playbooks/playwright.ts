/**
 * The Playwright export: a playbook as a module to read or run yourself. Nothing
 * here evaluates it. Locators follow matchElement's precedence.
 */
import workflow from '../../../../browser/scripts/workflow.cjs';

const { handlesOf, withoutLiveCount, rawTargetOf, volatileTarget } = workflow as any;

import { FILTERS, pipesOf } from '../agent/chat.ts';
import { DEFAULT_SCROLL_PX, FIND_ATTEMPTS, FIND_RETRY_MS, WORKFLOW_SCHEMA } from './constants.ts';
import { HAS_PLACEHOLDER, variablesOf } from './variables.ts';

/** Renders one step as a line of Playwright. */
type LineFor = (step: any) => string;
/** Renders a recorded element as a Playwright locator. */
type LocatorFor = (el: any, step: any) => string;

/** JSON as a JavaScript literal. */
const s = JSON.stringify;

/** A JS expression for a recorded string; placeholders become `vars["name"]`, filtered ones `v(vars, "name", pipes)`. */
function expr(text) {
  if (!HAS_PLACEHOLDER.test(text)) return JSON.stringify(text);
  return `\`${text
    .split(/(\{\{\w+(?:\|[^}]*)?\}\})/)
    .map(exprPart)
    .join('')}\``;
}

/** One piece of a template literal: escaped text, or a placeholder's interpolation. */
function exprPart(part) {
  const m = part.match(/^\{\{(\w+)((?:\|[^}]*)?)\}\}$/);
  if (!m) return part.replace(/[\\`$]/g, '\\$&');
  return m[2]
    ? `\${v(vars, ${JSON.stringify(m[1])}, ${JSON.stringify(pipesOf(m[2]))})}`
    : `\${vars[${JSON.stringify(m[1])}]}`;
}

/** Each recorded handle and its locator, in matchElement's order. */
/**
 * How each handle reads as a Playwright locator. Which handle to reach for first
 * is not decided here, `HANDLES` decides, so this export and the live replay
 * always aim at the same element.
 */
const AS_LOCATOR: Record<string, LocatorFor> = {
  testId: (el) => `page.getByTestId(${s(el.testId)})`,
  // Only where the target will be written the same way again: a link carrying the
  // visit's own qid matches nothing on the next run, so a lower handle takes it.
  href: (el) => (volatileTarget(rawTargetOf(el)) ? null : `page.locator(${s(`a[href=${s(rawTargetOf(el))}]`)})`),
  domId: (el) => `page.locator(${s(`[id=${s(el.domId)}]`)})`,
  ariaLabel: (el) => `page.getByLabel(${s(withoutLiveCount(el.ariaLabel))}, { exact: true })`,
  // Already a Playwright selector: the analyzer writes it as one, anchored on the
  // container where this name occurs once ('[data-row="7"] button:text-is("View")').
  scoped: (el) => `page.locator(${s(el.scoped)})`,
  text: (el, step) =>
    step.action === 'type' ? `page.getByLabel(${expr(el.text)})` : `page.getByText(${expr(el.text)}, { exact: true })`,
  name: (el) => `page.locator(${s(`${el.tag || ''}[name=${s(el.name)}]`)})`,
  placeholder: (el) => `page.getByPlaceholder(${s(el.placeholder)})`,
  path: (el) => `page.locator(${s(el.path)})`,
};

/**
 * The first recorded handle that renders as a locator at all, in the shared order of
 * trust. A handle whose renderer declines, a link target that will read differently
 * next visit, is passed over, exactly as it is in the live replay.
 */
function locatorFor(el, step) {
  return (
    handlesOf(el)
      .map((h) => AS_LOCATOR[h.kind]?.(el, step))
      .find(Boolean) || null
  );
}

/** The recorded element as a Playwright locator, by the shared order of trust. */
function locator(step) {
  const el = step.el || {};
  if (step.action === 'click' && HAS_PLACEHOLDER.test(el.text || ''))
    return `page.getByText(${expr(el.text)}, { exact: true })`;
  // Unaimable steps never reach here, stepLine refuses them, so the floor is only
  // for an action that needs no element.
  return locatorFor(el, step) || `page.locator(${s(el.tag || 'body')})`;
}

/** Actions that cannot run without knowing which element they act on. */
const NEEDS_ELEMENT = new Set(['click', 'double_click', 'type', 'select_option']);

/** Whether this step can be aimed at anything on a later page. */
function aimable(step) {
  const el = step.el || {};
  if (step.unaimable) return false;
  // A data-driven click aims by the value it is given, not by a recorded handle.
  if (step.action === 'click' && HAS_PLACEHOLDER.test(el.text || '')) return true;
  return Boolean(locatorFor(el, step));
}

/**
 * A step nothing can aim. It used to render as a click on `page.locator("body")`,
 * which runs, does nothing, and lets the rest of the script carry on as though the
 * step had worked. An export that cannot reach its element says so and stops.
 */
function refusal(step) {
  const what = step.el?.tag ? `a <${step.el.tag}>` : 'an element';
  const why = `Cannot replay this ${step.action}: no stable handle was recorded for ${what}. Re-record this step.`;
  return `throw new Error(${s(why)});`;
}

/** A scroll step: to the top, to the bottom, or by an amount. */
function scrollLine(step) {
  if (step.direction === 'top') return 'await page.evaluate(() => window.scrollTo(0, 0));';
  if (step.direction === 'bottom') return 'await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));';
  return `await page.mouse.wheel(0, ${(step.direction === 'up' ? -1 : 1) * (Number(step.amount) || DEFAULT_SCROLL_PX)});`;
}

/** Action → its line of Playwright. */
const LINES: Record<string, LineFor> = {
  navigate: (step) => `await page.goto(${expr(step.url)});`,
  click: (step) => `await ${locator(step)}.first().click();`,
  type: (step) => `await ${locator(step)}.first().fill(${expr(step.text ?? '')});`,
  select_option: (step) => `await ${locator(step)}.first().selectOption({ label: ${expr(step.option ?? '')} });`,
  // Not locator(step): the recorded handle is the visible button or drop zone, and
  // setInputFiles needs the input itself, which the replay finds from that anchor.
  upload_file: (step) => `await page.locator('input[type="file"]').first().setInputFiles(${expr(step.file ?? '')});`,
  press_key: (step) => `await page.keyboard.press(${s(step.key)});`,
  // The handler installed below already answered it; without this the recorded
  // step would read as unknown and the export would carry a stray comment.
  handle_dialog: () => `// dialog answered by the handler above`,
  wait: (step) =>
    `await page.waitForSelector(${s(step.selector)}${step.timeout ? `, { timeout: ${Number(step.timeout)} }` : ''});`,
  scroll: scrollLine,
  hover: (step) => `await ${locator(step)}.first().hover();`,
  go_back: () => 'await page.goBack();',
  go_forward: () => 'await page.goForward();',
  reload: () => 'await page.reload();',
  double_click: (step) =>
    step.el
      ? `await ${locator(step)}.first().dblclick();`
      : `await page.mouse.dblclick(${Number(step.x)}, ${Number(step.y)});`,
  // A point, not a handle. Exported so the run is complete, flagged so whoever
  // reads the export knows this is the line that breaks when a layout moves.
  click_coordinates: (step) =>
    `await page.mouse.click(${Number(step.x)}, ${Number(step.y)}); // recorded as a point: re-record if the layout changes`,
  keyboard_type: (step) => `await page.keyboard.type(${expr(step.text ?? '')});`,
  open_tab: (step) => `page = await context.newPage();\n  await page.goto(${expr(step.url ?? step.tabUrl ?? '')});`,
  // The tab is found by where it went, since a tab index is not stable across runs
  // and an SSO url carries a fresh token every time.
  switch_tab: (step) => `page = await tabAt(context, ${s(pathOf(step.tabUrl))});`,
  close_tab: () => `await page.close();\n  page = context.pages()[context.pages().length - 1];`,
};

/** The origin and path of a url, which is what identifies a tab across runs. */
function pathOf(url) {
  try {
    const u = new URL(url);
    return `${u.origin}${u.pathname}`;
  } catch {
    return String(url ?? '');
  }
}

/** One step as Playwright, or a comment for an action the export does not know. */
function stepLine(step) {
  const known = typeof step.action === 'string' && Object.hasOwn(LINES, step.action);
  if (!known) return `// skipped unknown step ${s(step.action)}`;
  if (NEEDS_ELEMENT.has(step.action) && !aimable(step)) return refusal(step);
  return LINES[step.action](step);
}

/** A Playwright module with non-secret defaults and caller-supplied overrides. */
export function renderPlaywright(pb) {
  if (pb.schemaVersion === WORKFLOW_SCHEMA) return workflow.generate(pb).code;
  const vars = variablesOf(pb.steps);
  return [...header(pb, vars), ...runFunction(pb, vars), ''].join('\n');
}

/** The module's opening comments and, when steps use filters, the filter helpers. */
function header(pb, vars) {
  return [
    `// Playbook ${s(pb.name)}, generated by Oya.`,
    `// vars: ${vars.length ? vars.join(', ') : '(none)'}`,
    ...uploadNote(pb.steps),
    ...filterHelpers(pb.steps),
  ];
}

/**
 * In this module they are paths, because that is what setInputFiles takes, the SDK's
 * play() wants a file() value for the same variable.
 */
function uploadNote(steps) {
  const uploads = steps
    .filter((step) => step.action === 'upload_file')
    .map((step) => variablesOf([step])[0])
    .filter(Boolean);
  if (!uploads.length) return [];
  const what = uploads.length > 1 ? 'are file paths' : 'is a file path';
  return [`// here ${uploads.join(', ')} ${what}, not an Oya file() value`];
}

/** The filter functions and `v()` helper, only when a placeholder uses a filter. */
function filterHelpers(steps) {
  if (!/\{\{\w+\|/.test(JSON.stringify(steps))) return [];
  return [
    'const FILTERS = {',
    ...Object.entries(FILTERS).map(([name, fn]) => `  ${name}: ${fn},`),
    '};',
    "const v = (vars, key, pipes) => pipes.reduce((s, [name, arg]) => (FILTERS[name] ? FILTERS[name](s, arg) : s), String(vars[key] ?? ''));",
  ];
}

/** Actions whose exported line needs the browser context and the tab finder. */
const TAB_STEPS = new Set(['open_tab', 'switch_tab', 'close_tab']);

/**
 * Finds a tab by where it went. An SSO handoff opens its tab a moment after the
 * click that starts it, and its url carries a fresh token each run, so this waits
 * and matches on origin and path rather than on an index or a whole url.
 */
const TAB_FINDER = [
  `  const context = page.context();`,
  `  const tabAt = async (context, target) => {`,
  `    for (let attempt = 0; attempt < ${FIND_ATTEMPTS}; attempt++) {`,
  `      const found = context.pages().find((p) => p.url().startsWith(target));`,
  `      if (found) return found;`,
  `      await page.waitForTimeout(${FIND_RETRY_MS});`,
  `    }`,
  `    throw new Error('no tab at ' + target);`,
  `  };`,
];

/** The tab finder, added only when the run used more than one tab. */
function tabPreamble(steps) {
  return steps.some((step) => TAB_STEPS.has(step.action)) ? TAB_FINDER : [];
}

/** The exported run function: defaults, the dialog handler, then the steps. */
function runFunction(pb, vars) {
  return [
    `export default async function run(page, vars = {}) {`,
    `  vars = { ...${s(exportDefaults(pb, vars))}, ...vars };`,
    ...dialogHandler(pb.steps),
    ...tabPreamble(pb.steps),
    ...pb.steps.map((step) => `  ${stepLine(step)}`),
    `}`,
  ];
}

/** Recorded defaults the steps use, minus secrets. */
function exportDefaults(pb, vars) {
  const secrets = new Set(pb.secrets || []);
  return Object.fromEntries(
    Object.entries(pb.defaults || {}).filter(([key]) => vars.includes(key) && !secrets.has(key)),
  );
}

/**
 * Playwright blocks the page on a dialog nobody answers. The recording shows
 * which way this flow went, so the export answers the same way.
 */
function dialogHandler(steps) {
  const dialogs = steps.filter((step) => step.action === 'handle_dialog' && step.accept !== false);
  if (!dialogs.length) return [];
  const answered = dialogs.find((step) => step.prompt_text);
  return [`  page.on('dialog', (d) => d.accept(${answered ? s(answered.prompt_text) : ''}));`];
}
