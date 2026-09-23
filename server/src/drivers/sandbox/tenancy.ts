/**
 * Whose runtime a key's cloud browsers run on.
 *
 * A key picks a runtime with the sandbox_runtime setting. Docker and Kubernetes
 * run on the operator's hardware through the server's own CLIs, so a key may
 * only choose them as the operator configured them: a tenant-supplied DOCKER_HOST,
 * kubeconfig (whose exec block runs a command here) or image is never read.
 * Daytona and ECS are remote APIs; a key that brings its own credential for
 * one runs on its own account, and then none of the operator's settings for
 * that runtime are mixed in, or the operator's AWS or Daytona account would
 * run whatever cluster or snapshot the key named.
 */
import * as keyConfig from '../../modules/config/service.ts';
import { WORKERS, DEFAULT_RUNTIME } from './worker.ts';
import { ecsExternalId } from './names.ts';

/** Settings that are the deployment's, whoever's runtime the browser runs on. */
const HOST_SETTINGS = ['OYA_PUBLIC_WS_URL', 'OYA_CLOUD_SANDBOX_TTL_MINUTES', 'DAYTONA_SANDBOX_TTL_MINUTES'];

/**
 * The environment a key's sandboxes are configured from: the key's own account
 * when it brings one, else the deployment's, on the runtime named by `runtime`,
 * else the key's choice, else the deployment's.
 */
export function sandboxEnv(apiKey?, runtime?) {
  const own = apiKey ? keyConfig.envFor(apiKey, {}) : {};
  const chosen = runtime || own.OYA_CLOUD_RUNTIME;
  // A key's own account counts on the deployment's runtime too, so setting `ecs` alone is enough there.
  const effective = chosen || process.env.OYA_CLOUD_RUNTIME || DEFAULT_RUNTIME;
  if (ownAccount(own, effective)) return { ...hostSettings(), ...own, ...fixed(apiKey), OYA_CLOUD_RUNTIME: effective };
  return chosen ? { ...process.env, OYA_CLOUD_RUNTIME: chosen } : process.env;
}

/** Whether the key's own settings are a complete account of its own on this runtime. */
const ownAccount = (own, runtime) => !!runtime && Object.hasOwn(WORKERS, runtime) && WORKERS[runtime].ownAccount(own);

/**
 * What a key's own account runs with but may not choose: the ExternalId a role
 * is assumed with is Oya's, one per key, so no key can have this server assume a
 * role another customer's trust policy grants to this server.
 */
const fixed = (apiKey) => ({ OYA_ECS_EXTERNAL_ID: ecsExternalId(apiKey) });

/** The deployment's settings that apply on any account. */
const hostSettings = () =>
  Object.fromEntries(HOST_SETTINGS.filter((name) => process.env[name]).map((name) => [name, process.env[name]]));
