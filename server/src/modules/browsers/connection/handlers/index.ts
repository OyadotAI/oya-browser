/**
 * Message type → handler: everything an authenticated browser can say. The
 * connection looks the type up here; an unknown type is ignored.
 */
import { ping, pong, frame, cmdResult, proxyBytes, cdpRelay } from './liveness.ts';
import { cookieDump, cookieChanged, cookiePull, storageChanged, profileFlush } from './cookies.ts';
import { desktopControlMessage } from './desktop-control.ts';
import type { Handler } from './types.ts';

export type { Connection, Handler } from './types.ts';

/** The handler for each message type. */
export const HANDLERS: Record<string, Handler> = {
  ping,
  pong,
  frame,
  cmd_result: cmdResult,
  proxy_bytes: proxyBytes,
  cdp: cdpRelay,
  cdp_opened: cdpRelay,
  cdp_closed: cdpRelay,
  cookie_dump: cookieDump,
  cookie_changed: cookieChanged,
  cookie_pull: cookiePull,
  storage_changed: storageChanged,
  profile_flush: profileFlush,
  desktop_control: desktopControlMessage,
};
