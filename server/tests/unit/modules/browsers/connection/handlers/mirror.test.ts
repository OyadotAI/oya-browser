/**
 * Unit tests for the mirror_persona handler: it turns a desktop's real browser
 * profiles into personas that carry the real device and its cookies, is
 * idempotent per profile, and refuses a malformed payload without a persona.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { HANDLERS, type Connection } from '../../../../../../src/modules/browsers/connection/handlers/index.ts';
import { container } from '../../../../../../src/app/container.ts';
import { getAll } from '../../../../../../src/modules/personas/cookies.ts';
import { FakeSocket } from '../../../../support/fakes.ts';

beforeEach(() => mock.method(console, 'log', () => {}));
afterEach(() => mock.restoreAll());

/** A device that passes validation: a real platform and a screen. */
const device = () => ({
  navigator: { platform: 'MacIntel' },
  screen: { width: 1440, height: 900 },
  chromeVersion: '128.0.0.0',
});

/** One profile as the desktop sends it, with a signed-in cookie. */
const profile = (over = {}) => ({
  source: 'chrome',
  profile: 'Default',
  device: device(),
  cookies: [{ name: 'sid', value: 'abc', domain: 'example.com', path: '/' }],
  ...over,
});

/** A connection whose socket records what the handler sends. */
function fakeConnection(): { conn: Connection; ws: FakeSocket } {
  const ws = new FakeSocket();
  const conn = {
    browserId: 'b-mirror',
    apiKey: `key-${Math.random()}`,
    persona: { id: 'p-mirror' },
    residentialProxy: false,
    localCommands: new Map(),
    changingControl: false,
    heard: mock.fn(),
    send: (m: object) => ws.send(JSON.stringify(m)),
    isOpen: () => true,
    isCurrent: () => true,
  } as Connection;
  return { conn, ws };
}

describe('mirror_persona handler', () => {
  it('creates a persona that runs as the real device and holds the profile cookies', () => {
    const { conn, ws } = fakeConnection();
    HANDLERS.mirror_persona(conn, { type: 'mirror_persona', profiles: [profile({ lastUsed: true })] });
    const [ok] = ws.ofType('mirror_ok');
    const persona = container.personas.get(conn.apiKey, ok.defaultPersonaId);
    assert.equal(persona!.device.chromeVersion, '128.0.0.0');
    assert.deepEqual(
      getAll(ok.defaultPersonaId).map((c) => c.name),
      ['sid'],
    );
  });

  it('returns the last-used profile as the default and one id per profile', () => {
    const { conn, ws } = fakeConnection();
    const profiles = [profile({ profile: 'Default' }), profile({ profile: 'Profile 1', lastUsed: true })];
    HANDLERS.mirror_persona(conn, { type: 'mirror_persona', profiles });
    const [ok] = ws.ofType('mirror_ok');
    assert.equal(ok.personaIds.length, 2);
    assert.equal(ok.defaultPersonaId, ok.personaIds[1]);
  });

  it('is idempotent: re-importing the same profile reuses the persona', () => {
    const { conn, ws } = fakeConnection();
    HANDLERS.mirror_persona(conn, { type: 'mirror_persona', profiles: [profile()] });
    HANDLERS.mirror_persona(conn, { type: 'mirror_persona', profiles: [profile()] });
    const [first, second] = ws.ofType('mirror_ok');
    assert.equal(first.defaultPersonaId, second.defaultPersonaId);
  });

  it('refuses an empty payload with mirror_failed and no persona', () => {
    const { conn, ws } = fakeConnection();
    HANDLERS.mirror_persona(conn, { type: 'mirror_persona', profiles: [] });
    assert.equal(ws.ofType('mirror_ok').length, 0);
    assert.equal(ws.ofType('mirror_failed').length, 1);
  });

  it('refuses a profile with no device', () => {
    const { conn, ws } = fakeConnection();
    HANDLERS.mirror_persona(conn, { type: 'mirror_persona', profiles: [profile({ device: undefined })] });
    assert.equal(ws.ofType('mirror_failed').length, 1);
  });
});
