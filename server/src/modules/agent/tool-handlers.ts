/**
 * Tool name → handler: what each browser tool the model calls does, answered as
 * text for the model. Errors a browser reports come back as `Error: …` text.
 */
import { sendCommand } from '../browsers/socket.ts';
import { selectOptionIn, uploadFileIn } from './page-scripts.ts';
import { dataKey } from './placeholders.ts';
import { elementOf, setElements } from './recorder.ts';
import { elementIndex } from './element-index.ts';
import { MAX_ANALYSIS_CHARS, NAVIGATE_TIMEOUT_MS } from './constants.ts';

/** Runs one tool call on a browser; `files` are the task's attachable files by name. */
export type ToolHandler = (browserId: string, args: Record<string, any>, files: Record<string, any>) => Promise<string>;

/** What the model is told when an element id is not in the latest analysis. */
const ELEMENT_GONE = 'Error: Element not found. Call analyze_page and use a current id.';

/** The CSS selector for an element id from the latest analysis. */
const byId = (elementId) => `[data-ac-id="${elementId}"]`;

/** The page as markdown plus the element index, capped to fit the context window. */
async function analyzePage(browserId) {
  const r = await sendCommand(browserId, 'analyze');
  if (!r.ok) return `Error: ${r.error}`;
  const { markdown, elements, truncated } = r.data;
  setElements(browserId, elements);
  // Cap total output to avoid blowing context window
  const result = markdown + elementIndex(elements, truncated);
  if (result.length > MAX_ANALYSIS_CHARS)
    return result.slice(0, MAX_ANALYSIS_CHARS) + '\n\n⚠ Output truncated to fit context window.';
  return result;
}

/** Goes to a URL, waiting as long as a slow site needs. */
async function navigate(browserId, args) {
  const r = await sendCommand(browserId, 'navigate', { url: args.url }, NAVIGATE_TIMEOUT_MS);
  return r.ok ? `Navigated to ${args.url}` : `Error: ${r.error}`;
}

/** Clicks an element by id. */
async function click(browserId, args) {
  const r = await sendCommand(browserId, 'click', { selector: byId(args.element_id) });
  return r.ok ? `Clicked element ${args.element_id}` : `Error: ${r.error}`;
}

/** Presses one safe key. */
async function pressKey(browserId, args) {
  const r = await sendCommand(browserId, 'press_key', { key: args.key });
  return r.ok ? `Pressed ${args.key}` : `Error: ${r.error}`;
}

/** Types into an element, pointing the model at autocomplete suggestions when they appear. */
async function type(browserId, args) {
  const r = await sendCommand(browserId, 'type', { selector: byId(args.element_id), text: args.text });
  return r.ok ? typedReport(args, r.data || {}) : `Error: ${r.error}`;
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
    return `${typed}. AUTOCOMPLETE SUGGESTIONS ARE VISIBLE — call analyze_page now to see and click a suggestion, or press Enter to submit as-is.`;
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
    ? `Selected "${r.chosen}" in element ${args.element_id}`
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

/** Captures the screen; the model only learns that it worked. */
async function screenshot(browserId) {
  const r = await sendCommand(browserId, 'screenshot');
  if (!r.ok) return `Error: ${r.error}`;
  if (r.data?.screenshot) return `Screenshot captured (base64 image data available)`;
  return 'Screenshot captured';
}

/** Scrolls the page. */
async function scroll(browserId, args) {
  const r = await sendCommand(browserId, 'scroll', { direction: args.direction, amount: args.amount });
  return r.ok ? `Scrolled ${args.direction}` : `Error: ${r.error}`;
}

/** Waits for a selector to appear. */
async function wait(browserId, args) {
  const r = await sendCommand(browserId, 'wait', { selector: args.selector, timeout: args.timeout });
  return r.ok ? `Element found: ${args.selector}` : `Error: ${r.error}`;
}

/** Lists the elements matching a selector. */
async function readElements(browserId, args) {
  const r = await sendCommand(browserId, 'read_page', { selector: args.selector, limit: args.limit });
  if (!r.ok) return `Error: ${r.error}`;
  const { url, title, elements } = r.data;
  const summary = elements.map((e) => `${e.tag}#${e.id || '?'} — ${e.text || e.aria_label || '(no text)'}`).join('\n');
  return `Page: ${title} (${url})\n\nElements (${elements.length}):\n${summary}`;
}

/** Lists the open tabs, marking the active one. */
async function listTabs(browserId) {
  const r = await sendCommand(browserId, 'list_tabs');
  if (!r.ok) return `Error: ${r.error}`;
  const list = (r.data.tabs || [])
    .map((t) => `${t.active ? '→ ' : '  '}[tab ${t.id}] ${t.title} — ${t.url}`)
    .join('\n');
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
  screenshot,
  scroll,
  wait,
  read_elements: readElements,
  list_tabs: listTabs,
  open_tab: openTab,
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
