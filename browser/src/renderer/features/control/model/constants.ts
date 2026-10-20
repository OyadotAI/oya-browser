/**
 * The control bar's fixed values: what the status says in each mode, and the
 * toolbar controls the watch-only guard holds back while an agent drives.
 */

/** What the status says in each control mode. `human` depends on whether it is you. */
export const CONTROL_LABELS: Readonly<Record<string, string>> = {
  agent: 'AI Mode',
  paused: 'Automation paused',
  offline: 'Offline · your control',
  disconnected: 'Disconnected',
  unavailable: 'Control unavailable',
  taking: 'Taking control…',
};

/** The status before any control state has come (matches the first paint). */
export const OFFLINE_MODE = 'offline';

/** The record button: the one page action that takes control instead of being refused. */
export const RECORD_ID = 'record-toggle';

/** New tab: a tab of the person's own, open to them beside an agent's tab. */
export const NEW_TAB_ID = 'btn-new-tab';

/** Toolbar controls that act on the page, marked blocked while it is watch-only. */
export const PAGE_ACTIONS = ['btn-reload', 'btn-zoom', NEW_TAB_ID, RECORD_ID] as const;

/** The address bar, read-only while watch-only. */
export const ADDRESS_ID = 'url-bar';

/** The Human Mode side of the switch, which a refused click points at. */
export const ACTION_ID = 'control-action';

/** Clicks refused while watch-only (matched before the controls' own listeners run). */
export const GUARDED = '#btn-back, #btn-forward, #btn-reload, #btn-zoom, #btn-new-tab, .tab-close, #record-toggle';

/** The words the bar says outside the mode labels. */
export const TEXT = {
  returning: 'Returning control…',
  mine: 'Human Mode',
  other: 'Another operator',
  checking: 'Checking control…',
  interactive: 'You can interact with this page.',
  watchOnly: 'OyaAI is driving this page. Switch to Human Mode to use it.',
  take: 'Switch to Human Mode: you drive this page',
  release: 'Switch to AI Mode: hand the page back to the agent',
  failed: 'Could not change control.',
} as const;

/** The switch's two sides. */
export const MODES = { agent: 'AI Mode', human: 'Human Mode' } as const;

/** The AI Mode side after a pause: it resumes the agent. */
export const RESUME_TITLE = 'Switch to AI Mode: the agent picks up where it paused';
