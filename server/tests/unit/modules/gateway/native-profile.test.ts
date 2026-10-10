/** Native gateway profile hydration is cold, owner-scoped and fenced by durable reservations. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { ownDataDir } from '../../support/data-dir.ts';
ownDataDir('oya-native-gateway-profile-');
const { prepareNativeProfile, saveNativeProfile } = await import('../../../../src/modules/gateway/native-profile.ts');
const profiles = await import('../../../../src/modules/gateway/profiles.ts');
const logins = await import('../../../../src/modules/personas/cookies.ts');
const { control } = await import('../../../../src/modules/control/service.ts');
const { container } = await import('../../../../src/app/container.ts');
const { fingerprint } = await import('../../../../src/platform/audit.ts');

/** Reserve exactly as the public gateway does, using a fresh tenant unless a test resumes it. */
async function start(profileName = 'work', token = randomUUID()) {
  const reservation = await control().reserve(token, {
    provider: 'gateway',
    request: { profile: profileName },
    maxConcurrent: 10,
  });
  return { token, owner: fingerprint(token), profileName, reservation };
}

it('restores the saved cookie/localStorage snapshot with the same device across cold sessions', async () => {
  const first = await start();
  const persona = await prepareNativeProfile(first);
  const device = container.personas.fingerprintFor(persona);
  logins.mergeDump(persona.id, [{ name: 'sid', value: 'saved', domain: 'shop.test', path: '/' }]);
  logins.mergeStorage(persona.id, { 'https://shop.test': { who: 'owner' } });
  await saveNativeProfile(first, persona);
  await control().update(first.token, first.reservation.id, { state: 'stopped' });
  logins.clear(persona.id);
  const next = await start('work', first.token);
  const restored = await prepareNativeProfile(next);
  assert.equal(restored.id, persona.id);
  assert.deepEqual(container.personas.fingerprintFor(restored), device);
  assert.equal(logins.getAll(restored.id)[0].value, 'saved');
  assert.deepEqual(logins.getStorage(restored.id), { 'https://shop.test': { who: 'owner' } });
});

it('refuses legacy sessionStorage before changing the saved profile', async () => {
  const request = await start();
  const saved = { version: 1, cookies: [], storage: { origin: 'https://shop.test', session: { secret: 'keep' } } };
  await profiles.saveProfile(request.owner, request.profileName, saved);
  await assert.rejects(prepareNativeProfile(request), { status: 422 });
  assert.deepEqual(await profiles.loadProfile(request.owner, request.profileName), saved);
});

it('imports legacy cookies and localStorage before persisting the native identity binding', async () => {
  const request = await start();
  await profiles.saveProfile(request.owner, request.profileName, {
    version: 1,
    cookies: [],
    storage: { origin: 'https://shop.test', local: { who: 'legacy' }, session: {} },
  });
  const persona = await prepareNativeProfile(request);
  assert.equal(logins.getStorage(persona.id)['https://shop.test'].who, 'legacy');
  assert.equal((await profiles.loadProfile(request.owner, request.profileName)).nativePersona, persona.id);
});

it('never resolves another tenant’s native identity even from an owner-sealed profile reference', async () => {
  const first = await start(),
    other = await start();
  const persona = await prepareNativeProfile(first);
  await profiles.saveProfile(other.owner, other.profileName, { version: 2, nativePersona: persona.id });
  await assert.rejects(prepareNativeProfile(other), { status: 409 });
});

it('refuses a second durable reservation for the same identity before touching its login state', async () => {
  const first = await start();
  const persona = await prepareNativeProfile(first);
  logins.mergeStorage(persona.id, { 'https://shop.test': { who: 'live' } });
  const next = await start('alias', first.token);
  await profiles.saveProfile(next.owner, next.profileName, { version: 2, nativePersona: persona.id, origins: {} });
  await assert.rejects(prepareNativeProfile(next), { status: 409 });
  assert.equal(logins.getStorage(persona.id)['https://shop.test'].who, 'live');
});

it('keeps identically named profiles separate across owners', async () => {
  const first = await start(),
    other = await start();
  const a = await prepareNativeProfile(first),
    b = await prepareNativeProfile(other);
  assert.notEqual(a.id, b.id);
});
