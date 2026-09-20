/**
 * What every fleet runtime has to get right, in one place.
 *
 * A governed browser is handed a session-scoped credential, an enrollment token
 * and egress proxy credentials, and it must never see the project API key. That
 * reasoning is identical whether the container is started by Docker, Kubernetes
 * or ECS, so it lives here rather than being re-derived per runtime — a second
 * copy is a second place for the credential handling to go subtly wrong.
 */

import { randomBytes } from 'node:crypto';
import { control, hash, fault } from '../service.ts';
import { Status } from '../../../platform/http-status.ts';
import { HTTP_PORT, HTTPS_PORT, TOKEN_BYTES } from './constants.ts';

/** Why a proxy URL is refused. */
const INVALID_PROXY = 'Managed proxy URL must be HTTP(S) without embedded credentials';

/** The egress proxy the browser is forced through. Credentials are per session. */
export function egressProxy(browserId, token) {
  const proxy = new URL(process.env.OYA_MANAGED_PROXY_URL);
  if (!['http:', 'https:'].includes(proxy.protocol) || proxy.username || proxy.password)
    throw fault('invalid_proxy', INVALID_PROXY, Status.UNPROCESSABLE);
  return { ...proxyEndpoint(proxy), username: browserId, password: token };
}

/** Scheme, host and port of the proxy URL. */
function proxyEndpoint(proxy) {
  const secure = proxy.protocol === 'https:';
  return {
    type: secure ? 'https' : 'http',
    host: proxy.hostname,
    port: Number(proxy.port || (secure ? HTTPS_PORT : HTTP_PORT)),
  };
}

/**
 * Claim the session, mint its credential, and return the environment the
 * container gets. The cleanup descriptor is supplied by the runtime because only
 * it knows what has to be torn down, but it is persisted *before* anything is
 * created — a crash between create and start must still leave something
 * deletable.
 */
export async function prepareSession(options) {
  const session = withDefaults(options);
  requireRegion(session.policies, session.runtime);
  const { token, proxy } = mintEgress(session.browserId);
  await claim(session, token);
  // The container never sees the project key: its credential registers only this
  // session and ends with it.
  const credential = await control().enrollmentCredential(session.apiKey, session.browserId);
  const environment = containerEnvironment(session, credential, token, proxy);
  requireSingleLines(environment);
  return { token, environment };
}

/** The options with `policies` defaulting to none. */
const withDefaults = ({ policies = [], ...rest }: any) => ({ ...rest, policies });

/** A fresh session token and the egress proxy credentials built on it. */
function mintEgress(browserId) {
  const token = randomBytes(TOKEN_BYTES).toString('base64url');
  return { token, proxy: egressProxy(browserId, token) };
}

/** Refuses a policy that pins a region this runtime is not in. */
function requireRegion(policies, runtime) {
  for (const policy of policies) {
    if (policy.region && policy.region !== runtime.region) {
      throw fault('region_unavailable', 'Managed runtime does not match the required region', Status.UNPROCESSABLE);
    }
  }
}

/** Persists the cleanup descriptor, runtime and token hashes before anything is created. */
function claim({ apiKey, browserId, cleanup, runtime, policies }, token) {
  return control().update(apiKey, browserId, {
    cleanup,
    runtime,
    egressHash: hash(token),
    enrollmentHash: hash(token),
    policies,
    managed: true,
  });
}

/** The container's environment. */
const containerEnvironment = (session, credential, token, proxy) => ({
  OYA_SERVER_URL: process.env.OYA_MANAGED_CONTROL_URL,
  OYA_API_KEY: credential.token,
  ...browserIdentity(session),
  OYA_ENROLLMENT_TOKEN: token,
  OYA_GOVERNANCE: JSON.stringify({ policies: session.policies, proxy }),
});

/** Who the browser is and how it connects. */
const browserIdentity = ({ browserId, persona, name, fallbackName }) => ({
  OYA_BROWSER_ID: browserId,
  OYA_PERSONA: persona,
  OYA_BROWSER_NAME: name || fallbackName,
  OYA_PROVIDER: 'oya-selfhosted',
  OYA_AUTO_CONNECT: 'true',
});

/** A newline would let a value forge an extra entry in an env file. */
function requireSingleLines(environment) {
  if (Object.values(environment).some((v) => String(v).includes('\n'))) {
    throw fault('invalid_environment', 'Runtime environment must not contain newlines', Status.BAD_REQUEST);
  }
}

export { hash, fault };
