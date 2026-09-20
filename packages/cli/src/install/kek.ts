/**
 * Guarding the KEK (OYA_PROFILE_SECRET). Encrypted state left behind by a
 * previous install is keyed to a secret we may no longer have; generating a
 * fresh one on top of it does not fail at install time — the server
 * crash-loops later on "unable to authenticate data", which is an awful way to
 * find out. So look before writing.
 */
import { askSecret, choose, confirm, note, success, warn, InputError, type Option } from '../prompt.ts';
import { capture, run } from './shell.ts';

/**
 * Pinned by digest. priorState() pulls this image and runs it with a volume of
 * sealed credentials mounted, so a mutable `alpine:latest` would mean running
 * whatever that tag points at today against the operator's data. This is the
 * multi-arch index digest of alpine:3.22.
 */
const PROBE_IMAGE = 'alpine@sha256:14358309a308569c32bdc37e2e0e9694be33a9d99e68afb0f5ff33cc1f695dce';

/** What sealed state looks like in a data volume. */
const SEALED = /cookies\.json|personas\.json|profiles|\.secret/;

/** Whether a volume holds sealed state. */
async function isSealed(name: string): Promise<boolean> {
  const listing = await capture('docker', ['run', '--rm', '-v', `${name}:/d`, PROBE_IMAGE, 'ls', '/d']);
  // If the probe cannot run, treat a volume that exists as suspect rather than
  // assuming it is empty — the failure mode of guessing wrong is a crash loop.
  return listing === null || SEALED.test(listing);
}

/** The first oya-data volume holding sealed state, or null. */
export async function priorState(): Promise<string | null> {
  const volumes = (await capture('docker', ['volume', 'ls', '-q', '--filter', 'name=oya-data'])) || '';
  for (const name of volumes
    .split('\n')
    .map((v) => v.trim())
    .filter(Boolean)) {
    if (await isSealed(name)) return name;
  }
  return null;
}

/** What can be done about sealed state from an earlier install. */
const RESOLUTIONS: Option[] = [
  { id: 'paste', label: 'I have the previous OYA_PROFILE_SECRET', note: 'reuse it and keep the data' },
  { id: 'reset', label: 'Delete the old data and start clean', note: 'personas, cookies and sessions are lost' },
  { id: 'abort', label: 'Stop so I can go and find it' },
];

/** Explains the conflict and asks what to do about it. */
function askResolution(volume: string): Promise<string> {
  warn(`${volume} already holds encrypted data from an earlier install.`);
  note('Cookies, proxy credentials and TOTP seeds in it were sealed with that');
  note("install's OYA_PROFILE_SECRET. A new secret cannot open them, and the");
  note('server refuses to start rather than silently losing them.');
  return choose('How should that be handled?', RESOLUTIONS);
}

/**
 * cwd: root, not process.cwd() — findRepoRoot walks up, so running the
 * wizard from a subdirectory leaves `docker compose` with no compose file and
 * nothing stopped. `docker volume rm -f` does not force a volume that is
 * still attached, so that failure must not be swallowed either: the whole
 * point of this branch is that the next step mints a fresh KEK, and doing
 * that over surviving sealed data is the crash loop this function exists to
 * prevent.
 */
async function deleteVolume(volume: string, root: string): Promise<void> {
  if (!(await confirm(`Delete ${volume} and everything stored in it?`, false))) {
    throw new InputError('Stopped without changing anything.');
  }
  await run('docker', ['compose', 'down', '-v'], { cwd: root, quiet: true }).catch(() => {});
  await run('docker', ['volume', 'rm', '-f', volume], { cwd: root, quiet: true });
  success(`removed ${volume}`);
}

/** Asks for the previous secret; blank is not accepted. */
const askPrevious = () =>
  askSecret('Previous OYA_PROFILE_SECRET:', {
    validate: (v) => (v ? undefined : 'Paste the secret, or pick another option.'),
  });

/** Returns the secret to use, or null to keep generating a fresh one. */
export async function resolveKekConflict(volume: string, root: string): Promise<string | null> {
  const choice = await askResolution(volume);
  if (choice === 'paste') return askPrevious();
  if (choice === 'abort') throw new InputError('Stopped without changing anything.');
  await deleteVolume(volume, root);
  return null;
}
