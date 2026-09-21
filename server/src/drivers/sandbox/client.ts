/**
 * The cloud runtime's SDK client, loaded lazily so the server runs without
 * @daytona/sdk installed when provisioning is not in use.
 */
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { settings, unconfigured } from './config.ts';

let sdkPromise;

/** The cloud SDK client, created once on first use. */
export async function client() {
  const config = settings();
  if (!config) throw unconfigured();
  // Lazy so the server runs without @daytona/sdk installed when provisioning
  // is not in use. Reset on failure so a later call can retry.
  sdkPromise ||= loadSdk(config);
  return sdkPromise;
}

/** Imports the SDK and builds the client; a failed import is forgotten so it can be retried. */
function loadSdk(config) {
  return import('@daytona/sdk')
    .then(({ Daytona }) => new Daytona(clientOptions(config)))
    .catch((err) => {
      sdkPromise = undefined;
      throw unavailable(err);
    });
}

/** The 409 for a runtime SDK that will not load. */
function unavailable(err) {
  const message = `Oya Cloud runtime unavailable: ${err.message}. Run \`npm i @daytona/sdk\` in server/.`;
  return new HttpError(Status.CONFLICT, message);
}

/** The SDK client's options for these settings. */
function clientOptions(config) {
  return { apiKey: config.apiKey, ...(config.apiUrl ? { apiUrl: config.apiUrl } : {}), target: config.target };
}

/** Whether an SDK error means the sandbox does not exist. */
export function isNotFound(err) {
  return [err?.status, err?.statusCode, err?.response?.status].includes(Status.NOT_FOUND);
}
