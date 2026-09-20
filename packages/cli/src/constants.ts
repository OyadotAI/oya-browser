/**
 * Every number the CLI's own commands run on, by name. The prompt layer and
 * the install wizard keep theirs in their own folders.
 */

/** argv entries before the command: node and the script. */
export const ARGV_SKIP = 2;
/** Indentation of pretty-printed JSON output. */
export const JSON_INDENT = 2;
/** Where `oya` commands point when neither a flag, the environment nor the saved file says otherwise. */
export const DEFAULT_BASE_URL = 'https://browser.getoya.ai';
/** The saved config holds a credential: owner read/write only. */
export const CONFIG_FILE_MODE = 0o600;

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
  /** A quota or a persona concurrency cap. */
  TOO_MANY_REQUESTS: 429,
} as const;
