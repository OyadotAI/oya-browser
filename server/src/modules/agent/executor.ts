/**
 * Runs one browser tool by name for the model, as text, with any dialog it
 * raised appended.
 */
import { takeDialogNote } from '../browsers/socket.ts';
import { TOOL_HANDLERS } from './tool-handlers.ts';

/**
 * Execute a tool by name and return the result as a string for the LLM.
 *
 * A dialog that fired during the action is appended here rather than in each
 * case: an alert's text is usually the reason the action did not do what the
 * model expected, and losing it is what left runs stuck on a blocked page.
 */
export async function executeTool(browserId, name, args, files = {}) {
  const out = await runTool(browserId, name, args, files);
  const note = takeDialogNote(browserId);
  return note ? `${out}\n\n${note}` : out;
}

/** The handler lookup behind executeTool; errors come back as text for the model rather than throwing. */
async function runTool(browserId, name, args, files = {}) {
  try {
    if (!Object.hasOwn(TOOL_HANDLERS, name)) return `Unknown tool: ${name}`;
    return await TOOL_HANDLERS[name](browserId, args, files);
  } catch (err) {
    return `Error: ${err.message}`;
  }
}
