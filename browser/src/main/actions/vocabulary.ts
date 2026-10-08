/**
 * The actions this app does, as it announces them to the server when it
 * connects, sorted, one spelling each. A plain literal with no dependencies:
 * the server's test reads it to hold the server's Oya column equal to it,
 * and this app's test holds it equal to the keys of its command maps.
 * Internal actions (evaluate_raw, record, run_script) are left out: only the server sends
 * them, and the browser detail never lists them.
 */
export const OYA_ACTIONS: readonly string[] = [
  'add_bookmark',
  'analyze',
  'back',
  'clear_history',
  'click',
  'click_coordinates',
  'close_tab',
  'dismiss_notification',
  'double_click',
  'drag',
  'forward',
  'handle_dialog',
  'hover',
  'keyboard_type',
  'list_bookmarks',
  'list_closed_tabs',
  'list_keyboard_shortcuts',
  'list_notifications',
  'list_tabs',
  'mark_notifications_read',
  'mouse_move',
  'navigate',
  'open_tab',
  'press_key',
  'read_console',
  'read_network',
  'read_page',
  'reload',
  'remove_bookmark',
  'reopen_closed_tab',
  'screenshot',
  'scroll',
  'search_history',
  'select',
  'switch_tab',
  'type',
  'wait',
  'workflow',
];
