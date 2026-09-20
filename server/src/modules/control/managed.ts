/**
 * The governed fleet, whichever runtime provides it.
 *
 * Docker, Kubernetes and (later) ECS all sit behind the provider id
 * `oya-selfhosted`: the runtime differs, the provider does not. That is what
 * keeps api.js's dispatch, PROVIDER_CHOICES, the SDK's Provider union and the
 * governance gate in service.js unchanged — they ask for a governed browser and
 * this module decides what starts one.
 *
 * Select with OYA_FLEET_RUNTIME=docker|k8s (default docker).
 */

import * as docker from './fleet/docker.ts';
import * as k8s from './fleet/k8s.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';

/** The runtimes OYA_FLEET_RUNTIME can name. */
const RUNTIMES = { docker, k8s };

/** The named runtime, else OYA_FLEET_RUNTIME, else Docker; throws runtime_unavailable for an unknown name. */
function runtimeFor(name?) {
  const chosen = RUNTIMES[name || process.env.OYA_FLEET_RUNTIME || 'docker'];
  if (!chosen) throw unknownRuntime();
  return chosen;
}

/** runtime_unavailable, naming the runtimes that do exist. */
function unknownRuntime() {
  const known = Object.keys(RUNTIMES).join(', ');
  const message = `Unknown OYA_FLEET_RUNTIME "${process.env.OYA_FLEET_RUNTIME}" (known: ${known})`;
  return new HttpError(Status.UNPROCESSABLE, message, { code: 'runtime_unavailable' });
}

/** Whether the selected runtime has the settings it needs; false rather than throwing for an unknown runtime. */
export function managedConfigured() {
  try {
    return runtimeFor().configured();
  } catch {
    return false;
  }
}

/** Refuses unless the selected runtime is safe to run governed browsers on; returns the identity sessions are pinned to. */
export function verifyRuntime() {
  return runtimeFor().verifyRuntime();
}

/** Starts a governed browser on the selected runtime. */
export function createManaged(options) {
  return runtimeFor().create(options);
}

/**
 * Cleanup follows the descriptor that was stored when the session was created,
 * not whatever OYA_FLEET_RUNTIME says now: a browser started on Docker must still
 * be torn down on Docker after the operator moves the fleet to Kubernetes, and
 * a pod started in one namespace must be deleted from that namespace, not from
 * whatever OYA_K8S_NAMESPACE points at today.
 * Descriptors written before runtimes existed carry no `runtime` and are Docker.
 */
export function removeManaged(container, apiKey, browserId, daemonId = null, runtime = 'docker', namespace = null) {
  return runtimeFor(runtime).remove(container, apiKey, browserId, daemonId, namespace);
}
