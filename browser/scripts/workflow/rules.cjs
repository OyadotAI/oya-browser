/**
 * The shared vocabulary of a workflow: which actions exist, which need a
 * target, and which variable names are allowed.
 */

/** Every action a step may take. */
const ACTIONS = new Set([
  'navigate',
  'click',
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
  'checkpoint',
]);

/** Actions that act on an element, so cannot run without a target. */
const TARGETED = [
  'click',
  'type',
  'select_option',
  'upload_file',
  'assert_visible',
  'assert_text',
  'assert_value',
  'wait',
];

/** Names that would reach an object's prototype if used as keys. */
/**
 * Actions whose target may be hidden: a file input is almost always hidden behind
 * its button. Every other target is matched among visible elements only, since a
 * hidden one cannot be clicked or typed into (a collapsed menu repeats link names).
 */
const HIDDEN_TARGET_ACTIONS = ['upload_file'];

/** The Playwright code that narrows a step's locator to what it may act on. */
const visibleOnly = (action) => (HIDDEN_TARGET_ACTIONS.includes(action) ? '' : '.filter({visible:true})');

const RESERVED = ['__proto__', 'constructor', 'prototype'];

/** A variable name: an identifier of at most 64 characters. */
const VARIABLE_NAME = /^[A-Za-z_]\w{0,63}$/;

/** A `{{variable}}` placeholder in a step value. */
const PLACEHOLDER = /\{\{([A-Za-z_]\w*)\}\}/g;

/** Whether `name` can be used as a variable. */
const isVariableName = (name) => typeof name === 'string' && VARIABLE_NAME.test(name) && !RESERVED.includes(name);

module.exports = {
  ACTIONS,
  TARGETED,
  HIDDEN_TARGET_ACTIONS,
  visibleOnly,
  RESERVED,
  VARIABLE_NAME,
  PLACEHOLDER,
  isVariableName,
};
