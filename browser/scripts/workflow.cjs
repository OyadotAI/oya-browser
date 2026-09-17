const { randomUUID } = require('node:crypto');
const ACTIONS = new Set(['navigate', 'click', 'type', 'select_option', 'upload_file', 'press_key', 'scroll', 'wait', 'assert_visible', 'assert_text', 'assert_value', 'assert_url', 'checkpoint']);
const clone = value => structuredClone(value);
function candidates(el = {}) {
  const out = [];
  if (el.testId) out.push({ kind: 'testId', value: el.testId });
  if (el.role && (el.ariaLabel || el.text)) out.push({ kind: 'role', role: el.role, value: el.ariaLabel || el.text });
  if (el.ariaLabel) out.push({ kind: 'label', value: el.ariaLabel });
  if (el.text) out.push({ kind: ['input', 'textarea', 'editable'].includes(el.type) ? 'label' : 'text', value: el.text });
  if (el.placeholder) out.push({ kind: 'placeholder', value: el.placeholder });
  if (el.domId) out.push({ kind: 'css', value: `[id=${JSON.stringify(el.domId)}]` });
  if (el.name) out.push({ kind: 'css', value: `[name=${JSON.stringify(el.name)}]` });
  if (el.href) out.push({ kind: 'css', value: `a[href=${JSON.stringify(el.href)}]` });
  return out;
}
const STRING_FIELDS = ['action', 'url', 'text', 'option', 'file', 'key', 'direction', 'expected', 'captureIssue'];
function normalizeStep(raw) {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) throw new Error('Invalid step');
  const step = {};
  for (const key of STRING_FIELDS) if (raw[key] !== undefined) { if (typeof raw[key] !== 'string' || raw[key].length > 16000) throw new Error('Invalid step ' + key); step[key] = raw[key]; }
  step.id = typeof raw.id === 'string' && /^[\w:.-]{1,150}$/.test(raw.id) ? raw.id : randomUUID();
  step.enabled = raw.enabled !== false; step.breakpoint = raw.breakpoint === true;
  step.candidates = (Array.isArray(raw.candidates) ? raw.candidates : candidates(raw.el)).slice(0, 12).map(c => {
    if (!c || !['testId', 'role', 'label', 'text', 'placeholder', 'css'].includes(c.kind) || typeof c.value !== 'string' || c.value.length > 4000 || c.kind === 'role' && (typeof c.role !== 'string' || !/^[a-z]{1,40}$/.test(c.role))) throw new Error('Invalid locator candidate');
    return { kind: c.kind, value: c.value, ...(c.kind === 'role' ? { role: c.role } : {}) };
  });
  if (raw.el && typeof raw.el === 'object') {
    step.el = {}; for (const key of ['type', 'tag', 'text', 'domId', 'name', 'placeholder', 'ariaLabel', 'testId', 'role', 'href']) if (typeof raw.el[key] === 'string') step.el[key] = raw.el[key].slice(0, 4000);
  }
  step.tab = typeof raw.tab === 'string' ? raw.tab.slice(0, 100) : 'main';
  if (raw.frames && (!Array.isArray(raw.frames) || raw.frames.some(f => typeof f !== 'string' || f.length > 4000) || raw.frames.length > 10)) throw new Error('Invalid frame path');
  step.frames = raw.frames || [];
  step.timeout = Math.max(500, Math.min(Number(raw.timeout) || 15000, 90000));
  if (!Number.isFinite(step.timeout)) step.timeout = 15000;
  if (raw.amount !== undefined) step.amount = Math.min(100000, Math.abs(Number(raw.amount)) || 500);
  if (raw.t) step.t = Number(raw.t); if (raw.start) step.start = true;
  return step;
}
function normalizeDraft(raw = {}) {
  if (raw.schemaVersion && raw.schemaVersion !== 2) throw new Error(`Unsupported recording version ${raw.schemaVersion}`);
  if (!Array.isArray(raw.steps || []) || (raw.steps || []).length > 500) throw new Error('A draft supports at most 500 steps');
  if (raw.variables && (typeof raw.variables !== 'object' || Array.isArray(raw.variables) || Object.keys(raw.variables).length > 100)) throw new Error('Invalid variables');
  const variables = {};
  for (const [name, config] of Object.entries(raw.variables || {})) {
    if (!/^[A-Za-z_]\w{0,63}$/.test(name) || ['__proto__', 'constructor', 'prototype'].includes(name) || !config || typeof config !== 'object') throw new Error('Invalid variable name');
    variables[name] = { secret: config.secret === true };
    if (!config.secret && config.default !== undefined) { if (typeof config.default !== 'string' || config.default.length > 16000) throw new Error('Invalid variable default'); variables[name].default = config.default; }
  }
  if (!Array.isArray(raw.secrets || []) || (raw.secrets || []).some(name => typeof name !== 'string' || !/^[A-Za-z_]\w{0,63}$/.test(name) || ['__proto__', 'constructor', 'prototype'].includes(name))) throw new Error('Invalid secret variable');
  const draft = { schemaVersion: 2, id: raw.id || randomUUID(), revision: Number(raw.revision) || 0, name: String(raw.name || 'Untitled workflow').slice(0, 64), description: String(raw.description || '').slice(0, 2000), steps: (raw.steps || []).map(normalizeStep), variables, secrets: [...new Set([...(raw.secrets || []), ...Object.keys(variables).filter(k => variables[k].secret)])], createdAt: raw.createdAt || Date.now(), updatedAt: raw.updatedAt || Date.now(), phase: raw.phase === 'recording' ? 'recording' : 'paused' };
  for (const key of ['repairedFrom', 'publishedAt', 'run']) if (raw[key] !== undefined) draft[key] = clone(raw[key]);
  for (const name of draft.secrets) { draft.variables[name] = { secret: true }; }
  const ids = new Set(); for (const step of draft.steps) { if (ids.has(step.id)) throw new Error('Step IDs must be unique'); ids.add(step.id); }
  return draft;
}
function issues(draft) {
  const result = [];
  for (const step of draft.steps.filter(step => step.enabled)) {
    if (!ACTIONS.has(step.action)) result.push({ stepId: step.id, message: `Unsupported interaction: ${step.action}. Replace or disable this step.` });
    if (['click', 'type', 'select_option', 'upload_file', 'assert_visible', 'assert_text', 'assert_value', 'wait'].includes(step.action) && !step.candidates.length) result.push({ stepId: step.id, message: 'Pick a target before validation.' });
    if (step.action === 'navigate' && !/^https?:\/\//i.test(step.url || '')) result.push({ stepId: step.id, message: 'Navigation requires an HTTP or HTTPS URL.' });
    if (step.candidates.some(c => !c.value.trim())) result.push({ stepId: step.id, message: 'Target cannot be empty.' });
    if (step.captureIssue) result.push({ stepId: step.id, message: step.captureIssue });
  }
  return result;
}
function variableNames(draft) {
  return [...new Set([...JSON.stringify(draft.steps).matchAll(/\{\{([A-Za-z_]\w*)\}\}/g)].map(match => match[1]))];
}
function locatorCode(candidate, owner = 'p', value = JSON.stringify(candidate?.value)) {
  const c = candidate;
  if (!c || !['testId', 'role', 'label', 'text', 'placeholder', 'css'].includes(c.kind)) throw new Error('Pick a supported locator');
  return c.kind === 'css' ? `${owner}.locator(${value})` : c.kind === 'role' ? `${owner}.getByRole(${JSON.stringify(c.role)}, {name: ${value}, exact:true})` : `${owner}.${({ testId: 'getByTestId', label: 'getByLabel', text: 'getByText', placeholder: 'getByPlaceholder' })[c.kind]}(${value}${c.kind === 'testId' ? '' : ', {exact:true}'})`;
}
function generate(input) {
  const draft = normalizeDraft(input), problems = issues(draft);
  if (problems.length) throw new Error(problems.map(issue => issue.message).join('\n'));
  if (variableNames(draft).some(name => ['__proto__', 'constructor', 'prototype'].includes(name))) throw new Error('Reserved variable name');
  const defaults = Object.fromEntries(Object.entries(draft.variables).filter(([, v]) => !v.secret && v.default !== undefined).map(([key, v]) => [key, v.default]));
  const lines = [], mapping = {};
  const add = text => lines.push(text);
  add('// Generated by Oya. Optional hooks power debugging; no Oya runtime is required.');
  add('export default async function run(page, vars = {}, hooks = {}) {');
  add("  const expect = hooks.expect || (await import('@playwright/test')).expect;");
  add(`  vars = {...${JSON.stringify(defaults)}, ...vars};`);
  add(`  for (const key of ${JSON.stringify(variableNames(draft))}) if (!Object.prototype.hasOwnProperty.call(vars, key) || vars[key] === undefined) throw new Error('Missing variable: ' + key);`);
  add("  const value = x => String(x ?? '').replace(/\\{\\{([A-Za-z_]\\w*)\\}\\}/g, (_, key) => String(vars[key] ?? '')); ");
  add("  const pages = hooks.pages || new Map([['main', page]]); let p = page;");
  for (const step of draft.steps.filter(step => step.enabled)) {
    mapping[step.id] = lines.length + 1;
    add(`  if (!hooks.shouldRun || hooks.shouldRun(${JSON.stringify(step.id)})) {`);
    add(`  // ${step.action}`);
    add(`  if (!pages.has(${JSON.stringify(step.tab)})) pages.set(${JSON.stringify(step.tab)}, await page.context().newPage());`);
    add(`  p = pages.get(${JSON.stringify(step.tab)});`);
    add('  try {');
    add(`    await hooks.beforeStep?.(${JSON.stringify(step.id)}, p);`);
    let owner = 'p';
    for (const frame of step.frames) owner += `.frameLocator(${JSON.stringify(frame)})`;
    const locator = step.candidates[0] ? locatorCode(step.candidates[0], owner, `value(${JSON.stringify(step.candidates[0].value)})`) : null;
    const timeout = `{timeout:${step.timeout}}`;
    const val = key => `value(${JSON.stringify(step[key] ?? '')})`;
    let command;
    switch (step.action) {
      case 'navigate': command = `await p.goto(${val('url')}, ${timeout});`; break;
      case 'click': command = `await ${locator}.click(${timeout});`; break;
      case 'type': command = `await ${locator}.fill(${val('text')}, ${timeout});`; break;
      case 'select_option': command = `await ${locator}.selectOption({label:${val('option')}}, ${timeout});`; break;
      case 'upload_file': command = `await ${locator}.setInputFiles(${val('file')}, ${timeout});`; break;
      case 'press_key': command = `await p.keyboard.press(${val('key')});`; break;
      case 'scroll': command = `await p.mouse.wheel(0, ${(step.direction === 'up' ? -1 : 1) * (Number(step.amount) || 500)});`; break;
      case 'wait': command = `await ${locator}.waitFor({state:'visible', timeout:${step.timeout}});`; break;
      case 'assert_visible': command = `await expect(${locator}).toBeVisible(${timeout});`; break;
      case 'assert_text': command = `await expect(${locator}).toHaveText(${val('expected')}, ${timeout});`; break;
      case 'assert_value': command = `await expect(${locator}).toHaveValue(${val('expected')}, ${timeout});`; break;
      case 'assert_url': command = `await expect(p).toHaveURL(${val('expected')}, ${timeout});`; break;
      case 'checkpoint': command = "if (!hooks.checkpoint) throw new Error('This workflow requires a human checkpoint'); await hooks.checkpoint(p);"; break;
    }
    mapping[step.id] = lines.length + 1;
    add('    ' + command);
    add(`    await hooks.afterStep?.(${JSON.stringify(step.id)}, p);`);
    add(`  } catch (error) { await hooks.failedStep?.(${JSON.stringify(step.id)}, error, p); throw error; }`);
    add('  }');
  }
  add('}');
  return { code: lines.join('\n') + '\n', mapping, issues: problems };
}
module.exports = { ACTIONS, candidates, normalizeStep, normalizeDraft, issues, variableNames, locatorCode, generate };
