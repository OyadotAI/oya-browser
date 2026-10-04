/**
 * The control bar's fixed values: what the status says in each mode, and the
 * toolbar controls the watch-only guard holds back while an agent drives.
 */

/** What the status says in each control mode. `human` depends on whether it is you. */
export const CONTROL_LABELS: Readonly<Record<string, string>> = {
  agent: 'Agent control',
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

/** Toolbar controls that act on the page, marked blocked while it is watch-only. */
export const PAGE_ACTIONS = ['btn-reload', 'btn-new-tab', RECORD_ID] as const;

/** The address bar, read-only while watch-only. */
export const ADDRESS_ID = 'url-bar';

/** Take control / Release, which a refused click points at. */
export const ACTION_ID = 'control-action';

/** Clicks refused while watch-only (matched before the controls' own listeners run). */
export const GUARDED = '#btn-back, #btn-forward, #btn-reload, #btn-new-tab, .tab-close, #record-toggle';

/** The words the bar says outside the mode labels. */
export const TEXT = {
  returning: 'Returning control…',
  mine: 'You’re in control',
  other: 'Another operator',
  checking: 'Checking control…',
  interactive: 'You can interact with this page.',
  watchOnly: 'Page is watch-only. Take control to interact.',
  take: 'Take control',
  release: 'Release to agent',
  failed: 'Could not change control.',
} as const;
