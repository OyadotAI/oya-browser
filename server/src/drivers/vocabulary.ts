/**
 * The one list of what a browser can be told to do, and which kinds of browser
 * can do it. The server checks every command against it before the browser is
 * asked, so a caller learns at once that an action does not exist, or does not
 * exist on this kind, instead of a timeout or a driver's own wording. The
 * desktop keeps its own literal of the Oya column (browser/main/actions/
 * vocabulary.cjs); a test holds the two equal, since the server's image does
 * not carry the desktop's code and cannot read it at run time.
 */

/** One action: which kinds do it, whether only the server may send it, and the CDP driver's own name for it. */
export type Action = {
  /** An Oya browser (desktop or cloud) does it. */
  oya: boolean;
  /** A CDP browser this server dials does it. */
  cdp: boolean;
  /** Sent by the server on its own behalf; a caller may not. */
  internal?: true;
  /** The CDP driver's name, where it grew a hyphenated one. */
  cdpName?: string;
};

/** Done by both kinds. */
const BOTH = { oya: true, cdp: true };
/** Done only by an Oya browser. */
const OYA = { oya: true, cdp: false };
/** Done only by a CDP browser. */
const CDP = { oya: false, cdp: true };

/** Every action, by the one spelling the SDK sends. */
export const VOCABULARY: Record<string, Action> = {
  analyze: BOTH,
  click: BOTH,
  click_coordinates: { ...BOTH, cdpName: 'click-coords' },
  close_tab: { ...BOTH, cdpName: 'close-tab' },
  double_click: BOTH,
  drag: BOTH,
  handle_dialog: BOTH,
  hover: BOTH,
  keyboard_type: BOTH,
  list_tabs: { ...BOTH, cdpName: 'list-tabs' },
  mouse_move: BOTH,
  navigate: BOTH,
  open_tab: { ...BOTH, cdpName: 'new-tab' },
  press_key: { ...BOTH, cdpName: 'press-key' },
  read_page: BOTH,
  // Internal: arbitrary JavaScript in the page. The agent's run_script tool refuses anything
  // that writes (agent/page-tool-handlers.ts); a caller sending it straight would not be checked.
  run_script: { ...BOTH, internal: true },
  screenshot: BOTH,
  scroll: BOTH,
  select: BOTH,
  switch_tab: { ...BOTH, cdpName: 'switch-tab' },
  type: BOTH,
  wait: BOTH,
  workflow: OYA,
  read_console: OYA,
  read_network: OYA,
  back: BOTH,
  forward: BOTH,
  reload: BOTH,
  cookies: CDP,
  evaluate_raw: { ...BOTH, internal: true },
  record: { ...BOTH, internal: true },
  evaluate: { oya: false, cdp: false, internal: true },
};

/**
 * Other spellings callers already send, by the action they mean: the CDP
 * driver's hyphenated names and the older agent-tool names. Accepted, and
 * forwarded as sent; the canonical name is used for the check only.
 */
export const SPELLINGS: Record<string, string> = {
  'click-coords': 'click_coordinates',
  'close-tab': 'close_tab',
  'list-tabs': 'list_tabs',
  'new-tab': 'open_tab',
  new_tab: 'open_tab',
  'press-key': 'press_key',
  'switch-tab': 'switch_tab',
  read_elements: 'read_page',
  'scroll-up': 'scroll',
  'scroll-down': 'scroll',
  'scroll-top': 'scroll',
  'scroll-bottom': 'scroll',
};

/** The browser kinds, as the registry names them. */
export type Kind = 'oya' | 'cdp';

/**
 * The action `name` means, or null when it names none. Only a string is
 * looked up: `["evaluate_raw"]` would read as the same key in a map.
 */
export function canonical(name: unknown): string | null {
  if (typeof name !== 'string') return null;
  if (Object.hasOwn(VOCABULARY, name)) return name;
  return Object.hasOwn(SPELLINGS, name) ? SPELLINGS[name] : null;
}

/** The public actions `kind` does, sorted, one spelling each. */
export const actionsFor = (kind: Kind): string[] =>
  Object.keys(VOCABULARY)
    .filter((name) => VOCABULARY[name][kind] && !VOCABULARY[name].internal)
    .sort();

/** The kinds that do `action`. */
export const supportedOn = (action: string): Kind[] =>
  (['oya', 'cdp'] as const).filter((kind) => VOCABULARY[action]?.[kind]);

/** Whether only the server may send `name`. */
export const isInternal = (name: unknown): boolean => {
  const action = canonical(name);
  return action !== null && VOCABULARY[action].internal === true;
};
