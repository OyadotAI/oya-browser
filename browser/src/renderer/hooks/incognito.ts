/**
 * Whether this shell window is incognito, and whether incognito is offered at
 * all (not in a governed or headless browser). Both ride in the page's address
 * and are fixed for the window's lifetime (public/first-paint.js), so they are
 * plain functions rather than hooks.
 */

/** The commands that hand work to an agent or open the workspace: none of them is offered in an incognito window. */
const AGENT_COMMANDS: ReadonlySet<string> = new Set([
  'tools',
  'ask',
  'record',
  'playbooks',
  'routines',
  'inspect',
  'network',
  'source',
]);
/** The command that opens an incognito window, under its shortcut and its palette names. */
const INCOGNITO_COMMANDS: ReadonlySet<string> = new Set(['new-incognito-window', 'newIncognitoWindow']);

/** The first paint's marks on the root; none outside a shell page (view-model tests run without a DOM). */
const marks = (): DOMStringMap | undefined => globalThis.document?.documentElement.dataset;

/** True in an incognito window. */
export const incognito = (): boolean => marks()?.incognito === 'true';

/** True when this browser offers incognito windows. */
export const incognitoAvailable = (): boolean => marks()?.incognitoAvailable === 'true';

/** Whether a command (palette id or shortcut name) is offered in this window, so nothing listed does nothing. */
export const commandOffered = (name: string): boolean =>
  !(incognito() && AGENT_COMMANDS.has(name)) && !(INCOGNITO_COMMANDS.has(name) && !incognitoAvailable());
