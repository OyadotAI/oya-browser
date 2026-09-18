/**
 * Keeps a long run's conversation inside the model's context window by dropping
 * the oldest tool results first.
 */
import { MAX_CONTEXT_CHARS, MIN_MESSAGES_KEPT } from './constants.ts';

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

/** Trim old tool results if context is getting too large. */
export function trimContext(messages) {
  while (contextSize(messages) > MAX_CONTEXT_CHARS && messages.length > MIN_MESSAGES_KEPT) {
    if (!dropOldestToolTurn(messages)) break;
  }
}
