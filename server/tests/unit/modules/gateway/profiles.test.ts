/**
 * Unit tests for persistent browser profiles: one session per profile at a
 * time, cookies and storage captured from and replayed into a loopback
 * browser, sealed per owner and name, and listed only to their owner.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync, copyFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { ownDataDir } from '../../support/data-dir.ts';

const dir = ownDataDir('oya-profiles-');
const profiles = await import('../../../../src/modules/gateway/profiles.ts');
const { fingerprint } = await import('../../../../src/platform/audit.ts');
const { fakeCdp, pageBrowser } = await import('../../support/gateway.ts');
const { endpointAt } = await import('../../../../src/drivers/cdp.ts');

const OWNER = fingerprint('profiles-key');
const OTHER = fingerprint('profiles-other');
const COOKIE = { name: 'sid', value: 'abc', domain: '.shop.test', path: '/' };
const STORAGE = { origin: 'https://shop.test', local: { cart: '3' }, session: {} };

/** A loopback browser holding one page with a cookie and some storage. */
const loggedIn = () =>
  fakeCdp(
    pageBrowser((method) => {
      if (method === 'Network.getAllCookies') return { cookies: [COOKIE] };
      if (method === 'Runtime.evaluate') return { result: { value: STORAGE } };
      return {};
    }),
  );
/** The profile file of `owner`'s `name`. */
const fileOf = (owner: string, name: string) => join(dir, 'profiles', `${owner}__${name}.enc`);

const opened: { close(): Promise<void> }[] = [];
afterEach(async () => {
  await Promise.all(opened.splice(0).map((b) => b.close()));
  for (const [o, n] of [
    [OWNER, 'shop'],
    [OWNER, 'other'],
    [OTHER, 'shop'],
  ])
    profiles.unlock(o, n);
});

describe('profile locks', () => {
  it('lets one session hold a profile and refuses a second', () => {
    assert.deepEqual(profiles.tryLock(OWNER, 'shop'), { ok: true });
    const second = profiles.tryLock(OWNER, 'shop');
    assert.equal(second.ok, false);
    assert.ok(second.since > 0);
    assert.equal(profiles.isLocked(OWNER, 'shop'), true);
    profiles.unlock(OWNER, 'shop');
    assert.equal(profiles.isLocked(OWNER, 'shop'), false);
  });

  it("keeps one owner's lock apart from another's profile of the same name", () => {
    profiles.tryLock(OWNER, 'shop');
    assert.deepEqual(profiles.tryLock(OTHER, 'shop'), { ok: true });
  });

  it('refuses an unsafe name or a missing owner with a 400', () => {
    for (const name of ['', '../etc', '.hidden', 'a'.repeat(65), 7]) {
      assert.throws(() => profiles.tryLock(OWNER, name), { status: 400, message: /Profile names are 1-64 chars/ });
    }
    assert.throws(() => profiles.tryLock('not-hex', 'shop'), {
      status: 400,
      message: 'A profile owner fingerprint is required',
    });
  });
});

describe('removeOwners', () => {
  it("deletes every profile an erased owner saved, and nobody else's", async () => {
    const { mkdirSync, writeFileSync, existsSync } = await import('node:fs');
    mkdirSync(join(dir, 'profiles'), { recursive: true });
    for (const [o, n] of [
      [OWNER, 'shop'],
      [OWNER, 'other'],
      [OTHER, 'shop'],
    ])
      writeFileSync(fileOf(o, n), 'sealed');
    assert.equal(await profiles.removeOwners(new Set([OWNER])), 2);
    assert.deepEqual(
      [fileOf(OWNER, 'shop'), fileOf(OWNER, 'other'), fileOf(OTHER, 'shop')].map((f) => existsSync(f)),
      [false, false, true],
    );
  });
});

describe('capture and restore', () => {
  beforeEach(async () => {
    await profiles.remove(OWNER, 'shop');
  });

  it("saves the browser's cookies and storage, sealed, owner-only", async () => {
    const browser = await loggedIn();
    opened.push(browser);
    assert.equal(await profiles.capture(OWNER, 'shop', { endpoint: endpointAt(browser.url) }), true);
    const raw = readFileSync(fileOf(OWNER, 'shop'));
    assert.doesNotMatch(raw.toString('latin1'), /abc|shop\.test/);
    assert.equal(statSync(fileOf(OWNER, 'shop')).mode & 0o777, 0o600);
  });

  it('enables no domain on the page it reads, which a page could notice', async () => {
    const browser = await loggedIn();
    opened.push(browser);
    await profiles.capture(OWNER, 'shop', { endpoint: endpointAt(browser.url) });
    const methods = browser.commands.map((c) => c.method);
    assert.deepEqual(
      methods.filter((m) => m.endsWith('.enable')),
      [],
    );
  });

  it('refuses capture from a browser without a page', async () => {
    const browser = await fakeCdp(() => ({ targetInfos: [] }));
    opened.push(browser);
    await assert.rejects(profiles.capture(OWNER, 'shop', { endpoint: endpointAt(browser.url) }), /requires a page/);
  });

  it('preserves saved state when reading storage fails', async () => {
    const source = await loggedIn();
    opened.push(source);
    await profiles.capture(OWNER, 'shop', { endpoint: endpointAt(source.url) });
    const saved = readFileSync(fileOf(OWNER, 'shop'));
    const browser = await fakeCdp(
      pageBrowser((method) => {
        if (method === 'Runtime.evaluate') throw new Error('no page');
        return method === 'Network.getAllCookies' ? { cookies: [COOKIE] } : {};
      }),
    );
    opened.push(browser);
    await assert.rejects(profiles.capture(OWNER, 'shop', { endpoint: endpointAt(browser.url) }), /no page/);
    assert.deepEqual(readFileSync(fileOf(OWNER, 'shop')), saved);
  });

  for (const failedMethod of ['Network.setCookies', 'Page.addScriptToEvaluateOnNewDocument']) {
    it(`refuses restore and closes its connection when ${failedMethod} fails`, async () => {
      const source = await loggedIn();
      opened.push(source);
      await profiles.capture(OWNER, 'shop', { endpoint: endpointAt(source.url) });
      const browser = await fakeCdp(
        pageBrowser((method) => {
          if (method === failedMethod) throw new Error('unsupported capability');
          return {};
        }),
      );
      opened.push(browser);
      const session: any = { endpoint: endpointAt(browser.url) };
      await assert.rejects(profiles.restore(OWNER, 'shop', session), /unsupported capability/);
      assert.equal(session.profileConn, undefined);
      for (let attempt = 0; attempt < 100 && browser.clients(); attempt++)
        await new Promise((resolve) => setTimeout(resolve, 5));
      assert.equal(browser.clients(), 0);
    });
  }

  it('refuses a stored profile when the new browser has no page', async () => {
    const source = await loggedIn();
    opened.push(source);
    await profiles.capture(OWNER, 'shop', { endpoint: endpointAt(source.url) });
    const browser = await fakeCdp(() => ({ targetInfos: [] }));
    opened.push(browser);
    await assert.rejects(profiles.restore(OWNER, 'shop', { endpoint: endpointAt(browser.url) }), /requires a page/);
  });

  it('replays the cookies, and registers the storage for its origin, into the next browser', async () => {
    const source = await loggedIn();
    opened.push(source);
    await profiles.capture(OWNER, 'shop', { endpoint: endpointAt(source.url) });
    const next = await fakeCdp(pageBrowser());
    opened.push(next);
    const session: any = { endpoint: endpointAt(next.url) };
    assert.equal(await profiles.restore(OWNER, 'shop', session), true);
    const set = next.commands.find((c) => c.method === 'Network.setCookies');
    assert.deepEqual(set.params.cookies, [COOKIE]);
    const script = next.commands.find((c) => c.method === 'Page.addScriptToEvaluateOnNewDocument');
    assert.match(script.params.source, /^if \(location\.origin === "https:\/\/shop\.test"\)/);
    assert.ok(session.profileConn, 'kept open so the replay script stays registered');
    session.profileConn.close();
  });

  it('restores nothing on first use of a profile', async () => {
    const next = await fakeCdp(pageBrowser());
    opened.push(next);
    assert.equal(await profiles.restore(OWNER, 'never-saved', { endpoint: endpointAt(next.url) }), false);
    assert.equal(next.commands.length, 0);
  });

  it("will not open a profile file moved onto another owner's name", async () => {
    const source = await loggedIn();
    opened.push(source);
    await profiles.capture(OWNER, 'shop', { endpoint: endpointAt(source.url) });
    copyFileSync(fileOf(OWNER, 'shop'), fileOf(OTHER, 'shop'));
    const next = await fakeCdp(pageBrowser());
    opened.push(next);
    await assert.rejects(profiles.restore(OTHER, 'shop', { endpoint: endpointAt(next.url) }));
    await profiles.remove(OTHER, 'shop');
  });

  it('seals under the owner-scoped name, so no other scope opens it', () => {
    const sealed = profiles._internals.seal(`${OWNER}__shop`, { cookies: [] });
    assert.deepEqual(profiles._internals.open(`${OWNER}__shop`, sealed), { cookies: [] });
    assert.throws(() => profiles._internals.open(`${OTHER}__shop`, sealed));
  });
});

describe('listing and removal', () => {
  it("lists only the owner's profiles, with whether a session holds them", async () => {
    const browser = await loggedIn();
    opened.push(browser);
    await profiles.capture(OWNER, 'shop', { endpoint: endpointAt(browser.url) });
    await profiles.capture(OWNER, 'other', { endpoint: endpointAt(browser.url) });
    await profiles.capture(OTHER, 'shop', { endpoint: endpointAt(browser.url) });
    profiles.tryLock(OWNER, 'shop');
    const listed = (await profiles.list(OWNER)).sort((a, b) => a.name.localeCompare(b.name));
    assert.deepEqual(
      listed.map((p) => [p.name, p.locked]),
      [
        ['other', false],
        ['shop', true],
      ],
    );
    assert.ok(listed[1].lockedSince > 0);
    assert.equal(readdirSync(join(dir, 'profiles')).length, 3);
  });

  it('refuses to delete a profile in use with a 409', async () => {
    profiles.tryLock(OWNER, 'shop');
    await assert.rejects(profiles.remove(OWNER, 'shop'), { status: 409, message: 'Profile is in use' });
  });

  it('deletes a saved profile, and says false when there was none', async () => {
    const browser = await loggedIn();
    opened.push(browser);
    await profiles.capture(OWNER, 'other', { endpoint: endpointAt(browser.url) });
    assert.equal(await profiles.remove(OWNER, 'other'), true);
    assert.equal(await profiles.remove(OWNER, 'other'), false);
  });
});
