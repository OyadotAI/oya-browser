/**
 * Keeps a long run's conversation inside the model's context window by dropping
 * the oldest tool results first.
 */
import {
  DROPPED_STEP_CHARS,
  IMAGE_CONTEXT_CHARS,
  MAX_CONTEXT_CHARS,
  MIN_MESSAGES_KEPT,
  PAGES_KEPT_IN_FULL,
  PAGE_RESULT_CHARS,
} from './constants.ts';

/** Tools whose long results are logs and lists, not pages: they are never condensed as a page read. */
const NOT_PAGES = new Set(['read_network', 'read_console', 'list_tabs']);

/** One message's content size: text as its length, an image as IMAGE_CONTEXT_CHARS. */
const contentSize = (content) =>
  Array.isArray(content)
    ? content.reduce((sum, part) => sum + (part.type === 'image_url' ? IMAGE_CONTEXT_CHARS : part.text?.length || 0), 0)
    : content?.length || 0;

/** The conversation's size in characters (~4 per token), tool calls and images included. */
const contextSize = (messages) =>
  messages.reduce((sum, m) => sum + contentSize(m.content) + JSON.stringify(m.tool_calls || '').length, 0);

/** The tool each call id was made for, from the assistant turns. */
const toolNames = (messages) =>
  new Map(messages.flatMap((m) => (m.tool_calls || []).map((tc) => [tc.id, tc.function?.name])));

/** A one-line record of an assistant turn's calls and what the first result said. */
function stepRecord(turn, results) {
  const calls = (turn.tool_calls || []).map((tc) => `${tc.function?.name || 'tool'} ${tc.function?.arguments || ''}`);
  const said = String(results[0]?.content || '').split('\n')[0];
  return `- ${calls.join('; ')} → ${said}`.slice(0, DROPPED_STEP_CHARS);
}

/** Adds a dropped turn's record to the task message, so the model still knows what it already did. */
function noteDropped(messages, record) {
  const at = messages.findIndex((m) => m.role === 'user' && typeof m.content === 'string');
  if (at === -1) return;
  const task = messages[at].content;
  // Replaced, not edited in place: the caller's own message objects stay as they were.
  messages[at] = { ...messages[at], content: `${task}${task.includes(EARLIER) ? '' : `\n\n${EARLIER}`}\n${record}` };
}

/** The heading of the record of dropped steps in the task message. */
const EARLIER = '[Earlier steps of this run, dropped to save context:]';

/** The index just past the run of tool results starting at `toolIdx`. */
function toolResultsEnd(messages, toolIdx) {
  let endIdx = toolIdx;
  while (endIdx < messages.length && messages[endIdx].role === 'tool') endIdx++;
  return endIdx;
}

/** Drops the oldest tool result, with the assistant turn that asked for it, leaving a record of it; false when there is none. */
function dropOldestToolTurn(messages) {
  const toolIdx = messages.findIndex((m, i) => i > 0 && m.role === 'tool');
  if (toolIdx === -1) return false;
  // Also remove the assistant message with tool_calls right before it, and every result it got
  const prevIdx = toolIdx - 1;
  const asked = prevIdx > 0 && messages[prevIdx].role === 'assistant' && messages[prevIdx].tool_calls;
  const removed = asked ? messages.splice(prevIdx, toolResultsEnd(messages, toolIdx) - prevIdx) : [];
  if (asked) noteDropped(messages, stepRecord(removed[0], removed.slice(1)));
  else messages.splice(toolIdx, 1);
  return true;
}

/** Whether a tool result is a page read rather than a short answer or a log. */
const isPageRead = (m, names) =>
  m.role === 'tool' && !NOT_PAGES.has(names.get(m.tool_call_id)) && (m.content?.length || 0) >= PAGE_RESULT_CHARS;

/** What an old page leaves behind: enough to remember it was read, nothing to act on. */
function pageNote(content) {
  const url = /^url:\s*(\S+)/m.exec(content)?.[1] || /"url":\s*"([^"]+)"/.exec(content)?.[1] || '';
  const elements = /elements:\s*(\d+)/.exec(content)?.[1] || '';
  const said = [url && `of ${url}`, elements && `${elements} elements`].filter(Boolean).join(', ');
  return `[earlier page read${said ? ` ${said}` : ''} dropped to save context. Its element ids are long gone; analyze_page again to act on that page.]`;
}

/**
 * Replaces every page read but the newest with a note. The ids in an older read
 * stopped working the moment the next analyze_page ran, so sending it again buys
 * the model nothing and costs it the whole page on every later turn.
 */
export function condenseOldPages(messages, keep = PAGES_KEPT_IN_FULL) {
  const names = toolNames(messages);
  const reads = messages.filter((m) => isPageRead(m, names));
  for (const message of reads.slice(0, Math.max(reads.length - keep, 0))) {
    message.content = pageNote(message.content);
  }
}

/** Trim old tool results if context is getting too large. */
export function trimContext(messages) {
  condenseOldPages(messages);
  while (contextSize(messages) > MAX_CONTEXT_CHARS && messages.length > MIN_MESSAGES_KEPT) {
    if (!dropOldestToolTurn(messages)) break;
  }
}
