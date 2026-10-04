/**
 * The shared vocabulary of a workflow: which actions exist, which need a
 * target, and which variable names are allowed.
 */

/** Every action a step may take. */
export const ACTIONS: ReadonlySet<string> = new Set([
  'navigate',
  'go_back',
  'go_forward',
  'click',
  'double_click',
  'hover',
  'type',
  'select_option',
  'upload_file',
  'press_key',
  'scroll',
  'wait',
  'assert_visible',
  'assert_text',
  'assert_value',
  'assert_url',
  'assert_page',
  'checkpoint',
]);

/** Actions that act on an element, so cannot run without a target. */
export const TARGETED: readonly string[] = [
  'click',
  'double_click',
  'hover',
  'type',
  'select_option',
  'upload_file',
  'assert_visible',
  'assert_text',
  'assert_value',
  'wait',
];

/**
 * Actions whose target may be hidden: a file input is almost always hidden behind
 * its button. Every other target is matched among visible elements only, since a
 * hidden one cannot be clicked or typed into (a collapsed menu repeats link names).
 */
export const HIDDEN_TARGET_ACTIONS: readonly string[] = ['upload_file'];

/** The Playwright code that narrows a step's locator to what it may act on. */
export const visibleOnly = (action: string): string =>
  HIDDEN_TARGET_ACTIONS.includes(action) ? '' : '.filter({visible:true})';

/** Names that would reach an object's prototype if used as keys. */
export const RESERVED: readonly string[] = ['__proto__', 'constructor', 'prototype'];

/** A variable name: an identifier of at most 64 characters. */
export const VARIABLE_NAME = /^[A-Za-z_]\w{0,63}$/;

/** A `{{variable}}` placeholder in a step value. */
export const PLACEHOLDER = /\{\{([A-Za-z_]\w*)\}\}/g;

/** Whether `name` can be used as a variable. */
export const isVariableName = (name: unknown): name is string =>
  typeof name === 'string' && VARIABLE_NAME.test(name) && !RESERVED.includes(name);
