/**
 * Unit tests for main/identity.cjs: the user agent, the page-side override and
 * the client-hint headers of a persona say the same thing, in Chrome's words.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { personaIdentity, brandsFor } = require('../../../main/identity.cjs');

const ELECTRON_UA = 'Mozilla/5.0 (Macintosh) Chrome/134.0.6998.44 Electron/35.1.2 Safari/537.36';
/** A brand list as Chrome prints it. */
const printed = (major) =>
  brandsFor(major)
    .map((b) => `${b.brand} ${b.version}`)
    .join(', ');

describe('brandsFor', () => {
  it("is Chrome's own list for each release: its GREASE brand, its version and its order", () => {
    assert.equal(printed('120'), 'Not_A Brand 8, Chromium 120, Google Chrome 120');
    assert.equal(printed('131'), 'Google Chrome 131, Chromium 131, Not_A Brand 24');
    assert.equal(printed('134'), 'Chromium 134, Not:A-Brand 24, Google Chrome 134');
  });
});

describe('personaIdentity', () => {
  it('reduces the version in the user agent and keeps the full one for the hints', () => {
    const { userAgent, hints } = personaIdentity({ navigator: { platform: 'Win32' } }, ELECTRON_UA);
    assert.equal(
      userAgent,
      'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36',
    );
    assert.equal(hints['sec-ch-ua-full-version'], '"134.0.6998.44"');
  });

  it('says in the headers exactly what the page override says', () => {
    const { override, hints } = personaIdentity({ navigator: { platform: 'Linux x86_64' } }, ELECTRON_UA);
    const meta = override.userAgentMetadata;
    assert.equal(override.platform, 'Linux x86_64');
    assert.equal(hints['sec-ch-ua'], '"Chromium";v="134", "Not:A-Brand";v="24", "Google Chrome";v="134"');
    assert.equal(hints['sec-ch-ua-platform'], `"${meta.platform}"`);
    assert.equal(hints['sec-ch-ua-arch'], `"${meta.architecture}"`);
    assert.match(hints['sec-ch-ua-full-version-list'], /"Google Chrome";v="134\.0\.6998\.44"/);
    assert.match(hints['sec-ch-ua-full-version-list'], /"Not:A-Brand";v="24\.0\.0\.0"/);
  });

  it("sends the persona's languages, which sets navigator.languages and Accept-Language together", () => {
    const persona = { navigator: { platform: 'Win32', languages: ['de-DE', 'de'] } };
    assert.equal(personaIdentity(persona, ELECTRON_UA).override.acceptLanguage, 'de-DE,de');
    assert.equal('acceptLanguage' in personaIdentity(null, ELECTRON_UA).override, false);
  });

  it("adds the base language Chrome sends by default, when a device's list has only the regional one", () => {
    const mirrored = { navigator: { platform: 'MacIntel', languages: ['en-US'] } };
    assert.equal(personaIdentity(mirrored, ELECTRON_UA).override.acceptLanguage, 'en-US,en');
    const bare = { navigator: { platform: 'MacIntel', languages: ['fr'] } };
    assert.equal(personaIdentity(bare, ELECTRON_UA).override.acceptLanguage, 'fr');
  });

  it('reads Apple Silicon from either GPU field: a mirrored device keeps the real name in the unmasked one', () => {
    const webgl = {
      renderer: 'WebKit WebGL',
      unmaskedRenderer: 'ANGLE (Apple, ANGLE Metal Renderer: Apple M4, Unspecified Version)',
    };
    const mirrored = { navigator: { platform: 'MacIntel' }, webgl };
    assert.equal(personaIdentity(mirrored, ELECTRON_UA).override.userAgentMetadata.architecture, 'arm');
  });

  it('never claims a Chrome other than the engine, whatever a mirrored profile says', () => {
    const mirrored = { navigator: { platform: 'MacIntel' }, chromeVersion: '149.0.7000.1' };
    assert.match(personaIdentity(mirrored, ELECTRON_UA).userAgent, /Chrome\/134\.0\.0\.0/);
  });

  it('reports Apple Silicon as arm while the platform stays MacIntel', () => {
    const m3 = { navigator: { platform: 'MacIntel' }, webgl: { renderer: 'ANGLE (Apple, Apple M3 Pro, OpenGL 4.1)' } };
    assert.equal(personaIdentity(m3, ELECTRON_UA).override.userAgentMetadata.architecture, 'arm');
    const intel = { navigator: { platform: 'MacIntel' }, webgl: { renderer: 'Intel Iris' } };
    assert.equal(personaIdentity(intel, ELECTRON_UA).override.userAgentMetadata.architecture, 'x86');
  });

  it("presents this machine's platform when there is no persona, and a known Chrome when the engine names none", () => {
    const { userAgent, override } = personaIdentity(null, 'Mozilla/5.0 Electron/35.1.2');
    const host = { darwin: 'MacIntel', win32: 'Win32' }[process.platform] || 'Linux x86_64';
    assert.equal(override.platform, host);
    assert.match(userAgent, /Chrome\/134\.0\.0\.0/);
    assert.doesNotMatch(userAgent, /Electron/);
  });
});
