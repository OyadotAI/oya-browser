/**
 * Tool name → handler: what each browser tool the model calls does, answered as
 * text for the model. Errors a browser reports come back as `Error: …` text.
 */
import { Status } from '../../platform/http-status.ts';
import { sendCommand } from '../browsers/socket.ts';
import { selectOptionIn, uploadFileIn } from './page-scripts.ts';
import { dataKey } from './placeholders.ts';
import { elementOf, rememberHandle, setElements } from './recorder.ts';
import { analysisText, elementList } from './element-index.ts';
import { NAVIGATE_TIMEOUT_MS, PAGE_FORMAT } from './constants.ts';

/** Runs one tool call on a browser; `files` are the task's attachable files by name. */
export type ToolHandler = (browserId: string, args: Record<string, any>, files: Record<string, any>) => Promise<string>;

/** What the model is told when an element id is not in the latest analysis. */
const ELEMENT_GONE = 'Error: Element not found. Call analyze_page and use a current id.';

/** The CSS selector for an element id from the latest analysis. */
const byId = (elementId) => `[data-ac-id="${elementId}"]`;

/** The page and its elements, capped to fit the context window; the elements alone when no content is asked for. */
async function analyzePage(browserId, args: Record<string, any> = {}) {
  const r = await sendCommand(browserId, 'analyze', { format: PAGE_FORMAT });
  if (!r.ok) return `Error: ${r.error}`;
  setElements(browserId, r.data.elements);
  return analysisText(r.data, { content: args.content !== false });
}

/** Goes to a URL, waiting as long as a slow site needs. */
async function navigate(browserId, args) {
  const r = await sendCommand(browserId, 'navigate', { url: args.url }, NAVIGATE_TIMEOUT_MS);
  return r.ok ? `Navigated to ${args.url}` : `Error: ${r.error}`;
}

/**
 * The elements of the page an action left behind, added to what the action
 * says. Without it the model must call analyze_page after every click merely to
 * learn the new ids, which costs a round trip and a whole page; the elements
 * alone are short and are what it needs to act again.
 */
async function withControls(browserId, said) {
  const r = await sendCommand(browserId, 'analyze', { format: PAGE_FORMAT });
  if (!r.ok || !r.data?.elements?.length) return said;
  setElements(browserId, r.data.elements);
  return `${said}\n\n${analysisText(r.data, { content: false })}`;
}

/** Clicks an element by id. */
async function click(browserId, args) {
  const r = await sendCommand(browserId, 'click', { selector: byId(args.element_id) });
  if (!r.ok) return `Error: ${r.error}`;
  rememberHandle(browserId, args.element_id, r.data?.handle);
  return withControls(browserId, `Clicked element ${args.element_id}`);
}

/** Presses one safe key. */
async function pressKey(browserId, args) {
  const r = await sendCommand(browserId, 'press_key', { key: args.key });
  return r.ok ? `Pressed ${args.key}` : `Error: ${r.error}`;
}

/** Types into an element, pointing the model at autocomplete suggestions when they appear. */
async function type(browserId, args) {
  const r = await sendCommand(browserId, 'type', { selector: byId(args.element_id), text: args.text });
  if (!r.ok) return `Error: ${r.error}`;
  rememberHandle(browserId, args.element_id, r.data?.handle);
  return withControls(browserId, typedReport(args, r.data || {}));
}

/**
 * What typing did, in the words the model acts on: the value a date input now
 * holds, visible suggestions to pick from, or a field (a mask, say) that shows
 * something other than what was typed.
 */
function typedReport(args, data) {
  const typed = `Typed "${args.text}" into element ${args.element_id}`;
  if (data.value !== undefined) return `Set element ${args.element_id} to ${data.value}`;
  if (data.suggestions_visible)
    return `${typed}. AUTOCOMPLETE SUGGESTIONS ARE VISIBLE, call analyze_page now to see and click a suggestion, or press Enter to submit as-is.`;
  if (typeof data.shown === 'string')
    return `${typed}, but the field now shows "${data.shown}". If that is wrong, type the whole value again.`;
  return typed;
}

/** Chooses a native <select> option by its text, listing the options when none matches. */
async function selectOption(browserId, args) {
  const el = elementOf(browserId, args.element_id);
  if (!el) return ELEMENT_GONE;
  const r = await selectOptionIn(browserId, el, args.option);
  return r.ok
    ? withControls(browserId, `Selected "${r.chosen}" in element ${args.element_id}`)
    : `Error: ${r.error}${r.options ? `. Options: ${r.options.join(' | ')}` : ''}`;
}

/** The answer when the model names a file the task does not have. */
function missingFile(key, files) {
  const have = Object.keys(files);
  return `Error: no file named "${key}" in the task data.${have.length ? ` Available: ${have.join(', ')}.` : ' This task was given no files.'}`;
}

/** Attaches a task file to the file input behind the named element, or the page's only one. */
async function uploadFile(browserId, args, files) {
  const key = dataKey(args.name);
  const f = files[key];
  if (!f) return missingFile(key, files);
  const el = args.element_id != null ? elementOf(browserId, args.element_id) : null;
  if (args.element_id != null && !el) return ELEMENT_GONE;
  const r = await uploadFileIn(browserId, el, f);
  return r.ok ? `Attached ${f.file} to ${r.field}` : `Error: ${r.error}`;
}

/** Scrolls the page, and shows the page it landed on when the browser analysed it (the Oya client does). */
async function scroll(browserId, args) {
  const { direction, amount } = args;
  const r = await sendCommand(browserId, 'scroll', { direction, amount, format: PAGE_FORMAT });
  if (!r.ok) return `Error: ${r.error}`;
  if (!r.data?.elements) return `Scrolled ${args.direction}`;
  setElements(browserId, r.data.elements);
  return `Scrolled ${args.direction}.\n\n` + analysisText(r.data);
}

/** Waits for a selector to appear. */
async function wait(browserId, args) {
  const r = await sendCommand(browserId, 'wait', { selector: args.selector, timeout: args.timeout });
  return r.ok ? `Element found: ${args.selector}` : `Error: ${r.error}`;
}

/** Lists the elements matching a selector. */
async function readElements(browserId, args) {
  const r = await sendCommand(browserId, 'analyze', args.selector ? { selector: args.selector } : {});
  if (!r.ok) return `Error: ${r.error}`;
  setElements(browserId, r.data.elements);
  return elementList(r.data, args.limit);
}

/** One console entry as the model reads it. */
const consoleLine = (e) => `[${e.level}] ${e.message}${e.source ? ` (${e.source}:${e.line})` : ''}`;

/** One request as the model reads it: how it ended, then what it was. */
const requestLine = (q) => `${q.status ?? q.error} ${q.method} ${q.url}${q.type ? ` (${q.type})` : ''}`;

/** What the page logged, newest first, so a failure can be read rather than guessed at. */
async function readConsole(browserId, args) {
  const params = { level: args.level, pattern: args.pattern, limit: args.limit };
  const r = await sendCommand(browserId, 'read_console', params);
  if (!r.ok) return `Error: ${r.error}`;
  const entries = r.data?.entries || [];
  return entries.length ? entries.map(consoleLine).join('\n') : 'The page has logged nothing matching that.';
}

/**
 * Whether the browser really refused this request. Checked here as well as in
 * the browser because a fleet runs mixed versions, and an older one calls a
 * successful request's error `net::OK`, which would fill an agent's answer with
 * failures that never happened.
 */
const trulyFailed = (q) => (q.error && q.error !== 'net::OK') || (q.status !== null && q.status >= Status.BAD_REQUEST);

/** What the page requested and how the server answered. */
async function readNetwork(browserId, args) {
  const params = { failedOnly: args.failed_only === true, pattern: args.pattern, limit: args.limit };
  const r = await sendCommand(browserId, 'read_network', params);
  if (!r.ok) return `Error: ${r.error}`;
  const all = r.data?.requests || [];
  const requests = args.failed_only === true ? all.filter(trulyFailed) : all;
  return requests.length ? requests.map(requestLine).join('\n') : 'No requests matching that.';
}

/** Lists the open tabs, marking the active one. */
async function listTabs(browserId) {
  const r = await sendCommand(browserId, 'list_tabs');
  if (!r.ok) return `Error: ${r.error}`;
  const list = (r.data.tabs || []).map((t) => `${t.active ? '→ ' : '  '}[tab ${t.id}] ${t.title}, ${t.url}`).join('\n');
  return `Tabs:\n${list}`;
}

/** Opens a tab, at a URL when one is given. */
async function openTab(browserId, args) {
  const r = await sendCommand(browserId, 'open_tab', { url: args.url }, NAVIGATE_TIMEOUT_MS);
  return r.ok ? `Opened tab ${r.data?.tab_id || ''}${args.url ? ' at ' + args.url : ''}` : `Error: ${r.error}`;
}

/** Makes another tab the active one. */
async function switchTab(browserId, args) {
  const r = await sendCommand(browserId, 'switch_tab', { tab_id: args.tab_id });
  return r.ok ? `Switched to tab ${args.tab_id}` : `Error: ${r.error}`;
}

/** Clicks at page coordinates. */
async function clickCoordinates(browserId, args) {
  const r = await sendCommand(browserId, 'click_coordinates', { x: args.x, y: args.y });
  return r.ok ? `Clicked at ${args.x},${args.y}` : `Error: ${r.error}`;
}

/** Moves the mouse to page coordinates. */
async function mouseMove(browserId, args) {
  const r = await sendCommand(browserId, 'mouse_move', { x: args.x, y: args.y });
  return r.ok ? `Moved the mouse to ${args.x},${args.y}` : `Error: ${r.error}`;
}

/** Double-clicks an element by id, or at coordinates when no id is given. */
async function doubleClick(browserId, args) {
  const target =
    args.element_id != null
      ? { element_id: args.element_id, selector: byId(args.element_id) }
      : { x: args.x, y: args.y };
  const r = await sendCommand(browserId, 'double_click', target);
  return r.ok
    ? `Double-clicked ${args.element_id != null ? `element ${args.element_id}` : `at ${args.x},${args.y}`}`
    : `Error: ${r.error}`;
}

/** Types into whatever has focus. */
async function keyboardType(browserId, args) {
  const r = await sendCommand(browserId, 'keyboard_type', { text: args.text });
  return r.ok ? `Typed "${args.text}" into the focused element` : `Error: ${r.error}`;
}

/** Drags between two points. */
async function drag(browserId, args) {
  const { from_x, from_y, to_x, to_y } = args;
  const r = await sendCommand(browserId, 'drag', { from_x, from_y, to_x, to_y });
  return r.ok ? `Dragged from ${args.from_x},${args.from_y} to ${args.to_x},${args.to_y}` : `Error: ${r.error}`;
}

/** Accepts or dismisses the open confirm or prompt. */
async function handleDialog(browserId, args) {
  const r = await sendCommand(browserId, 'handle_dialog', { accept: args.accept, prompt_text: args.prompt_text });
  if (!r.ok) return `Error: ${r.error}`;
  return `${r.data?.accepted === false ? 'Dismissed' : 'Accepted'} the ${r.data?.type || ''} dialog. Call analyze_page to see the page now.`;
}

/** Closes a tab. */
async function closeTab(browserId, args) {
  const r = await sendCommand(browserId, 'close_tab', { tab_id: args.tab_id });
  return r.ok ? `Closed tab` : `Error: ${r.error}`;
}

/** The handler for each tool in BROWSER_TOOLS. */
export const TOOL_HANDLERS: Record<string, ToolHandler> = {
  analyze_page: analyzePage,
  navigate,
  click,
  press_key: pressKey,
  type,
  select_option: selectOption,
  upload_file: uploadFile,
  scroll,
  wait,
  read_elements: readElements,
  list_tabs: listTabs,
  open_tab: openTab,
  read_console: readConsole,
  read_network: readNetwork,
  switch_tab: switchTab,
  // Offered in BROWSER_TOOLS for pages element ids cannot reach; not recorded, since replays cannot aim them.
  click_coordinates: clickCoordinates,
  mouse_move: mouseMove,
  double_click: doubleClick,
  keyboard_type: keyboardType,
  drag,
  handle_dialog: handleDialog,
  close_tab: closeTab,
};
