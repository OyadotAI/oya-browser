/**
 * Keeps a long run's conversation inside the model's context window by dropping
 * the oldest tool results first.
 */
import { MAX_CONTEXT_CHARS, MIN_MESSAGES_KEPT, PAGES_KEPT_IN_FULL, PAGE_RESULT_CHARS } from './constants.ts';

/** The conversation's size in characters (~4 per token), tool calls included. */
const contextSize = (messages) =>
  messages.reduce((sum, m) => sum + (m.content?.length || 0) + JSON.stringify(m.tool_calls || '').length, 0);

/** The index just past the run of tool results starting at `toolIdx`. */
function toolResultsEnd(messages, toolIdx) {
  let endIdx = toolIdx;
  while (endIdx < messages.length && messages[endIdx].role === 'tool') endIdx++;
  return endIdx;
}

/** Drops the oldest tool result, with the assistant turn that asked for it; false when there is none. */
function dropOldestToolTurn(messages) {
  const toolIdx = messages.findIndex((m, i) => i > 0 && m.role === 'tool');
  if (toolIdx === -1) return false;
  // Also remove the assistant message with tool_calls right before it, and every result it got
  const prevIdx = toolIdx - 1;
  if (prevIdx > 0 && messages[prevIdx].role === 'assistant' && messages[prevIdx].tool_calls)
    messages.splice(prevIdx, toolResultsEnd(messages, toolIdx) - prevIdx);
  else messages.splice(toolIdx, 1);
  return true;
}

/** Whether a tool result is a page read rather than a short answer. */
const isPageRead = (m) => m.role === 'tool' && (m.content?.length || 0) >= PAGE_RESULT_CHARS;

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
  const reads = messages.filter(isPageRead);
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
