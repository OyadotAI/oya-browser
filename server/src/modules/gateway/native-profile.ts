/** Restore gateway profiles into an unexposed native persona before its worker is created. */
import { container } from '../../app/container.ts';
import { control, live, projectId } from '../control/service.ts';
import * as logins from '../personas/cookies.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { NATIVE_PROFILE_VERSION } from './constants.ts';
import { loadProfile, saveProfile } from './profiles.ts';

/** Resolve the saved identity, refusing unsupported legacy state before any browser exists. */
export async function prepareNativeProfile(start) {
  const saved = start.profileName ? await loadProfile(start.owner, start.profileName) : null;
  assertRestorable(saved);
  const persona = resolvePersona(start, saved);
  await claimPersona(start, persona);
  await hydrate(persona.id, saved);
  await container.personas.drain();
  if (start.profileName) await saveNativeProfile(start, persona);
  return persona;
}

/** Refuse legacy sessionStorage rather than silently dropping a saved credential. */
function assertRestorable(saved) {
  if (
    saved &&
    (![1, NATIVE_PROFILE_VERSION].includes(saved.version) ||
      (saved.version === NATIVE_PROFILE_VERSION && typeof saved.nativePersona !== 'string'))
  )
    throw profileFault(Status.UNPROCESSABLE, 'Unsupported native profile snapshot');
  if (Object.keys(saved?.storage?.session || {}).length)
    throw profileFault(Status.UNPROCESSABLE, 'Native profile restore does not support saved sessionStorage');
}

/** Saved ids are owner-checked and never replaced by a default persona. */
function resolvePersona(start, saved) {
  if (!saved?.nativePersona)
    return container.personas.create(start.token, { name: start.profileName || 'Gateway', maxConcurrent: 1 });
  const persona = container.personas.get(start.token, saved.nativePersona);
  if (!persona) throw profileFault(Status.CONFLICT, 'Saved native profile identity is unavailable');
  if (container.personas.activeCount(persona.id)) throw profileFault(Status.CONFLICT, 'Native profile is in use');
  return persona;
}

/** Reserve the actual persona atomically, including other replicas and non-gateway users of that identity. */
async function claimPersona(start, persona) {
  await control().store.transact(async (tx) => {
    const project = projectId(start.token);
    await tx.lock('project', project);
    const session = await reserved(tx, start.reservation.id, project);
    await assertVacant(tx, project, session.id, persona.id, start.profileName);
    session.persona = persona.id;
  });
}

/** Only the caller's live provisioning reservation can claim a native identity. */
async function reserved(tx, id, project) {
  const session = await tx.get('session', id);
  if (!session || session.project !== project || session.state !== 'provisioning')
    throw Error('Gateway reservation is unavailable');
  return session;
}

/** A native reservation keeps the named-profile lock even after browser enrollment changes its persona field. */
async function assertVacant(tx, project, id, persona, profile) {
  const sessions = await tx.list('session', { project, states: live });
  const conflict = sessions.some(
    (entry) => entry.id !== id && (entry.persona === persona || (profile && entry.request?.profile === profile)),
  );
  if (conflict) throw profileFault(Status.CONFLICT, 'Native profile is in use');
}

/** Replace the scratch persona's login snapshot; no page or renderer has been exposed yet. */
async function hydrate(id, saved) {
  logins.clear(id);
  logins.mergeDump(id, saved?.cookies || []);
  const origins =
    saved?.origins || (saved?.storage?.origin ? { [saved.storage.origin]: saved.storage.local || {} } : {});
  logins.mergeStorage(id, origins);
  await logins.drain();
}

/** Save a native snapshot under the same owner/name encryption boundary as external profiles. */
export async function saveNativeProfile(start, persona) {
  await logins.drain();
  await saveProfile(start.owner, start.profileName, {
    version: NATIVE_PROFILE_VERSION,
    nativePersona: persona.id,
    savedAt: new Date().toISOString(),
    cookies: logins.getAll(persona.id),
    origins: logins.getStorage(persona.id),
  });
}

/** A caller/profile problem must not cool down the provider or fail over to an unrelated browser. */
function profileFault(status, message) {
  return new HttpError(status, message, { code: 'native_profile_unavailable' });
}
