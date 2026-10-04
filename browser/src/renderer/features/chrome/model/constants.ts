/**
 * The window chrome's fixed values: the appearances, how a server command
 * shows in the activity log, the launch's phases, and the words.
 */

/** The appearances a person can pick. */
export const THEMES = ['system', 'light', 'dark'] as const;

/** An appearance a person can pick. */
export type ThemePreference = (typeof THEMES)[number];

/** A theme the shell draws. */
export type AppliedTheme = 'light' | 'dark';

/** A command from the server, as the activity log names it. */
export const AGENT_COMMAND = /^cmd:/;

/** The activity log's direction for messages from the server. */
export const INCOMING = 'in';

/** The launch: playing from the first paint, dissolving, then gone. */
export type LaunchPhase = 'playing' | 'leaving' | 'gone';

/** What the appearance picker says. */
export const TEXT = {
  appearance: 'Appearance',
  themes: { system: 'System', light: 'Light', dark: 'Dark' },
} as const;

/** Where the launch's orb lands on the welcome screen, when the start page is not up. */
export const WELCOME_ORB = 'body.mode-setup .welcome-stage .oya-orb';
