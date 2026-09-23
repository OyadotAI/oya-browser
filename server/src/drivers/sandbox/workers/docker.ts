/**
 * Docker sandbox runtime: one container of the browser image per browser, on
 * the Docker daemon this server's docker CLI reaches. Operator-only: the image,
 * network and daemon come from the deployment's environment, never from a key.
 *
 * Docker has no idle stop; the image stops itself at OYA_MAX_LIFETIME_MINUTES
 * and --rm removes the container, and its credentials, when it does.
 */
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { HARDENING, withEnvFile } from '../../../modules/control/managed.ts';
import { unset } from '../names.ts';
import { MS_PER_SECOND, SANDBOX_CLI_MAX_OUTPUT, SANDBOX_CREATE_TIMEOUT_S } from '../../constants.ts';

/** execFile, awaitable. */
const exec = promisify(execFile);

/** Runs the docker CLI and returns its stdout. Long enough for a first pull of the image. */
export const docker = async (args: string[]) =>
  (
    await exec('docker', args, {
      timeout: SANDBOX_CREATE_TIMEOUT_S * MS_PER_SECOND,
      maxBuffer: SANDBOX_CLI_MAX_OUTPUT,
    })
  ).stdout;

/** The runtime's name, as OYA_CLOUD_RUNTIME selects it. */
export const id = 'docker';

/** Runs on the operator's daemon, so a key can never bring its own. */
export const ownAccount = () => false;

/**
 * The Docker settings from env, or null unless the image is set. The platform
 * is for an image built for another architecture than the host's (the browser
 * image is linux/amd64; an arm64 host runs it emulated).
 */
export function settings(env) {
  if (!env.OYA_CLOUD_IMAGE) return null;
  const optional = { network: env.OYA_CLOUD_DOCKER_NETWORK || null, platform: env.OYA_CLOUD_DOCKER_PLATFORM || null };
  return { image: env.OYA_CLOUD_IMAGE, ...optional };
}

/** What is unset. */
export const missing = (env) => unset([['OYA_CLOUD_IMAGE', env.OYA_CLOUD_IMAGE]]);

/** Starts the container; its credentials go in through an env file, never argv. */
export async function create(config, spec) {
  let containerId = '';
  await withEnvFile(spec.env, async (file) => {
    containerId = (await docker(runArgs(config, spec, file))).trim();
  });
  return { id: containerId };
}

/** `docker run` for this spec: hardened, labelled, removed when it exits. */
function runArgs(config, spec, file) {
  const labels = Object.entries(spec.labels).flatMap(([key, value]) => ['--label', `${key}=${value}`]);
  const network = config.network ? ['--network', config.network] : [];
  const platform = config.platform ? ['--platform', config.platform] : [];
  // So OYA_PUBLIC_WS_URL can name the host on Linux the way it already can on Docker Desktop.
  const host = ['--add-host', 'host.docker.internal:host-gateway'];
  const run = ['run', '-d', '--rm', '--name', spec.name];
  return [...run, ...HARDENING, ...labels, ...network, ...platform, ...host, '--env-file', file, config.image];
}

/** The container of this name, or null when the daemon has none. */
export async function find(_config, name) {
  const [info] = await inspect([name]);
  if (!info) return null;
  return { ...found(info), destroy: () => docker(['rm', '-f', name]).then(() => {}) };
}

/** Every browser container labelled for this owner, running or not. */
export async function list(_config, owner) {
  const filters = ['--filter', 'label=oya-browser=true', '--filter', `label=oya-owner=${owner}`];
  const ids = (await docker(['ps', '-aq', ...filters])).split('\n').filter(Boolean);
  return ids.length ? (await inspect(ids)).map(found) : [];
}

/** The inspect records of these containers; none for a name the daemon does not know. */
async function inspect(names) {
  try {
    return JSON.parse(await docker(['inspect', ...names]));
  } catch (err) {
    if (/No such (object|container)/.test(err.stderr || '')) return [];
    throw err;
  }
}

/** A container's labels and state. */
const found = (info) => ({ labels: info.Config?.Labels || {}, state: info.State?.Status || 'unknown' });
