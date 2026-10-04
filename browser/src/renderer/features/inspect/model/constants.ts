/**
 * The Inspect panes' fixed words and tables: what the Actions pane reads from
 * each field and checks before sending, which action Enter runs in each field,
 * the Activity log's filters and directions, the Source pane's formats, and the
 * icon each Actions button shows.
 * Numbers come from core/constants.ts.
 */
import type { IconName } from '../../../ui/index.ts';

/** The Actions pane's input fields, by id. */
export const ACTION_FIELDS = [
  'action-click-id',
  'action-type-text',
  'action-url',
  'action-key',
  'action-wait-sel',
  'action-cx',
  'action-cy',
] as const;

/** One of the Actions pane's input fields. */
export type ActionField = (typeof ACTION_FIELDS)[number];

/** What the fields hold, by id. */
export type ActionFields = Readonly<Record<ActionField, string>>;

/** How each action reads its parameters from the fields. */
export const ACTION_PARAMS: Readonly<Record<string, (f: ActionFields) => Record<string, string>>> = {
  navigate: (f) => ({ url: f['action-url'].trim() }),
  click: (f) => ({ element_id: f['action-click-id'].trim() }),
  type: (f) => ({ element_id: f['action-click-id'].trim(), text: f['action-type-text'] }),
  'press-key': (f) => ({ key: f['action-key'].trim() }),
  hover: (f) => ({ element_id: f['action-click-id'].trim() }),
  'click-coords': (f) => ({ x: f['action-cx'].trim(), y: f['action-cy'].trim() }),
  wait: (f) => ({ selector: f['action-wait-sel'].trim() }),
};

/** Whether `value` is a whole element number. */
const isElementNumber = (value: string): boolean => /^\d+$/.test(value);

/** Whether `value` is a real coordinate: an empty field is not 0. */
const isCoordinate = (value: string): boolean => value !== '' && Number.isFinite(Number(value));

/** Why a parameter cannot be sent, by its name ('' when it can). */
export const ACTION_CHECKS: Readonly<Record<string, (value: string) => string>> = {
  element_id: (v) => (isElementNumber(v) ? '' : 'Enter an element number, such as 12'),
  x: (v) => (isCoordinate(v) ? '' : 'Enter X and Y as numbers'),
  y: (v) => (isCoordinate(v) ? '' : 'Enter X and Y as numbers'),
};

/** The action Enter runs in each field: its row's button, or Click (Type in the text field). */
export const FIELD_ACTION: Readonly<Record<ActionField, string>> = {
  'action-click-id': 'click',
  'action-type-text': 'type',
  'action-url': 'navigate',
  'action-key': 'press-key',
  'action-wait-sel': 'wait',
  'action-cx': 'click-coords',
  'action-cy': 'click-coords',
};

/** Actions that only look, so they run while the agent holds the page. */
export const UNGUARDED: readonly string[] = ['screenshot', 'list-tabs'];

/** What an action's name reads as in a result's heading, where it is not the name capitalized. */
export const ACTION_TITLES: Readonly<Record<string, string>> = {
  'click-coords': 'Click at',
  'press-key': 'Press key',
  'list-tabs': 'Tabs',
  'new-tab': 'New tab',
};

/** The words the Actions pane shows. */
export const ACTIONS_TEXT = {
  takeControl: 'Take control to use this',
  done: 'done',
  failed: 'failed',
  unknownError: 'Unknown error',
  cut: 'Showing the first part. Copy takes all of it.',
  shotAlt: 'Screenshot of the page',
  hintStart: 'Analyze the page to see its element numbers.',
  hintPick: 'Pick an element, then Click, Hover or Type.',
  hintNone: 'Nothing to act on in view.',
} as const;

/** What a Copy button says: at rest, after a copy, and when copying failed. */
export const COPY_TEXT = { idle: 'Copy', copied: 'Copied', failed: 'Copy failed' } as const;

/** What a filter reads of an entry. */
export interface FilterEntry {
  /** 'in' or 'out'. */
  dir: string;
  /** The message type. */
  type: string;
}

/** The Activity log's filters: which entries each shows. */
export const NET_FILTERS: Readonly<Record<string, (entry: FilterEntry) => boolean>> = {
  all: () => true,
  in: (entry) => entry.dir === 'in',
  out: (entry) => entry.dir === 'out',
  cmd: (entry) => entry.type.startsWith('cmd:') || entry.type === 'auth',
  result: (entry) => entry.type.startsWith('result:'),
};

/** The filter buttons, as they read. */
export const NET_FILTER_LABELS: readonly [string, string][] = [
  ['all', 'All'],
  ['in', 'From server'],
  ['out', 'To server'],
  ['cmd', 'Commands'],
  ['result', 'Results'],
];

/** How each direction reads. */
export const DIRECTIONS = { in: 'From server', out: 'To server' } as const;

/** The type an entry without one is shown as. */
export const DEFAULT_ENTRY_TYPE = 'message';

/** What each page format is called above the read. */
export const FORMAT_LABELS: Readonly<Record<string, string>> = { markdown: 'Markdown', toon: 'TOON', jsonl: 'JSONL' };

/** The format the Source pane starts at. */
export const DEFAULT_FORMAT = 'markdown';

/** The words the Source pane shows. */
export const SOURCE_TEXT = {
  prompt: 'The current page, as the agent reads it. Refresh to read it.',
  readPrompt: 'Refresh, or right-click the page and choose View Page Source.',
  noHtmlYet: 'No HTML yet.',
  noHtml: 'No HTML',
  nothing: 'Nothing to read on this page',
  readFrom: (url: string) => `Read from ${url}`,
  changed: 'The page changed. Refresh to read it again.',
  couldNotRead: (why: string) => `Could not read the page: ${why}`,
  couldNotFormat: (why: string) => `Could not show this format: ${why}`,
  unknownError: 'Unknown error',
  refresh: 'Refresh',
  reading: 'Reading…',
} as const;

/** The icon each Actions button shows. */
export const ACTION_ICONS: Readonly<Record<string, IconName>> = {
  analyze: 'scan',
  screenshot: 'camera',
  reload: 'reload',
  'scroll-down': 'down',
  'scroll-up': 'up',
  'list-tabs': 'panel',
  'new-tab': 'plus',
};
