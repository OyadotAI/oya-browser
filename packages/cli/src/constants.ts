/**
 * Every number the CLI's own commands run on, by name. The prompt layer and
 * the install wizard keep theirs in their own folders.
 */

/** argv entries before the command: node and the script. */
export const ARGV_SKIP = 2;

/** How many of a key's last characters whoami shows, so two keys can be told apart without showing either. */
export const KEY_TAIL = 4;
/** A key shorter than this shows none of itself: four characters would be most of it. */
export const MIN_KEY_FOR_TAIL = 12;

/** How long signing in may take before it is given up, as the SDK allows any call. */
export const LOGIN_TIMEOUT_MS = 60_000;

/** How `oya` exits, so a script can tell "fix the command" from "retry or look into it". */
export const ExitCode = {
  /** Done. */
  DONE: 0,
  /** The request failed: the server or the browser refused it, or no answer came. */
  FAILED: 1,
  /** The command could not be run as typed; nothing was sent. */
  USAGE: 2,
} as const;
/** Indentation of pretty-printed JSON output. */
export const JSON_INDENT = 2;
/** Where `oya` commands point when neither a flag, the environment nor the saved file says otherwise. */
export const DEFAULT_BASE_URL = 'https://oyabrowser.com';
/** The saved config holds a credential: owner read/write only. */
export const CONFIG_FILE_MODE = 0o600;
/** Permissions for a file of exported cookies: owner only, because it holds live sessions. */
export const PRIVATE_FILE_MODE = 0o600;

/** Column widths in `oya ls`. */
export const LsColumns = {
  /** Provider name. */
  PROVIDER: 14,
  /** Persona name. */
  PERSONA: 14,
  /** Browser name. */
  NAME: 18,
  /** Current URL, after the scheme is dropped. */
  URL: 40,
} as const;

/** Column widths and row count in `oya status`. */
export const StatusColumns = {
  /** Recent actions shown. */
  RECENT: 10,
  /** Action name column. */
  ACTION: 18,
} as const;

/** Persona name column in `oya personas`. */
export const PERSONA_NAME_WIDTH = 20;

/** HTTP statuses the CLI explains when a command fails. */
export const Status = {
  /** The API key was rejected. */
  UNAUTHORIZED: 401,
  /** A role refusal, or the auth layer's answer for an unknown key. */
  FORBIDDEN: 403,
  /** An endpoint another browser already holds. */
  CONFLICT: 409,
  /** A quota or a persona concurrency cap. */
  TOO_MANY_REQUESTS: 429,
} as const;
