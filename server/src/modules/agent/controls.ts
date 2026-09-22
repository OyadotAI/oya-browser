/**
 * What every page action shares: how an element id becomes a selector, what the
 * model hears when an id has gone, and the page an action left behind.
 */
import { sendCommand } from '../browsers/socket.ts';
import { setElements } from './recorder.ts';
import { analysisText } from './element-index.ts';
import { changeNote } from './changes.ts';
import { PAGE_FORMAT } from './constants.ts';

/** What the model is told when an element id is not in the latest analysis. */
export const ELEMENT_GONE = 'Error: Element not found. Call analyze_page and use a current id.';

/** The CSS selector for an element id from the latest analysis. */
export const byId = (elementId) => `[data-ac-id="${elementId}"]`;

/**
 * The elements of the page an action left behind, added to what the action
 * says, with what it changed (changes.ts). Without it the model must call
 * analyze_page after every click merely to learn the new ids, which costs a
 * round trip and a whole page; the elements alone are short and are what it
 * needs to act again. `expected` says the action should visibly change the page.
 */
export async function withControls(browserId, said, expected = false) {
  const r = await sendCommand(browserId, 'analyze', { format: PAGE_FORMAT });
  if (!r.ok || !r.data?.elements?.length) return said;
  setElements(browserId, r.data.elements);
  const change = changeNote(browserId, r.data, expected);
  return `${said}${change ? `. ${change}` : ''}\n\n${analysisText(r.data, { content: false })}`;
}
