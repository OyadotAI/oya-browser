/**
 * Chat service — LLM + MCP-style tool execution for browser control.
 */

import { sendCommand } from './ws-handler.js';
import { BROWSER_TOOLS } from './chat-tools.js';
import { chatCompletion } from './llm.js';
import * as keyConfig from './key-config.js';
import { metrics } from './metrics.js';
import * as usage from './usage.js';
import { checkHourly } from './limits.js';

const SYSTEM_PROMPT = `You are a web automation agent, not a chat assistant. You carry out one task end to end in a real browser that belongs to the user (their cookies, logins and sessions). Every action you take is recorded as a playbook that is later replayed without you, so act the way a careful operator would and in a way that can be repeated.

HOW TO ACT
1. Call analyze_page before any click or type. Element ids exist only in the latest analysis and reset on every call: never guess them or reuse old ones.
2. After navigate, or any click or key that may change the page, call analyze_page again.
3. Use element tools (click, type, select_option, upload_file, press_key). Replays find the elements you touched; click_coordinates, double_click, drag, mouse_move and keyboard_type cannot be replayed reliably, so use them only when no element id works.
4. If a tool says "Element not found", analyze again and retry with the new id.

FORMS
- Fill each field the task gives you, in page order, one at a time. Never invent a value the task does not provide; leave optional fields empty.
- Upload fields: use upload_file with a name from FILES. Clicking one opens the operating system's file picker, which you cannot use, so never click it.
- Native dropdowns (select elements): use select_option with the option's text. Custom dropdowns, radio groups and autocompletes: open or type, analyze, then click the option that matches.
- Fit values to the fields: split a full name across first and last name fields, and put a date in the format or parts the form asks for. If type() reports AUTOCOMPLETE SUGGESTIONS ARE VISIBLE, analyze and click a suggestion instead of pressing Enter.
- Before submitting, analyze and fix any validation message rather than resubmitting blindly.
- Submit only if the task asks you to. After submitting, analyze the page and confirm success from what the site shows: a confirmation message, a reference number, or the next step of the flow.

BLOCKERS
- A CAPTCHA, an MFA prompt, a login you were not given, or a question only the user can answer: call request_human if you have it; otherwise stop and say exactly what blocked you. Never guess credentials or data.

KEYBOARD SAFETY
- press_key only with Enter, Escape, Tab, ArrowDown, ArrowUp, ArrowLeft, ArrowRight, Backspace, Delete, Space, Home, End, PageUp or PageDown. Never F-keys, Meta, Control, Alt, Shift or key combos.

FINISH
- Stop calling tools once the task is done or cannot continue. Reply with a short report whose first line starts with "DONE:" or "FAILED:", followed by what you submitted and any confirmation or reference number the site showed.`;

// The last run per browser as replayable steps, for playbook.js. Element ids die
// with each analysis, so steps keep the analyzer's stable metadata instead.
// ponytail: in memory, oldest evicted past 1000 browsers; a run is lost on restart unless saved as a playbook.
const RECORDED = new Set(['navigate', 'click', 'type', 'select_option', 'upload_file', 'press_key', 'scroll', 'wait']);
const runs = new Map(); // browserId -> { prompt, steps, elements }
const stable = ({ type, tag, text, domId, name, ariaLabel, testId, placeholder, href } = {}) =>
  ({ type, tag, text, domId, name, ariaLabel, testId, placeholder, href });

export function lastRun(browserId) { return runs.get(browserId) || null; }

const PAGE_CHANGING = new Set(['navigate', 'click', 'press_key']);

/**
 * Transforms a placeholder applies to its value, so a run can split or reformat a task
 * value and still replay with other data: {{name|first}}, {{dob|date:MM/DD/YYYY}}.
 * Self-contained arrow functions: renderPlaywright() copies their source into the export.
 */
export const FILTERS = {
  first: (s) => s.trim().split(/\s+/)[0] || '',
  last: (s) => s.trim().split(/\s+/).at(-1) || '',
  part: (s, n) => s.trim().split(/\s+/)[Number(n) - 1] || '',
  upper: (s) => s.toUpperCase(),
  lower: (s) => s.toLowerCase(),
  digits: (s) => s.replace(/\D/g, ''),
  date: (s, format = 'MM/DD/YYYY') => {
    const iso = /^\d{4}-\d{2}-\d{2}$/.test(s.trim());
    const d = new Date(iso ? `${s.trim()}T00:00:00Z` : s);
    if (Number.isNaN(d.getTime())) return s;
    const [y, m, day] = iso ? [d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate()] : [d.getFullYear(), d.getMonth(), d.getDate()];
    const months = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
    const pad = (x) => String(x).padStart(2, '0');
    const parts = { YYYY: String(y), YY: String(y).slice(-2), MMMM: months[m], MMM: months[m].slice(0, 3), MM: pad(m + 1), M: String(m + 1), DD: pad(day), D: String(day) };
    return format.replace(/YYYY|YY|MMMM|MMM|MM|M|DD|D/g, (t) => parts[t]);
  },
};

/** `{{key}}` or `{{key|filter|filter:arg}}`. */
export const PLACEHOLDER = /\{\{(\w+)((?:\|\w+(?::[^|}]*)?)*)\}\}/g;

/** "|first|date:MM/DD" -> [['first'], ['date', 'MM/DD']] */
export const pipesOf = (raw = '') => raw.split('|').slice(1).map((p) => {
  const i = p.indexOf(':');
  return i < 0 ? [p] : [p.slice(0, i), p.slice(i + 1)];
});

/** Placeholders filled from values with their filters applied; unknown keys stay as typed. */
export const fill = (text, data = {}) => (typeof text === 'string'
  ? text.replace(PLACEHOLDER, (m, k, raw) => (data[k] != null
    ? pipesOf(raw).reduce((s, [name, arg]) => (FILTERS[name] ? FILTERS[name](s, arg) : s), String(data[k]))
    : m))
  : text);

/** Data values turned back into their placeholders, so the model never reads them. */
export function redact(text, data = {}) {
  if (typeof text !== 'string') return text;
  // ponytail: values under 3 characters are left alone; redacting them would blank unrelated text.
  const pairs = Object.entries(data).map(([k, v]) => [k, String(v ?? '')]).filter(([, v]) => v.length >= 3)
    .sort((a, b) => b[1].length - a[1].length);
  for (const [k, v] of pairs) text = text.split(v).join(`{{${k}}}`);
  return text;
}

/**
 * Choose a native <select> option by its text. Runs in the page's main world, where the
 * analyzer's element ids do not reach, so the select is found by the stable handles analyze
 * recorded. Every interpolated value is a JSON literal.
 */
const SELECT_OPTION_JS = (el, option) => `(() => {
  const handle = ${JSON.stringify({ domId: el.domId, name: el.name, ariaLabel: el.ariaLabel, text: el.text })};
  const want = ${JSON.stringify(String(option ?? ''))}.trim().toLowerCase();
  const selects = [...document.querySelectorAll('select')];
  const byId = handle.domId && document.getElementById(handle.domId);
  const sel = (byId && byId.tagName === 'SELECT' && byId)
    || (handle.name && selects.find((s) => s.name === handle.name))
    || (handle.ariaLabel && selects.find((s) => s.getAttribute('aria-label') === handle.ariaLabel))
    || (handle.text && selects.find((s) => (s.labels && s.labels[0] ? s.labels[0].textContent.trim() : '') === handle.text));
  if (!sel) return { ok: false, error: 'dropdown not found on the page' };
  const opts = [...sel.options];
  const text = (o) => o.text.trim().toLowerCase();
  const only = (list) => (list.length === 1 ? list[0] : null);
  const pick = opts.find((o) => text(o) === want) || opts.find((o) => o.value.trim().toLowerCase() === want)
    || only(opts.filter((o) => want && text(o).startsWith(want))) || only(opts.filter((o) => want && text(o).includes(want)));
  if (!pick) return { ok: false, error: 'no single option matches', options: opts.map((o) => o.text.trim()).slice(0, 60) };
  sel.value = pick.value;
  sel.dispatchEvent(new Event('input', { bubbles: true }));
  sel.dispatchEvent(new Event('change', { bubbles: true }));
  return { ok: true, chosen: pick.text.trim() };
})()`;

// ponytail: top document only; selects inside iframes or shadow roots are not reached.
export async function selectOptionIn(browserId, el, option) {
  const r = await sendCommand(browserId, 'evaluate_raw', { expression: SELECT_OPTION_JS(el || {}, option) });
  if (!r.ok) return { ok: false, error: r.error || 'select failed' };
  return r.data?.result ?? r.data ?? { ok: false, error: 'select failed' };
}

/**
 * Put a file into a page's file input. Main world, like select_option, because the
 * analyzer's element ids live in an isolated world and its tag attribute is randomised
 * per session — so the element is found by the stable handles analyze recorded.
 *
 * The input itself is usually not what the agent can see: upload widgets hide the real
 * `<input type=file>` behind a button or a drop zone, and a hard-hidden node never gets
 * an element id at all. So the handle names whatever was visible and the input is found
 * from there.
 *
 * ponytail: a 10MB file rides as a ~13.4MB Runtime.evaluate expression; chunk it only if
 * that measurably hurts. Top document only, so inputs inside iframes or shadow roots are
 * out of reach, and a drop zone with no file input behind it cannot be fed this way.
 */
export const UPLOAD_FILE_JS = (el, f) => `(() => {
  const handle = ${JSON.stringify({ domId: el.domId, name: el.name, ariaLabel: el.ariaLabel, text: el.text })};
  const meta = ${JSON.stringify({ file: f.file, type: f.type })};
  const b64 = ${JSON.stringify(String(f.b64 || ''))};
  const inputs = [...document.querySelectorAll('input[type="file"]')];
  if (!inputs.length) return { ok: false, error: 'this page has no file input' };

  const named = handle.domId || handle.name || handle.ariaLabel || handle.text;
  const pool = [...document.querySelectorAll('input,button,label,a,div,span,p,section,form,[role]')];
  const byId = handle.domId && document.getElementById(handle.domId);
  const anchor = byId
    || (handle.name && pool.find((e) => e.getAttribute('name') === handle.name))
    || (handle.ariaLabel && pool.find((e) => e.getAttribute('aria-label') === handle.ariaLabel))
    || (handle.text && pool.find((e) => (e.textContent || '').trim() === handle.text))
    || null;
  if (named && !anchor) return { ok: false, error: 'that element is no longer on the page; analyze again' };

  const isFileInput = (n) => n && n.tagName === 'INPUT' && n.type === 'file';
  const near = (node) => {
    if (!node) return null;
    if (isFileInput(node)) return node;
    if (node.htmlFor) { const t = document.getElementById(node.htmlFor); if (isFileInput(t)) return t; }
    const inside = node.querySelector && node.querySelector('input[type="file"]');
    if (inside) return inside;
    let up = node.parentElement;
    for (let i = 0; i < 4 && up; i++, up = up.parentElement) {
      const found = up.querySelector('input[type="file"]');
      if (found) return found;
    }
    return null;
  };

  const input = near(anchor) || (inputs.length === 1 ? inputs[0] : null);
  if (!input) {
    return { ok: false, error: anchor
      ? 'no file input belongs to that element'
      : 'this page has ' + inputs.length + ' file inputs; pass the element id of the upload button or field you mean' };
  }
  if (input.disabled) return { ok: false, error: 'that file input is disabled' };

  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  const dt = new DataTransfer();
  dt.items.add(new File([bytes], meta.file, { type: meta.type }));
  try { input.files = dt.files; } catch (e) { return { ok: false, error: 'the page would not take the file: ' + e.message }; }
  if (!input.files.length) return { ok: false, error: 'the page cleared the file straight away' };
  input.dispatchEvent(new Event('input', { bubbles: true }));
  input.dispatchEvent(new Event('change', { bubbles: true }));
  return { ok: true, field: input.name || input.id || (input.labels && input.labels[0] && input.labels[0].textContent.trim()) || 'the file input' };
})()`;

/** The upload twin of selectOptionIn. `el` is null when the agent named no element. */
export async function uploadFileIn(browserId, el, f) {
  const r = await sendCommand(browserId, 'evaluate_raw', { expression: UPLOAD_FILE_JS(el || {}, f || {}) }, 120_000);
  if (!r.ok) return { ok: false, error: r.error || 'upload failed' };
  return r.data?.result ?? r.data ?? { ok: false, error: 'upload failed' };
}

/** A file task value, as the SDK's file() builds it. */
export const isFileValue = (v) => !!v && typeof v === 'object' && typeof v.b64 === 'string';

/**
 * upload_file takes the name of a file, not a placeholder — but every other tool is
 * taught to write `{{name}}`, so the model reaches for one here too. Take both.
 */
export const dataKey = (v) => String(v ?? '').replace(/^\{\{\s*|\s*\}\}$/g, '');

const fileSize = (f) => {
  const kb = Math.round((f.b64.length * 3) / 4 / 1024);
  return kb >= 1024 ? `${(kb / 1024).toFixed(1)} MB` : `${kb} KB`;
};

const REQUEST_HUMAN = {
  type: 'function',
  function: {
    name: 'request_human',
    description: 'Ask a person for help when you are stuck: a question only the user can answer, a login you cannot pass, or something the tools cannot do. Waits for their reply.',
    parameters: {
      type: 'object',
      properties: { message: { type: 'string', description: 'What you need and why' } },
      required: ['message'],
      additionalProperties: false,
    },
  },
};

function recordStep(browserId, name, args, values) {
  const run = runs.get(browserId);
  if (!run || !RECORDED.has(name)) return;
  const step = { action: name };
  if (['click', 'type', 'select_option', 'upload_file'].includes(name) && args.element_id != null) {
    const el = stable(run.elements.find((e) => e.id === Number(args.element_id)));
    // Visible data and secrets alike: a playbook stores placeholders, never values.
    for (const k of Object.keys(el)) el[k] = redact(el[k], values);
    step.el = el;
  }
  // The bytes never enter a playbook; the variable name does, so a replay brings its own file.
  if (name === 'upload_file' && args.name) step.file = `{{${dataKey(args.name)}}}`;
  for (const k of ['url', 'text', 'option', 'key', 'direction', 'amount', 'selector', 'timeout']) if (args[k] !== undefined) step[k] = args[k];
  run.steps.push(step);
}

/**
 * Execute a tool by name and return the result as a string for the LLM.
 */
async function executeTool(browserId, name, args, files = {}) {
  try {
    switch (name) {
      case 'analyze_page': {
        const r = await sendCommand(browserId, 'analyze');
        if (!r.ok) return `Error: ${r.error}`;
        const { markdown, elements, scroll, viewport, truncated } = r.data;
        const run = runs.get(browserId);
        if (run) run.elements = elements;
        const visible = elements.filter((e) => e.visible);
        const offscreen = elements.filter((e) => !e.visible);
        let index = `\n\n## Element Index (${elements.length} total, ${visible.length} visible)\n\n`;
        if (visible.length) {
          index += '### Visible\n' + visible.map((e) => {
            let line = `  [#${e.id}] ${e.type}`;
            if (e.text) line += `: ${e.text}`;
            if (e.href) line += ` → ${e.href}`;
            if (e.value) line += ` value="${e.value}"`;
            if (e.checked) line += ' ✓';
            if (e.disabled) line += ' (disabled)';
            return line;
          }).join('\n') + '\n';
        }
        if (offscreen.length) {
          index += '\n### Off-screen\n' + offscreen.slice(0, 30).map((e) => `  [#${e.id}] ${e.type}: ${e.text || ''}`).join('\n') + '\n';
          if (offscreen.length > 30) index += `  ... and ${offscreen.length - 30} more off-screen elements\n`;
        }
        if (truncated) index += '\n⚠ Page content was truncated.\n';
        // Cap total output to avoid blowing context window
        const result = markdown + index;
        if (result.length > 30000) return result.slice(0, 30000) + '\n\n⚠ Output truncated to fit context window.';
        return result;
      }
      case 'navigate': {
        const r = await sendCommand(browserId, 'navigate', { url: args.url }, 90000);
        return r.ok ? `Navigated to ${args.url}` : `Error: ${r.error}`;
      }
      case 'click': {
        const r = await sendCommand(browserId, 'click', { selector: `[data-ac-id="${args.element_id}"]` });
        return r.ok ? `Clicked element ${args.element_id}` : `Error: ${r.error}`;
      }
      case 'press_key': {
        const r = await sendCommand(browserId, 'press_key', { key: args.key });
        return r.ok ? `Pressed ${args.key}` : `Error: ${r.error}`;
      }
      case 'type': {
        const r = await sendCommand(browserId, 'type', { selector: `[data-ac-id="${args.element_id}"]`, text: args.text });
        if (!r.ok) return `Error: ${r.error}`;
        if (r.data?.suggestions_visible) {
          return `Typed "${args.text}" into element ${args.element_id}. AUTOCOMPLETE SUGGESTIONS ARE VISIBLE — call analyze_page now to see and click a suggestion, or press Enter to submit as-is.`;
        }
        return `Typed "${args.text}" into element ${args.element_id}`;
      }
      case 'select_option': {
        const el = runs.get(browserId)?.elements.find((e) => e.id === Number(args.element_id));
        if (!el) return 'Error: Element not found. Call analyze_page and use a current id.';
        const r = await selectOptionIn(browserId, el, args.option);
        return r.ok
          ? `Selected "${r.chosen}" in element ${args.element_id}`
          : `Error: ${r.error}${r.options ? `. Options: ${r.options.join(' | ')}` : ''}`;
      }
      case 'upload_file': {
        const key = dataKey(args.name);
        const f = files[key];
        if (!f) {
          const have = Object.keys(files);
          return `Error: no file named "${key}" in the task data.${have.length ? ` Available: ${have.join(', ')}.` : ' This task was given no files.'}`;
        }
        const el = args.element_id != null ? runs.get(browserId)?.elements.find((e) => e.id === Number(args.element_id)) : null;
        if (args.element_id != null && !el) return 'Error: Element not found. Call analyze_page and use a current id.';
        const r = await uploadFileIn(browserId, el, f);
        return r.ok ? `Attached ${f.file} to ${r.field}` : `Error: ${r.error}`;
      }
      case 'screenshot': {
        const r = await sendCommand(browserId, 'screenshot');
        if (!r.ok) return `Error: ${r.error}`;
        if (r.data?.screenshot) return `Screenshot captured (base64 image data available)`;
        return 'Screenshot captured';
      }
      case 'scroll': {
        const r = await sendCommand(browserId, 'scroll', { direction: args.direction, amount: args.amount });
        return r.ok ? `Scrolled ${args.direction}` : `Error: ${r.error}`;
      }
      case 'wait': {
        const r = await sendCommand(browserId, 'wait', { selector: args.selector, timeout: args.timeout });
        return r.ok ? `Element found: ${args.selector}` : `Error: ${r.error}`;
      }
      case 'read_elements': {
        const r = await sendCommand(browserId, 'read_page', { selector: args.selector, limit: args.limit });
        if (!r.ok) return `Error: ${r.error}`;
        const { url, title, elements } = r.data;
        const summary = elements.map((e) => `${e.tag}#${e.id || '?'} — ${e.text || e.aria_label || '(no text)'}`).join('\n');
        return `Page: ${title} (${url})\n\nElements (${elements.length}):\n${summary}`;
      }
      case 'list_tabs': {
        const r = await sendCommand(browserId, 'list_tabs');
        if (!r.ok) return `Error: ${r.error}`;
        const list = (r.data.tabs || []).map((t) => `${t.active ? '→ ' : '  '}[tab ${t.id}] ${t.title} — ${t.url}`).join('\n');
        return `Tabs:\n${list}`;
      }
      case 'open_tab': {
        const r = await sendCommand(browserId, 'open_tab', { url: args.url }, 90000);
        return r.ok ? `Opened tab ${r.data?.tab_id || ''}${args.url ? ' at ' + args.url : ''}` : `Error: ${r.error}`;
      }
      case 'switch_tab': {
        const r = await sendCommand(browserId, 'switch_tab', { tab_id: args.tab_id });
        return r.ok ? `Switched to tab ${args.tab_id}` : `Error: ${r.error}`;
      }
      // Offered in BROWSER_TOOLS for pages element ids cannot reach; not recorded, since replays cannot aim them.
      case 'click_coordinates': {
        const r = await sendCommand(browserId, 'click_coordinates', { x: args.x, y: args.y });
        return r.ok ? `Clicked at ${args.x},${args.y}` : `Error: ${r.error}`;
      }
      case 'mouse_move': {
        const r = await sendCommand(browserId, 'mouse_move', { x: args.x, y: args.y });
        return r.ok ? `Moved the mouse to ${args.x},${args.y}` : `Error: ${r.error}`;
      }
      case 'double_click': {
        const target = args.element_id != null ? { element_id: args.element_id, selector: `[data-ac-id="${args.element_id}"]` } : { x: args.x, y: args.y };
        const r = await sendCommand(browserId, 'double_click', target);
        return r.ok ? `Double-clicked ${args.element_id != null ? `element ${args.element_id}` : `at ${args.x},${args.y}`}` : `Error: ${r.error}`;
      }
      case 'keyboard_type': {
        const r = await sendCommand(browserId, 'keyboard_type', { text: args.text });
        return r.ok ? `Typed "${args.text}" into the focused element` : `Error: ${r.error}`;
      }
      case 'drag': {
        const r = await sendCommand(browserId, 'drag', { from_x: args.from_x, from_y: args.from_y, to_x: args.to_x, to_y: args.to_y });
        return r.ok ? `Dragged from ${args.from_x},${args.from_y} to ${args.to_x},${args.to_y}` : `Error: ${r.error}`;
      }
      case 'close_tab': {
        const r = await sendCommand(browserId, 'close_tab', { tab_id: args.tab_id });
        return r.ok ? `Closed tab` : `Error: ${r.error}`;
      }
      default:
        return `Unknown tool: ${name}`;
    }
  } catch (err) {
    return `Error: ${err.message}`;
  }
}

/**
 * Run the agentic loop: LLM → tool calls → execute → feed back → repeat until done.
 * Streams the final text response.
 */
/**
 * `data` values are typed through `{{key}}` placeholders and redacted from everything
 * the model reads. `checkpoint` runs after page-changing tools; `requestHuman`, when
 * given, lets the agent ask a person and wait.
 */
export async function runChat(browserId, messages, { apiKey, onToolCall, onText, data = {}, secrets = {}, checkpoint, requestHuman } = {}) {
  // Settings belong to the calling API key; a key that has set none falls
  // back to the deployment-wide values.
  const { openaiKey, baseUrl, model, own } = keyConfig.resolve(apiKey);

  // A runaway agent loop is the most expensive thing this control plane can do
  // on someone else's behalf, so the ceiling is checked before the first call.
  // A key with its own LLM credential pays for its own tokens and has no ceiling.
  const budget = own ? { allowed: true } : checkHourly('chatTokensPerHour', apiKey);
  if (!budget.allowed) {
    metrics.chatRequests.inc({ outcome: 'quota' });
    throw Object.assign(
      new Error(`Chat token quota reached for this hour (${budget.current}/${budget.quota})`),
      { status: 429 },
    );
  }
  if (!openaiKey) {
    throw new Error('No LLM key configured for this API key. Add one in Settings, run `oya init`, or POST /api/config.');
  }
  const OPENAI_BASE = baseUrl;
  const MODEL = model;

  const prompt = messages.filter((m) => m.role === 'user').at(-1)?.content;
  // A file is attached, never typed, so it is split out before anything that fills or
  // redacts a placeholder sees it — String(a file) is "[object Object]".
  const files = Object.fromEntries(Object.entries(data).filter(([, v]) => isFileValue(v)));
  const scalars = Object.fromEntries(Object.entries(data).filter(([, v]) => !isFileValue(v)));
  // `data` the model reads; `secrets` it never does. Both are typed through placeholders.
  const values = { ...scalars, ...secrets };
  const run = { prompt: typeof prompt === 'string' ? redact(prompt, values) : '', steps: [], elements: [], secrets: Object.keys(secrets) };
  // The page the run starts on, so a playbook replays from the same place.
  const tabs = await sendCommand(browserId, 'list_tabs').catch(() => null);
  const startUrl = tabs?.data?.tabs?.find((t) => t.active)?.url;
  if (/^https?:/.test(startUrl || '')) run.steps.push({ action: 'navigate', url: startUrl, start: true });
  runs.delete(browserId);
  runs.set(browserId, run);
  if (runs.size > 1000) runs.delete(runs.keys().next().value);

  const system = [SYSTEM_PROMPT];
  if (Object.keys(values).length) {
    system.push('TASK VALUES: type every task value as its {{placeholder}}, never as literal text, so the recorded playbook replays with other data. Transform a value with filters instead of retyping part of it: {{name|first}}, {{name|last}}, {{name|part:2}} (Nth word), {{x|upper}}, {{x|lower}}, {{x|digits}}, {{dob|date:MM/DD/YYYY}} (tokens YYYY YY MMMM MMM MM M DD D; separate month, day and year fields take {{dob|date:MM}}, {{dob|date:DD}}, {{dob|date:YYYY}}). select_option takes placeholders too.');
  }
  if (Object.keys(scalars).length) {
    system.push(`DATA (you can read these to decide what to do):\n${Object.entries(scalars).map(([k, v]) => `{{${k}}} = ${JSON.stringify(String(v))}`).join('\n')}`);
  }
  if (Object.keys(files).length) {
    system.push(`FILES you can attach:\n${Object.entries(files).map(([k, f]) => `  ${k} — "${f.file}" (${f.type}, ${fileSize(f)})`).join('\n')}\nUse upload_file with the name on the left. The real file input is usually hidden behind a "Choose file" or "Upload" button or a drop zone, so pass the element id of whatever you can see there and it will be found; leave element_id out only when the page has a single upload field.`);
  }
  if (Object.keys(secrets).length) {
    system.push(`SECRETS (hidden from you): ${Object.keys(secrets).map((k) => `{{${k}}}`).join(', ')}. Type them as placeholders; the real value is filled in and reads back as the placeholder, so a field showing one is filled correctly. Filters work on them too.`);
  }
  const allMessages = [
    { role: 'system', content: system.join('\n\n') },
    ...messages.map((m) => ({ ...m, content: redact(m.content, secrets) })),
  ];

  let iterations = 0;
  const maxIterations = parseInt(process.env.CHAT_MAX_ITERATIONS || '200', 10);

  while (iterations < maxIterations) {
    iterations++;

    // Trim old tool results if context is getting too large (~4 chars per token)
    let totalChars = allMessages.reduce((sum, m) => sum + (m.content?.length || 0) + JSON.stringify(m.tool_calls || '').length, 0);
    while (totalChars > 400000 && allMessages.length > 3) {
      // Find the oldest tool result and truncate it
      const toolIdx = allMessages.findIndex((m, i) => i > 0 && m.role === 'tool');
      if (toolIdx === -1) break;
      // Also remove the assistant message with tool_calls right before it
      const prevIdx = toolIdx - 1;
      if (prevIdx > 0 && allMessages[prevIdx].role === 'assistant' && allMessages[prevIdx].tool_calls) {
        // Count how many tool results follow this assistant message
        let endIdx = toolIdx;
        while (endIdx < allMessages.length && allMessages[endIdx].role === 'tool') endIdx++;
        allMessages.splice(prevIdx, endIdx - prevIdx);
      } else {
        allMessages.splice(toolIdx, 1);
      }
      totalChars = allMessages.reduce((sum, m) => sum + (m.content?.length || 0) + JSON.stringify(m.tool_calls || '').length, 0);
    }

    // Not `data`: that name is the caller's hidden values, which fill() and redact() read below.
    const completion = await chatCompletion({
      baseUrl: OPENAI_BASE,
      apiKey: openaiKey,
      model: MODEL,
      messages: allMessages,
      tools: requestHuman ? [...BROWSER_TOOLS, REQUEST_HUMAN] : BROWSER_TOOLS,
    });

    // Every iteration of the agentic loop bills, so account per iteration
    // rather than once per request.
    const input = completion.usage?.prompt_tokens || 0;
    const output = completion.usage?.completion_tokens || 0;
    if (input) { metrics.chatTokens.inc({ direction: 'input' }, input); usage.record(apiKey, 'chat_input_tokens', input); }
    if (output) { metrics.chatTokens.inc({ direction: 'output' }, output); usage.record(apiKey, 'chat_output_tokens', output); }

    const choice = completion.choices?.[0];
    if (!choice) throw new Error('No completion in response');

    const msg = choice.message;

    if (msg.tool_calls?.length) {
      const toolCalls = msg.tool_calls.filter((tc) => tc && tc.id);
      if (toolCalls.length !== msg.tool_calls.length) {
        console.warn('[chat] Skipped tool calls without id');
      }
      const toolResults = [];
      for (const tc of toolCalls) {
        const name = tc.function?.name;
        let args = {};
        try {
          if (tc.function?.arguments) args = JSON.parse(tc.function.arguments);
        } catch {}
        onToolCall?.({ name, args });
        let result;
        try {
          result = name === 'request_human' && requestHuman
            ? `The person replied: ${await requestHuman({ reason: 'agent', message: String(args.message || '') })}`
            : await executeTool(browserId, name, Object.fromEntries(Object.entries(args).map(([k, v]) => [k, ['text', 'option', 'url'].includes(k) ? fill(v, values) : v])), files);
        } catch (err) {
          result = `Error: ${err.message}`;
        }
        if (!String(result).startsWith('Error')) {
          recordStep(browserId, name, args, values);
          if (PAGE_CHANGING.has(name)) await checkpoint?.();
        }
        // ponytail: whole secret values are redacted; a filtered piece of one (its digits, a first name) read back from the page is not.
        result = redact(result, secrets);
        toolResults.push({ tool_call_id: tc.id, content: result });
      }
      allMessages.push({
        role: 'assistant',
        content: msg.content ?? null,
        // Spread, not rebuilt: Gemini 3 rejects the next turn unless each call's
        // extra_content (its thought signature) comes back unchanged.
        tool_calls: toolCalls.map((tc) => ({
          ...tc,
          type: 'function',
          function: { name: tc.function?.name, arguments: tc.function?.arguments ?? '{}' },
        })),
      });
      for (const tr of toolResults) {
        allMessages.push({ role: 'tool', tool_call_id: tr.tool_call_id, content: tr.content });
      }
      continue;
    }

    const text = msg.content?.trim();
    if (text) {
      onText?.(text);
      return { text, toolCalls: [] };
    }
  }

  return { text: 'Reached iteration limit.', toolCalls: [], limited: true };
}
