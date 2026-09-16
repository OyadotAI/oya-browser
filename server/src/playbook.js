/**
 * Playbooks: an ask() run frozen into steps that replay without the LLM.
 *
 * Steps carry the analyzer's stable element metadata (testId, DOM id, aria-label,
 * text, name), never the numeric id, which dies with each analysis. Every value that
 * was typed, picked or clicked is a `{{name}}` placeholder filled at replay, with the
 * recorded value kept as its default — so a playbook is a form, not a transcript, and
 * a run that passes nothing still does what was demonstrated. Passwords are the one
 * value with no default: they never leave the page. Replay re-analyzes and matches,
 * so it works on every provider. The Playwright code is an export to read or run
 * yourself; nothing here ever evaluates it.
 */

import { sendCommand } from './ws-handler.js';
import { runChat, lastRun, fill, FILTERS, pipesOf, selectOptionIn, uploadFileIn, isFileValue } from './chat-service.js';
import * as keyConfig from './key-config.js';

const NAME = /^[\w-]{1,64}$/;
const IDENT = /^[A-Za-z_]\w{0,39}$/;
const HAS_PLACEHOLDER = /\{\{\w+(?:\|[^}]*)?\}\}/;
const PAGE_CHANGING = new Set(['navigate', 'click', 'press_key']);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const fail = (status, message) => Object.assign(new Error(message), { status });
const same = (a, b) => String(a).trim().toLowerCase() === String(b).trim().toLowerCase();

/** The live element a recorded one corresponds to, or null. `text` overrides the label for a data-driven click. */
export function matchElement(el = {}, elements = [], text) {
  const pool = [...elements.filter((e) => e.visible), ...elements.filter((e) => !e.visible)];
  if (text !== undefined) return pool.find((e) => e.text && same(e.text, text)) || null;
  const by = (field, sameTag) => el[field] && pool.find((e) => e[field] === el[field] && (!sameTag || e.tag === el.tag));
  return by('testId') || by('domId') || by('ariaLabel', true)
    || (el.text && pool.find((e) => e.type === el.type && e.text === el.text))
    || by('name', true) || by('placeholder', true) || by('href') || null;
}

/** Actions a recording may contain — the same set the agent records, minus what cannot replay. */
const RECORDABLE = new Set(['navigate', 'click', 'type', 'select_option', 'upload_file', 'press_key', 'scroll']);
const EL_FIELDS = ['type', 'tag', 'text', 'domId', 'name', 'ariaLabel', 'testId', 'placeholder', 'href'];
const STEP_FIELDS = ['url', 'text', 'option', 'file', 'key', 'direction'];
const FIELDY = new Set(['input', 'textarea', 'editable', 'select']);
const MAX_STEPS = 500;
const MAX_LEN = 2000;

const sameEl = (a = {}, b = {}) => EL_FIELDS.every((k) => a[k] === b[k]);

/**
 * Steps recorded in a browser, cleaned up and checked before they become a playbook.
 * The caller is authenticated but the payload is built in a web page, so nothing
 * here trusts a shape: unknown actions, unknown fields and oversized strings are
 * dropped rather than stored and replayed later.
 */
export function sanitizeSteps(steps) {
  if (!Array.isArray(steps)) throw fail(400, 'steps must be an array');
  if (steps.length > MAX_STEPS) throw fail(400, `A recording is limited to ${MAX_STEPS} steps`);
  const str = (v) => (typeof v === 'string' ? v.slice(0, MAX_LEN) : typeof v === 'number' ? String(v) : undefined);

  const out = [];
  for (const raw of steps) {
    if (!raw || !RECORDABLE.has(raw.action)) continue;
    const step = { action: raw.action };
    if (raw.start) step.start = true;
    for (const k of STEP_FIELDS) { const v = str(raw[k]); if (v !== undefined) step[k] = v; }
    if (raw.amount !== undefined) step.amount = Math.min(Math.abs(Number(raw.amount)) || 0, 100000);
    if (raw.el && typeof raw.el === 'object') {
      const el = {};
      for (const k of EL_FIELDS) { const v = str(raw.el[k]); if (v) el[k] = v; }
      if (Object.keys(el).length) step.el = el;
    }
    if (step.action === 'navigate' && !/^https?:\/\//i.test(step.url || '')) continue;
    if (['click', 'type', 'select_option'].includes(step.action) && !step.el) continue;

    // Two sources can see the same navigation — the browser's own address bar and the
    // command the live view sent — and going there twice is a slower way to be nowhere new.
    if (step.action === 'navigate' && out[out.length - 1]?.action === 'navigate' && out[out.length - 1].url === step.url) continue;

    // A click into a field, then typing in it, is one action to a person and two
    // to the DOM. Replaying the click adds a step that can only go wrong.
    const prev = out[out.length - 1];
    if (step.action === 'type' && prev?.action === 'click' && FIELDY.has(prev.el?.type) && sameEl(prev.el, step.el)) out.pop();
    out.push(step);
  }
  if (!out.some((s) => s.action !== 'navigate')) throw fail(400, 'Nothing to save: the recording has no actions.');
  return out;
}

/** Every placeholder the steps use, in order of appearance. */
export const variablesOf = (steps) => [...new Set([...JSON.stringify(steps).matchAll(/\{\{(\w+)(?:\|[^}]*)?\}\}/g)].map((m) => m[1]))];

export const missingVariables = (pb, vars = {}) => variablesOf(pb.steps).filter((k) => vars[k] == null && pb.defaults?.[k] == null);

/** A JS expression for a recorded string; placeholders become `vars["name"]`, filtered ones `v(vars, "name", pipes)`. */
function expr(text) {
  if (!HAS_PLACEHOLDER.test(text)) return JSON.stringify(text);
  return '`' + text.split(/(\{\{\w+(?:\|[^}]*)?\}\})/).map((part) => {
    const m = part.match(/^\{\{(\w+)((?:\|[^}]*)?)\}\}$/);
    if (!m) return part.replace(/[\\`$]/g, '\\$&');
    return m[2] ? `\${v(vars, ${JSON.stringify(m[1])}, ${JSON.stringify(pipesOf(m[2]))})}` : `\${vars[${JSON.stringify(m[1])}]}`;
  }).join('') + '`';
}

/** Same precedence as matchElement, as a Playwright locator expression. */
function locator(step) {
  const s = JSON.stringify;
  const el = step.el || {};
  if (step.action === 'click' && HAS_PLACEHOLDER.test(el.text || '')) return `page.getByText(${expr(el.text)}, { exact: true })`;
  if (el.testId) return `page.getByTestId(${s(el.testId)})`;
  if (el.domId) return `page.locator(${s(`[id=${s(el.domId)}]`)})`;
  if (el.ariaLabel) return `page.getByLabel(${s(el.ariaLabel)}, { exact: true })`;
  if (el.text) return step.action === 'type' ? `page.getByLabel(${expr(el.text)})` : `page.getByText(${expr(el.text)}, { exact: true })`;
  if (el.name) return `page.locator(${s(`${el.tag || ''}[name=${s(el.name)}]`)})`;
  if (el.placeholder) return `page.getByPlaceholder(${s(el.placeholder)})`;
  if (el.href) return `page.locator(${s(`a[href=${s(el.href)}]`)})`;
  return `page.locator(${s(el.tag || 'body')}) /* no stable handle was recorded */`;
}

/** A Playwright module for the playbook. Values stay out of it; the caller passes `vars`. */
export function renderPlaywright(pb) {
  const s = JSON.stringify;
  const vars = variablesOf(pb.steps);
  const lines = pb.steps.map((step) => {
    switch (step.action) {
      case 'navigate': return `await page.goto(${expr(step.url)});`;
      case 'click': return `await ${locator(step)}.first().click();`;
      case 'type': return `await ${locator(step)}.first().fill(${expr(step.text ?? '')});`;
      case 'select_option': return `await ${locator(step)}.first().selectOption({ label: ${expr(step.option ?? '')} });`;
      // Not locator(step): the recorded handle is the visible button or drop zone, and
      // setInputFiles needs the input itself — which the replay finds from that anchor.
      case 'upload_file': return `await page.locator('input[type="file"]').first().setInputFiles(${expr(step.file ?? '')});`;
      case 'press_key': return `await page.keyboard.press(${s(step.key)});`;
      case 'wait': return `await page.waitForSelector(${s(step.selector)}${step.timeout ? `, { timeout: ${Number(step.timeout)} }` : ''});`;
      case 'scroll':
        if (step.direction === 'top') return 'await page.evaluate(() => window.scrollTo(0, 0));';
        if (step.direction === 'bottom') return 'await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));';
        return `await page.mouse.wheel(0, ${(step.direction === 'up' ? -1 : 1) * (Number(step.amount) || 500)});`;
      default: return `// skipped unknown step ${s(step.action)}`;
    }
  });
  const filtered = /\{\{\w+\|/.test(JSON.stringify(pb.steps));
  const uploads = pb.steps.filter((step) => step.action === 'upload_file').map((step) => variablesOf([step])[0]).filter(Boolean);
  return [
    `// Playbook ${s(pb.name)}, generated by Oya from an ask() run.`,
    `// vars: ${vars.length ? vars.join(', ') : '(none)'}`,
    // In this module they are paths, because that is what setInputFiles takes — the SDK's
    // play() wants a file() value for the same variable.
    ...(uploads.length ? [`// here ${uploads.join(', ')} ${uploads.length > 1 ? 'are file paths' : 'is a file path'}, not an Oya file() value`] : []),
    ...(filtered ? [
      'const FILTERS = {',
      ...Object.entries(FILTERS).map(([name, fn]) => `  ${name}: ${fn},`),
      '};',
      "const v = (vars, key, pipes) => pipes.reduce((s, [name, arg]) => (FILTERS[name] ? FILTERS[name](s, arg) : s), String(vars[key] ?? ''));",
    ] : []),
    `export default async function run(page, vars = {}) {`,
    ...lines.map((l) => `  ${l}`),
    `}`,
    '',
  ].join('\n');
}

const describe = (pb) => ({ name: pb.name, variables: variablesOf(pb.steps), defaults: pb.defaults || {}, steps: pb.steps.length, code: renderPlaywright(pb) });

const CAMEL = (s) => String(s).trim().toLowerCase()
  .replace(/[^a-z0-9]+(.)?/g, (_, c) => (c ? c.toUpperCase() : ''))
  .slice(0, 40);

/**
 * The variable name a step's value gets, from whatever the element says about itself.
 * Never a bare `field`: a form of anonymous boxes is one nobody can fill, so a value
 * with no handle to read is named after what it is and the step it was at.
 */
function nameFor(step, i) {
  const el = step.el || {};
  const kind = CAMEL(el.type || el.tag || '') || 'field';
  const from = step.action === 'click'
    ? [el.text, el.ariaLabel, el.testId, el.domId, el.name]
    : [el.name, el.domId, el.ariaLabel, el.placeholder, el.text, el.testId];
  for (const raw of from) {
    const name = raw && CAMEL(raw);
    if (!name) continue;
    if (IDENT.test(name)) return name;
    // "2024" or "1st line" is a fine name once it is told what it names.
    const prefixed = `${kind}${name[0].toUpperCase()}${name.slice(1)}`.slice(0, 40);
    if (IDENT.test(prefixed)) return prefixed;
  }
  return `${kind}${i + 1}`;
}

/**
 * Every value a person typed, picked or clicked becomes a `{{name}}`, with the
 * recorded value as its default — so a replay that passes nothing behaves exactly as
 * it was recorded, and every value is an input the caller can override. Values that
 * already carry a placeholder (an ask() run, a masked password) are left alone, and
 * so are navigate URLs: the addresses are the flow, not its data.
 *
 * ponytail: names come from DOM handles, so a field the page never labelled lands on
 * what it is and where — `input6`. A rename step in the record dialog if that bites.
 */
export function templateValues(pb) {
  const taken = new Map();   // name -> the value it was first given
  pb.labels = [];            // the click-label ones: a button name is not task data
  for (const [i, step] of pb.steps.entries()) {
    const key = step.action === 'type' ? 'text' : step.action === 'select_option' ? 'option' : null;
    // An icon button records no text: nothing to name a variable after, and nothing
    // for a replay to match it on, so it stays the element handle it already is.
    const value = key ? step[key] : step.action === 'click' ? step.el?.text : null;
    if (!value || HAS_PLACEHOLDER.test(value)) continue;

    const base = nameFor(step, i);
    let name = base;
    // One field typed into twice is one input. Two fields that derive the same name are two,
    // even when the person happened to type the same thing into both.
    for (let n = 2; taken.has(name) && taken.get(name) !== value; n++) name = `${base}${n}`;
    taken.set(name, value);

    if (key) step[key] = `{{${name}}}`;
    else { step.el.text = `{{${name}}}`; pb.labels.push(name); }
    pb.defaults[name] = value;
    // A healing agent reads the prompt, so the data in it has to travel too. Only
    // what was typed or picked: substituting a button label mangles the description.
    if (key && pb.prompt && value.length >= 3) pb.prompt = pb.prompt.split(value).join(`{{${name}}}`);
  }
  return pb;
}

/** `run` defaults to the browser's last ask(); a recording passes its own steps in. */
export async function create(apiKey, browserId, name, run = lastRun(browserId)) {
  if (typeof name !== 'string' || !NAME.test(name)) throw fail(400, 'Playbook name must be 1-64 letters, digits, _ or -');
  if (!run?.steps.some((s) => s.action !== 'navigate')) throw fail(409, 'Nothing to save: run ask() on this browser first.');

  // Which keys were secrets, so a healing replay keeps them hidden from the model.
  const pb = { name, prompt: run.prompt, steps: structuredClone(run.steps), defaults: {}, secrets: run.secrets || [], createdAt: new Date().toISOString() };
  templateValues(pb);
  await keyConfig.savePlaybook(apiKey, name, pb);
  return describe(pb);
}

/** Make a healed draft the playbook. */
export async function promote(apiKey, name) {
  const draft = keyConfig.getPlaybook(apiKey, `${name}:draft`);
  if (!draft) throw fail(404, `No healed draft for ${name}`);
  const { healedAt, healedFrom, ...pb } = draft;
  await keyConfig.savePlaybook(apiKey, name, { ...pb, name, promotedAt: new Date().toISOString() });
  await keyConfig.deletePlaybook(apiKey, `${name}:draft`);
  return describe({ ...pb, name });
}

/** Every playbook, newest first, each with the healed draft waiting for review. */
export function list(apiKey) {
  const all = keyConfig.listPlaybooks(apiKey);
  const drafts = new Map(all.filter((p) => p.name.endsWith(':draft')).map((p) => [p.name.slice(0, -':draft'.length), p]));
  return all.filter((p) => !p.name.endsWith(':draft'))
    .map((pb) => {
      const draft = drafts.get(pb.name);
      return {
        ...describe(pb),
        createdAt: pb.createdAt || null,
        promotedAt: pb.promotedAt || null,
        draft: draft ? { ...describe(draft), healedAt: draft.healedAt, healedFrom: draft.healedFrom } : null,
      };
    })
    .sort((a, b) => String(b.createdAt).localeCompare(String(a.createdAt)));
}

/** Delete a playbook and its draft, or just a draft when named `<name>:draft`. */
export async function remove(apiKey, name) {
  if (!keyConfig.getPlaybook(apiKey, name)) throw fail(404, `No playbook named ${name}`);
  await keyConfig.deletePlaybook(apiKey, name);
  if (!name.endsWith(':draft')) await keyConfig.deletePlaybook(apiKey, `${name}:draft`);
}

async function command(browserId, action, params = {}, timeout) {
  const r = await sendCommand(browserId, action, params, timeout);
  if (!r.ok) throw new Error(r.error || `${action} failed`);
  return r.data;
}

async function find(browserId, el, text) {
  // Pages settle after a click or navigation; give the element a few seconds to show up.
  for (let attempt = 0; attempt < 5; attempt++) {
    const { elements } = await command(browserId, 'analyze');
    const match = matchElement(el, elements, text);
    if (match) return match;
    await sleep(1000);
  }
  throw new Error(`no element matching ${JSON.stringify(el?.text ?? el?.domId ?? el?.name ?? '')}`);
}

async function runStep(browserId, step, values, defaults = {}) {
  switch (step.action) {
    case 'navigate': return command(browserId, 'navigate', { url: fill(step.url, values) }, 90_000);
    case 'press_key': return command(browserId, 'press_key', { key: step.key });
    case 'scroll': return command(browserId, 'scroll', { direction: step.direction, amount: step.amount });
    case 'wait': return command(browserId, 'wait', { selector: step.selector, timeout: step.timeout });
    case 'click': {
      const key = (step.el?.text || '').match(/^\{\{(\w+)\}\}$/)?.[1];
      const filled = key ? fill(step.el.text, values) : undefined;
      // Still the recorded label: match on the full precedence, testId first. Changed —
      // a data-driven option, an insurer, a plan — and the old DOM id belonged to
      // another choice, so the value is the only handle left.
      const byValue = filled !== undefined && filled !== defaults[key] ? filled : undefined;
      const el = await find(browserId, filled === undefined ? step.el : { ...step.el, text: filled }, byValue);
      return command(browserId, 'click', { selector: `[data-ac-id="${el.id}"]` });
    }
    case 'type': {
      const el = await find(browserId, step.el);
      return command(browserId, 'type', { selector: `[data-ac-id="${el.id}"]`, text: fill(step.text ?? '', values) });
    }
    case 'select_option': {
      const el = await find(browserId, step.el);
      const r = await selectOptionIn(browserId, el, fill(step.option ?? '', values));
      if (!r.ok) throw new Error(`${r.error}${r.options ? ` (options: ${r.options.join(' | ')})` : ''}`);
      return r;
    }
    case 'upload_file': {
      // Only the variable name was recorded, so the replay brings its own file.
      const key = (step.file || '').match(/^\{\{(\w+)\}\}$/)?.[1];
      const f = key && values[key];
      if (!isFileValue(f)) throw new Error(`${key || 'this upload'} needs a file() value`);
      const el = step.el ? await find(browserId, step.el) : null;
      const r = await uploadFileIn(browserId, el, f);
      if (!r.ok) throw new Error(r.error);
      return r;
    }
    default: throw new Error(`unknown step ${step.action}`);
  }
}

/**
 * Replay without the LLM. `checkpoint` runs after page-changing steps (CAPTCHA,
 * MFA). When a step no longer fits the page: autoHeal off throws; on, the agent
 * finishes the task and its steps are saved as the draft `<name>:draft`.
 */
export async function play(apiKey, browserId, pb, vars = {}, { autoHeal = true, checkpoint, requestHuman } = {}) {
  const values = { ...pb.defaults, ...vars };
  const total = pb.steps.length;
  for (let i = 0; i < total; i++) {
    const step = pb.steps[i];
    try {
      await runStep(browserId, step, values, pb.defaults || {});
    } catch (err) {
      if (!autoHeal) throw fail(422, `Step ${i + 1} of ${total} (${step.action}) failed: ${err.message}`);
      return heal(apiKey, browserId, pb, i, err, values, { checkpoint, requestHuman });
    }
    if (PAGE_CHANGING.has(step.action)) await checkpoint?.();
  }
  return { steps: total, total, fellBack: false };
}

async function heal(apiKey, browserId, pb, i, err, values, hooks) {
  const total = pb.steps.length;
  const task = `${pb.prompt}\n\nA recorded playbook already did ${i} of ${total} steps of this task, then failed (${err.message}). Look at the page and finish the task from where it is.`;
  let result;
  try {
    const hidden = new Set(pb.secrets || []);
    // Button and link labels are how the flow was clicked, not what it was about. Handing
    // them over as data makes redact() rewrite "Sign in" to {{signIn}} in the page the
    // agent is reading, which is the opposite of help.
    const ignored = new Set([...hidden, ...(pb.labels || [])]);
    const data = Object.fromEntries(Object.entries(values).filter(([k]) => !ignored.has(k)));
    const secrets = Object.fromEntries(Object.entries(values).filter(([k]) => hidden.has(k)));
    result = await runChat(browserId, [{ role: 'user', content: task }], { apiKey, data, secrets, ...hooks });
    if (result.limited) throw new Error('the agent hit its step limit');
    if (/(^|\n)\s*FAILED:/i.test(result.text)) throw new Error(result.text.trim());
  } catch (healErr) {
    if (!hooks.requestHuman) throw healErr;
    await hooks.requestHuman({ reason: 'heal_failed', message: `Replay broke at step ${i + 1} and the agent could not finish (${healErr.message}). Finish it in the live view, then respond.` });
    return { steps: i, total, fellBack: true, healed: false, text: 'Finished by a person.' };
  }
  const healed = (lastRun(browserId)?.steps || []).filter((s) => !s.start);
  const draft = { ...pb, steps: [...pb.steps.slice(0, i), ...healed], healedFrom: i, healedAt: new Date().toISOString() };
  await keyConfig.savePlaybook(apiKey, `${pb.name}:draft`, draft);
  return { steps: i, total, fellBack: true, healed: true, draft: `${pb.name}:draft`, text: result.text };
}
