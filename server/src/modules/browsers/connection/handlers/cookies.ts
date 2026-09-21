/**
 * The persona's session state as the browser reports it: cookie jars,
 * localStorage, and saving the profile.
 */
import {
  mergeDump,
  applyChange,
  getForDomains,
  mergeStorage,
  drain as drainLogins,
  summary as loginSummary,
} from '../../../personas/cookies.ts';
import { metrics } from '../../../../platform/metrics.ts';
import * as usage from '../../../../platform/usage.ts';
import { MAX_COOKIE_CHANGES } from '../constants.ts';
import type { Handler } from './types.ts';

/**
 * The full jar on connect, merged into the persona's jar and left there.
 * Fanning it out to every peer was O(pool size) per connect.
 */
export const cookieDump: Handler = ({ browserId, persona }, msg) => {
  if (!Array.isArray(msg.cookies)) return;
  const merged = mergeDump(persona.id, msg.cookies);
  console.log(`[ws] Cookie dump from ${browserId}: ${msg.cookies.length} cookies, jar now ${merged.length}`);
};

/** One change or a batch, as a list. */
const changesIn = (msg) => (Array.isArray(msg.changes) ? msg.changes : msg.change ? [msg.change] : []);

/** Incremental cookie changes, recorded in the jar; peers pull them when they navigate. */
export const cookieChanged: Handler = ({ persona }, msg) => {
  const changes = changesIn(msg);
  for (const change of changes.slice(0, MAX_COOKIE_CHANGES)) applyChange(persona.id, change);
  metrics.cookieChanges.inc({}, changes.length);
};

/** The browser asks for the cookies of the hosts it is about to visit. */
export const cookiePull: Handler = (conn, msg) => {
  const cookies = getForDomains(conn.persona.id, msg.domains || []);
  metrics.cookiePulls.inc({});
  usage.record(conn.apiKey, 'cookie_pulls');
  conn.send({ type: 'cookie_sync', cookies, pullId: msg.pullId });
};

/** localStorage changes for the persona's signed-in origins. */
export const storageChanged: Handler = ({ persona }, msg) => mergeStorage(persona.id, msg.origins);

/** The person saved their profile: persist logins now and say how many were kept. */
export const profileFlush: Handler = (conn) => {
  const answer = (body: object) => conn.isOpen() && conn.send({ type: 'profile_saved', ...body });
  drainLogins()
    .then(() => answer(loginSummary(conn.persona.id)))
    .catch(() => answer({ error: 'Could not save profile. Try again.' }));
};
