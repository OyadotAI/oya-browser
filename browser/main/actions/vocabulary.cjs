/**
 * The actions this app does, as it announces them to the server when it
 * connects, sorted, one spelling each. A plain literal with no dependencies:
 * the server's test reads it to hold the server's Oya column equal to it,
 * and this app's test holds it equal to the keys of its command maps.
 * Internal actions (evaluate_raw, record, run_script) are left out: only the server sends
 * them, and the browser detail never lists them.
 */
const OYA_ACTIONS = [
  'analyze',
  'back',
  'click',
  'click_coordinates',
  'close_tab',
  'double_click',
  'drag',
  'forward',
  'handle_dialog',
  'hover',
  'keyboard_type',
  'list_tabs',
  'mouse_move',
  'navigate',
  'open_tab',
  'press_key',
  'read_console',
  'read_network',
  'read_page',
  'reload',
  'screenshot',
  'scroll',
  'select',
  'switch_tab',
  'type',
  'wait',
  'workflow',
];

module.exports = { OYA_ACTIONS };
