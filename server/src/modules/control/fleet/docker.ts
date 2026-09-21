/**
 * Docker fleet runtime: one container per session, on a Docker daemon this
 * process can reach. The operator supplies an internal-only network containing
 * the control/egress service, nothing here punches a hole in it.
 */

import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { prepareSession, hash, fault } from './common.ts';
import { Status } from '../../../platform/http-status.ts';
import { CAPABILITIES, CLI_TIMEOUT_MS, MAX_DOCKER_OUTPUT } from './constants.ts';

/** execFile, awaitable. */
const exec = promisify(execFile);
/** Runs the docker CLI and returns its stdout. */
const docker = async (args) =>
  (await exec('docker', args, { timeout: CLI_TIMEOUT_MS, maxBuffer: MAX_DOCKER_OUTPUT })).stdout;

/** Hardening flags for every managed container: no capabilities, no privilege gain, bounded pids, memory and /dev/shm. */
const HARDENING = [
  '--cap-drop',
  'ALL',
  '--security-opt',
  'no-new-privileges',
  '--pids-limit',
  '256',
  '--memory',
  '2g',
  '--shm-size',
  '512m',
];

/** Why governance is refused before the runtime is set up. */
const UNCONFIGURED = 'Configure the managed Docker runtime before requesting governance';

/** Marks a container that no longer exists. */
const GONE = Symbol('gone');

/** The runtime's name, as OYA_FLEET_RUNTIME selects it. */
export const id = 'docker';

/** Whether the network, image, control URL and proxy URL are all set. */
export function configured() {
  return !!(
    process.env.OYA_MANAGED_NETWORK &&
    process.env.OYA_MANAGED_IMAGE &&
    process.env.OYA_MANAGED_CONTROL_URL &&
    process.env.OYA_MANAGED_PROXY_URL
  );
}

/**
 * Refuse unless the network is an internal bridge and the image carries the
 * governance label. Returns the runtime identity sessions are pinned to.
 */
export async function verifyRuntime() {
  if (!configured()) throw fault('runtime_unavailable', UNCONFIGURED, Status.UNPROCESSABLE);
  const network = await internalNetwork();
  const image = await governedImage();
  return runtimeIdentity(await daemonId(), network, image);
}

/** The managed network, refused unless it is an internal bridge. */
async function internalNetwork() {
  const network = JSON.parse(await docker(['network', 'inspect', process.env.OYA_MANAGED_NETWORK]))[0];
  if (!network?.Internal || network.Driver !== 'bridge')
    throw fault('unsafe_network', 'Managed browsers require an internal Docker bridge network', Status.UNPROCESSABLE);
  return network;
}

/** The managed image, refused unless it carries the governance label. */
async function governedImage() {
  const image = JSON.parse(await docker(['image', 'inspect', process.env.OYA_MANAGED_IMAGE]))[0];
  if (image?.Config?.Labels?.['ai.getoya.governance'] !== '1')
    throw fault('unsupported_image', 'Managed image must contain the governance browser hooks', Status.UNPROCESSABLE);
  return image;
}

/** The daemon's ID, so cleanup can insist on the same daemon. */
const daemonId = async () => (await docker(['info', '--format', '{{.ID}}'])).trim();

/** What a session records about the runtime it was started on. */
const runtimeIdentity = (daemon, network, image) => ({
  id: 'docker-local',
  daemonId: daemon,
  networkId: network.Id,
  imageId: image.Id,
  region: process.env.OYA_MANAGED_REGION || 'local',
  capabilities: [...CAPABILITIES],
  verifiedAt: Date.now(),
});

/** Create and start the session's hardened container on the managed network. */
export async function create({ apiKey, browserId, persona, name, policies = [] }) {
  const runtime = await verifyRuntime();
  const container = `oya-managed-${browserId}`;
  const session = { apiKey, browserId, persona, name, policies, runtime, fallbackName: container };
  // `runtime` is carried so cleanup still targets Docker after an operator
  // switches OYA_FLEET_RUNTIME; the kind stays 'docker' so sessions created by
  // earlier versions keep being collected.
  const cleanup = { kind: 'docker', runtime: 'docker', container, daemonId: runtime.daemonId };
  const { environment } = await prepareSession({ ...session, cleanup });
  await withEnvFile(environment, (file) => startContainer(container, runtime, apiKey, browserId, file));
  return { browserId, runtime };
}

/** Credentials go to docker create through an env file, never command-line arguments; the file is removed afterwards. */
async function withEnvFile(environment, use) {
  const dir = await mkdtemp(join(tmpdir(), 'oya-runtime-'));
  try {
    const file = join(dir, 'env');
    await writeFile(file, envFile(environment), { mode: 0o600 });
    await use(file);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}

/** KEY=value lines. */
const envFile = (environment) =>
  Object.entries(environment)
    .map(([k, v]) => `${k}=${v || ''}`)
    .join('\n');

/** Creates the hardened, labelled container and starts it. */
async function startContainer(container, runtime, apiKey, browserId, file) {
  const labels = ['--label', `oya.session=${browserId}`, '--label', `oya.owner=${hash(apiKey)}`];
  const args = ['--name', container, '--network', runtime.networkId, ...HARDENING, ...labels];
  await docker(['create', ...args, '--env-file', file, runtime.imageId]);
  await docker(['start', container]);
}

/** Remove the session's container, only on the daemon that made it and only if its labels match the owner and session. Already gone is fine. */
export async function remove(container, apiKey, browserId, daemon = null) {
  if (daemon && (await daemonId()) !== daemon)
    throw fault('runtime_unavailable', 'Cleanup requires the original Docker daemon', Status.UNAVAILABLE);
  const info = await inspect(container);
  if (info === GONE) return;
  if (info.Config?.Labels?.['oya.owner'] !== hash(apiKey) || info.Config?.Labels?.['oya.session'] !== browserId)
    throw fault('ownership_mismatch', 'Managed container ownership mismatch');
  await docker(['rm', '-f', container]);
}

/** The container's inspect record, or GONE when it no longer exists. */
async function inspect(container) {
  try {
    return JSON.parse(await docker(['inspect', container]))[0];
  } catch (e) {
    if (/No such (object|container)/.test(e.stderr || '')) return GONE;
    throw e;
  }
}
