/**
 * Numbers and fixed text for the CDP front door: its waits and limits, the
 * error it answers a refused command with, and what each refusal says.
 */

/** A tab opened through the front door is used after this even if its first load has not settled. */
export const FRONT_DOOR_TAB_WAIT_MS = 15_000;

/** Largest CDP message the front door accepts (256 MiB): a full-page screenshot is big. */
export const FRONT_DOOR_MAX_PAYLOAD = 268_435_456;

/** JSON-RPC's generic server error, as Chromium answers a refused command. */
export const CDP_SERVER_ERROR = -32000;

/** What a remote CDP caller is told when it asks to read or write this computer's files. */
export const LOCAL_FILES_UNAVAILABLE = 'Files on this computer cannot be read or written from a remote connection.';

/** What a remote CDP caller is told when it asks for something that reaches past the page to the computer. */
export const REMOTE_UNAVAILABLE =
  'This command reaches the computer, not the page, so a remote connection cannot send it.';

/** What any CDP client is told when a command would reach Chromium around the front door and its rules. */
export const AROUND_THE_DOOR =
  'This browser does not take this command: it would reach Chromium around the front door.';

/** HTTP statuses the front door answers with, by name. */
export const Status = { OK: 200, BAD_REQUEST: 400, FORBIDDEN: 403, METHOD_NOT_ALLOWED: 405, BAD_GATEWAY: 502 } as const;
