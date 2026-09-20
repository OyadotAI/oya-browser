/**
 * Queued starts: each saved start request is replayed against the API handler
 * once capacity allows, up to a batch per tick.
 */
import { Status } from '../../../platform/http-status.ts';
import { flags, workerHealth } from './state.ts';
import { DEFAULT_PUBLIC_WS_URL, PROVISION_BATCH } from './constants.ts';

/** Stands in for the Express response when a queued start is replayed, keeping what the handler answered. */
class ReplayResponse {
  /** The status the handler set; 200 unless it set one. */
  recordedStatus: number = Status.OK;
  /** The body the handler sent. */
  recordedBody: unknown;

  /** Records the status. */
  status(code) {
    this.recordedStatus = code;
    return this;
  }

  /** Records the body. */
  json(body) {
    this.recordedBody = body;
    return this;
  }
}

/** Starts the queued-start pass in the background unless one is already running. */
export function startProvisioning(service) {
  if (flags.provisioning) return;
  flags.provisioning = provisionQueued(service)
    .catch((err) => {
      workerHealth.lastError = err.message;
    })
    .finally(() => {
      flags.provisioning = null;
    });
}

/** Starts queued browsers, up to eight per tick, by replaying each saved start request against the API handler. */
async function provisionQueued(service) {
  const host = new URL(process.env.OYA_PUBLIC_WS_URL || DEFAULT_PUBLIC_WS_URL).host;
  // ponytail: sequential, up to eight per tick; run claims in parallel if queue drain rate matters.
  for (let i = 0; i < PROVISION_BATCH; i++) {
    const job = await service.claimQueued();
    if (!job) return;
    await provisionOne(service, job, host);
  }
}

/** Replays one start; an outcome it cannot record is marked unknown. */
async function provisionOne(service, job, host) {
  const { startBrowser } = await import('../../../app/api.ts');
  const res = new ReplayResponse();
  try {
    await startBrowser(replayRequest(job, host), res);
    await service.complete(job.key, job.session.id, res.recordedStatus, res.recordedBody);
  } catch {
    await markUnknown(service, job);
  }
}

/** The start's outcome could not be recorded; a failed write is left for the lease to lapse. */
async function markUnknown(service, job) {
  await service
    .update(job.key, job.session.id, { state: 'unknown_outcome' }, { fence: job.session.fence })
    .catch(() => {});
}

/** The request the job's caller would have made, bound to its reserved session. */
function replayRequest(job, host) {
  return {
    headers: { authorization: `Bearer ${job.key}`, host },
    body: job.request,
    controlSession: job.session,
    socket: {},
    secure: process.env.OYA_PUBLIC_WS_URL?.startsWith('wss:'),
  };
}
