/**
 * Where self-hosted servers send their daily ping, on the hosted deployment:
 * counted as an anonymous product event under the install's random id, and
 * answered with whether the install's license has been revoked. No address is
 * kept; the ping says only what is in it.
 */
import { Router } from 'express';
import { invalid, notFound } from '../../platform/errors.ts';
import { hosted } from '../billing/config.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { consume } from '../../platform/limits.ts';
import { clientAddress } from '../auth/service.ts';
import { recordPing, isRevoked, type Ping } from '../admin/index.ts';
import { track } from './service.ts';
import { PING_MAX_COUNT } from './constants.ts';

/** The ping routes, mounted on the API router. */
export const router = Router({ caseSensitive: true });

/** A random install id, as the licensing module makes them. */
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
/** A release version. */
const VERSION = /^[\w.-]{1,32}$/;
/** A license id. */
const LICENSE_ID = /^[\w-]{1,64}$/;

/** A count the ping reports, or a 400 naming the field. */
function count(body, field: string) {
  const n = body?.[field];
  if (!Number.isInteger(n) || n < 0 || n > PING_MAX_COUNT) throw invalid(field, 'a whole number', n);
  return n;
}

/** A license id, or null when the install runs without one; a 400 for anything else. */
function licenseOf(body) {
  const license = body?.license_id ?? null;
  if (license !== null && !LICENSE_ID.test(String(license))) throw invalid('license_id', 'a license id', license);
  return license as string | null;
}

/** The ping's fields, each checked, or a 400 for the first that is wrong. */
function parsePing(body) {
  if (!UUID.test(String(body?.install_id))) throw invalid('install_id', 'a UUID', body?.install_id);
  if (!VERSION.test(String(body?.version))) throw invalid('version', 'a version', body?.version);
  const counts = { browsers: count(body, 'browsers'), peak_cloud: count(body, 'peak_cloud') };
  return { id: String(body.install_id), version: String(body.version), ...counts, license: licenseOf(body) } as Ping;
}

/**
 * POST /telemetry/ping, a self-hosted server's daily ping; answers whether its
 * license is revoked. Limited per address, and counted as an event at most once
 * a day per install, so a flood of made-up installs cannot flood the analytics.
 */
router.post('/telemetry/ping', async (req, res) => {
  if (!hosted()) throw notFound('Route');
  if (!consume('installPing', clientAddress(req)).allowed)
    throw new HttpError(Status.TOO_MANY_REQUESTS, 'Too many pings');
  const ping = parsePing(req.body);
  if (await recordPing(ping)) track.installPinged(ping.id, eventOf(ping));
  res.json({ revoked: await isRevoked(ping.license) });
});

/** The event a ping is counted as. */
const eventOf = ({ version, browsers, peak_cloud, license }: Ping) => ({
  version,
  browsers,
  peak_cloud,
  licensed: Boolean(license),
});
