/**
 * The hosted receiver for Keel's daily usage report. Installs have no API key;
 * the address limit bounds anonymous writes and is separate from Oya pings.
 */
import { Router } from 'express';
import { HttpError, notFound } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { consume } from '../../platform/limits.ts';
import { clientAddress } from '../auth/service.ts';
import { hosted } from '../billing/config.ts';
import { parseKeelReport } from './keel-batch.ts';
import { recordKeelReport } from './keel-repository.ts';

/** Keel telemetry routes mounted under /api. */
export const router = Router({ caseSensitive: true });

/** POST /telemetry/keel/ping accepts only the anonymous fields keel-license sends. */
router.post('/telemetry/keel/ping', async (req, res) => {
  if (!hosted()) throw notFound('Route');
  if (!consume('installPing', `keel:${clientAddress(req)}`).allowed)
    throw new HttpError(Status.TOO_MANY_REQUESTS, 'Too many pings');
  await recordKeelReport(parseKeelReport(req.body));
  res.json({ ok: true });
});
