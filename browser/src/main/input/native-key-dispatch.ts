/** Scope agent-key provenance to one synchronous, browser-owned native dispatch, never an async typing run. */
import type { KeyboardInputEvent, WebContents } from 'electron';

/** Exact contents currently dispatching a native key; unrelated views and real user input remain fenced. */
const dispatching = new WeakSet<object>();

/** The shortcut fence can distinguish browser-dispatched keys without consulting page-controlled data. */
export const isNativeKeyDispatch = (contents: object): boolean => dispatching.has(contents);

/** Native before-input-event callbacks run within sendInputEvent; always revoke the grant before returning. */
export function sendNativeKey(contents: Pick<WebContents, 'sendInputEvent'>, event: KeyboardInputEvent): void {
  const nested = dispatching.has(contents);
  dispatching.add(contents);
  try {
    contents.sendInputEvent(event);
  } finally {
    if (!nested) dispatching.delete(contents);
  }
}
