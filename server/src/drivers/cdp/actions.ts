/**
 * The action vocabulary a CDP browser speaks, and how the Oya client's
 * spellings map onto it.
 */

/** Actions a CDP browser supports, in both the hyphenated and the underscored spelling. */
export const CDP_CAPABILITIES = new Set([
  'handle_dialog',
  'navigate',
  'reload',
  'back',
  'forward',
  'screenshot',
  'analyze',
  'read_page',
  'record',
  'click',
  'click-coords',
  'hover',
  'type',
  'select',
  'wait',
  'cookies',
  // Both spellings, because normalise() accepts both. A caller checking this
  // set must not conclude that press_key is unsupported when it is.
  'press-key',
  'press_key',
  'click_coordinates',
  'mouse_move',
  'double_click',
  'drag',
  'keyboard_type',
  'scroll',
  'scroll-up',
  'scroll-down',
  'scroll-top',
  'scroll-bottom',
  'list-tabs',
  'list_tabs',
  'new-tab',
  'open_tab',
  'switch-tab',
  'switch_tab',
  'close-tab',
  'close_tab',
]);

/**
 * One action vocabulary, two spellings. Underscored names come from the Oya
 * client and the agent tools; hyphenated ones are this driver's own.
 */
const ACTION_ALIASES = {
  click_coordinates: 'click-coords',
  press_key: 'press-key',
  list_tabs: 'list-tabs',
  open_tab: 'new-tab',
  new_tab: 'new-tab',
  switch_tab: 'switch-tab',
  close_tab: 'close-tab',
  read_elements: 'read_page',
};

/** Maps an action and its params onto this driver's own vocabulary. */
export function normalise(action, params: any = {}) {
  // `scroll` carries its direction in the params; this driver has it in the name.
  if (action === 'scroll') {
    const direction = String(params.direction || 'down');
    return { action: `scroll-${direction}`, params };
  }
  const mapped = Object.hasOwn(ACTION_ALIASES, action) ? ACTION_ALIASES[action] : action;
  // The Oya client names tabs `tab_id`; every Target.* call here wants `id`.
  const id = params.tab_id ?? params.id;
  return { action: mapped, params: id === undefined ? params : { ...params, id } };
}
