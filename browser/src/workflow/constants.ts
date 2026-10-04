/**
 * Every number the workflow model uses, by name: limits on recorded drafts,
 * step timing, and how much a redacted value keeps.
 */

/** Diagnostics: how deep, how long and how many entries a redacted value keeps. */
export const REDACT = { MAX_DEPTH: 8, MAX_TEXT: 4000, MAX_ITEMS: 100, MIN_SECRET_LENGTH: 2 } as const;

/** Draft limits: the most a normalized draft or step may hold. */
export const DRAFT = {
  SCHEMA_VERSION: 2,
  MAX_STEPS: 500,
  MAX_VARIABLES: 100,
  MAX_NAME: 64,
  MAX_DESCRIPTION: 2000,
  MAX_VALUE: 16000,
  MAX_CANDIDATES: 12,
  MAX_CANDIDATE_VALUE: 4000,
  MAX_TAB_NAME: 100,
  MAX_FRAMES: 10,
} as const;

/** Step timing, in milliseconds, and the default scroll distance in pixels. */
export const STEP = {
  MIN_TIMEOUT: 500,
  DEFAULT_TIMEOUT: 15000,
  MAX_TIMEOUT: 90000,
  DEFAULT_SCROLL: 500,
  MAX_SCROLL: 100000,
  // How many of a step's recorded targets its generated code tries in turn.
  MAX_FALLBACKS: 3,
  // The pause between keys typed into a search box with suggestions, as a person types.
  TYPE_DELAY_MS: 40,
} as const;
