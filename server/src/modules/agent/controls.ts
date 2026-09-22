/**
 * What every page action shares: how an element id becomes a selector, what the
 * model hears when an id has gone, and the page an action left behind.
 */
import { sendCommand } from '../browsers/socket.ts';
import { elementsOf, setElements } from './recorder.ts';
import { analysisText } from './element-index.ts';
import { changeNote } from './changes.ts';
import { PAGE_FORMAT } from './constants.ts';

/** What the model is told when an element id is not in the latest analysis. */
export const ELEMENT_GONE = 'Error: Element not found. Call analyze_page and use a current id.';

/** Said in place of the element index when the page is the one the model already has. */
const UNCHANGED = 'The elements are the ones you already have: the ids you were given still work.';

/** Whether the page carries the same elements, by id and label, as the ones the model holds. */
function sameElements(browserId: string, elements: any[]) {
  const held = elementsOf(browserId);
  if (!held?.length || held.length !== elements.length) return false;
  return elements.every(
    (e, i) => e.id === held[i].id && (e.text || e.name || '') === (held[i].text || held[i].name || ''),
  );
}

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
  const same = sameElements(browserId, r.data.elements);
  setElements(browserId, r.data.elements);
  const change = changeNote(browserId, r.data, expected);
  const head = `${said}${change ? `. ${change}` : ''}`;
  // A page that did not move leaves every id where it was, so sending the index
  // again buys the model nothing and costs it the whole list on every later turn.
  if (same) return `${head}\n\n${UNCHANGED}`;
  return `${head}\n\n${analysisText(r.data, { content: false })}`;
}
