/**
 * Fixed values for the project switcher: storage keys, timings, HTTP codes,
 * lengths and the class strings its panels share.
 */
import type { PickerState } from './types';

/** sessionStorage key for the open project's id. */
export const PROJECT_ID = 'oya_project_id';
/** sessionStorage key for the open project's one-hour credential. */
export const PROJECT_CREDENTIAL = 'oya_project_credential';

/** The HTTP answers the switcher tells apart. */
export const HttpStatus = {
  /** The credential was refused. */
  UNAUTHORIZED: 401,
  /** The account may not open the project. */
  FORBIDDEN: 403,
  /** The project is not there. */
  NOT_FOUND: 404,
  /** The project was deleted. */
  GONE: 410,
} as const;

/** Answers that mean this account can no longer open the project, as opposed to a passing outage. */
export const GONE_STATUSES: number[] = [
  HttpStatus.UNAUTHORIZED,
  HttpStatus.FORBIDDEN,
  HttpStatus.NOT_FOUND,
  HttpStatus.GONE,
];

/** Milliseconds in a minute. */
const MS_PER_MINUTE = 60_000;
/** Minutes between renewals: the credential lasts an hour, so renew well before that. */
const RENEW_MINUTES = 45;
/** How often the open project's credential is renewed. */
export const RENEW_INTERVAL_MS = RENEW_MINUTES * MS_PER_MINUTE;

/** Hex characters of the key's SHA-256 in a project id, as the server derives it. */
export const PROJECT_ID_HEX_CHARS = 24;
/** Radix for hex digits. */
export const HEX_RADIX = 16;
/** Hex digits per byte. */
export const HEX_BYTE_WIDTH = 2;
/** Letters of the project name in its badge. */
export const BADGE_LETTERS = 2;
/** Longest project name the form accepts. */
export const NAME_MAX_LENGTH = 100;

/** Custom event the API client fires when the server refuses the credential. */
export const CREDENTIAL_GONE_EVENT = 'oya:credential-gone';

/** The popover as it starts: closed, on the project list. */
export const INITIAL_PICKER: PickerState = {
  open: false,
  form: null,
  target: null,
  revealedKey: '',
  name: '',
  secret: '',
  query: '',
  error: '',
  restore: null,
  copied: false,
  pending: null,
  busy: false,
};

/** Text inputs in the switcher. */
export const FIELD_CLASS =
  'w-full h-9 rounded-md border border-border bg-transparent px-3 text-sm text-text placeholder:text-text-dim focus:outline-none focus:ring-1 focus:ring-accent/50';

/** Full-width action buttons in the switcher. */
export const ACTION_CLASS =
  'flex w-full items-center gap-3 rounded-lg px-3 py-3 text-left text-sm transition-colors hover:bg-text/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/50 disabled:opacity-50';
