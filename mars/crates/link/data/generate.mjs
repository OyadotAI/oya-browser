/**
 * Writes the persona data Mars's link crate is pinned to, from Oya's own TypeScript:
 *
 * - devices.json: the server's seeded device tables (server/src/modules/personas/devices.ts). Their
 *   contents and order are part of every persona's fingerprint, so Rust reads this copy instead of
 *   restating them, and a table edit on the Oya side shows up here as a diff.
 * - ../tests/vectors/persona.json: profiles from the server's generateProfile for fixed seeds and
 *   prefs, and identities from the browser's nativePolicyForPersona and personaIdentity. The Rust
 *   port must reproduce every one exactly: same persona, same device, forever.
 *
 * Run from anywhere: node mars/crates/link/data/generate.mjs
 */
import { writeFileSync, mkdirSync } from 'node:fs';

const root = new URL('../../../../', import.meta.url);
const devices = await import(new URL('server/src/modules/personas/devices.ts', root));
const { generateProfile } = await import(new URL('server/src/modules/personas/profile.ts', root));
const { seedFromString } = await import(new URL('server/src/modules/personas/prng.ts', root));
const { nativePolicyForPersona } = await import(new URL('browser/src/main/native-policy/persona.ts', root));
const { personaIdentity } = await import(new URL('browser/src/main/identity/identity.ts', root));

/** The f64 bits of a noise seed, so the comparison never depends on a decimal parser. */
function bits(x) {
  const view = new DataView(new ArrayBuffer(8));
  view.setFloat64(0, x);
  return view.getBigUint64(0).toString(16).padStart(16, '0');
}

/** Seed strings: API-key style, persona-id style, empty, non-ASCII and long. */
const SEED_STRINGS = ['', 'a', 'oya_test_key', 'p-0123456789abcdef', 'apikey-abcdef012345', 'ünïcødé ✓', 'x'.repeat(500)];
/** Prefs: none, each platform, checked and unchecked place choices, refused values and `auto`. */
const PREFS = [
  undefined,
  { platform: 'MacIntel' },
  { platform: 'Win32' },
  { platform: 'Linux x86_64' },
  { platform: 'Android' },
  { platform: 'MacIntel', timezone: 'Europe/Berlin', locale: 'de-DE' },
  { platform: 'MacIntel', timezone: 'Europe/Berlin', locale: 'de-DE', checked: true },
  { timezone: 'America/New_York', locale: 'en-US', checked: true },
  { timezone: 'Mars/Olympus', locale: 'xx-XX', checked: true },
  { timezone: 'auto', locale: 'auto' },
];

/** One profile vector per seed (string-derived and raw numbers) and prefs. */
function profiles() {
  const seeds = [...SEED_STRINGS.map((s) => ({ string: s, seed: seedFromString(s) })), ...[0, 1, 4294967295, 2147483648].map((seed) => ({ seed }))];
  return seeds.flatMap(({ string, seed }) =>
    PREFS.map((prefs, i) => {
      const identity = { id: `v-${seed}-${i}`, seed, ...(prefs ? { prefs } : {}), ...(i === 1 ? { proxy: { url: 'http://proxy.test:8080', username: 'u', password: 'p' } } : {}) };
      const profile = generateProfile(identity);
      const noise = { canvas: bits(profile.canvas.noiseSeed), audio: bits(profile.audio.noiseSeed), rects: bits(profile.rects.noiseSeed) };
      return { ...(string === undefined ? {} : { string }), identity, profile, noise };
    }),
  );
}

/** Engine versions and GPUs the identity is derived from. */
const ENGINES = ['Mozilla/5.0 Chrome/153.0.1234.56 Safari/537.36', 'Mozilla/5.0 Chrome/134.0.6998.205 Safari/537.36', 'Chrome/140.1.2.3'];
const GPUS = [undefined, { renderer: 'Apple M2' }, { renderer: 'ANGLE (Intel)', unmaskedRenderer: 'Apple M1 Pro' }, { renderer: 'Intel Iris' }];

/** Native policies and request hints for every platform, engine, GPU and language shape. */
function identities() {
  const cases = [];
  for (const platform of ['MacIntel', 'Win32', 'Linux x86_64'])
    for (const engine of ENGINES)
      for (const webgl of GPUS)
        for (const languages of [['en-US', 'en'], ['en-US'], ['fr-FR', 'fr'], ['de']]) {
          const locale = languages[0];
          const persona = { navigator: { platform, languages, hardwareConcurrency: 8 }, timezone: 'Europe/Paris', locale, ...(webgl ? { webgl } : {}) };
          const policy = nativePolicyForPersona(persona, engine);
          const { userAgent, override, hints } = personaIdentity(persona, engine);
          cases.push({ persona, engine, policy, userAgent, acceptLanguage: override.acceptLanguage ?? null, hints });
        }
  return cases;
}

const tables = { ...devices };
writeFileSync(new URL('./devices.json', import.meta.url), JSON.stringify(tables, null, 1) + '\n');
const vectors = new URL('../tests/vectors/', import.meta.url);
mkdirSync(vectors, { recursive: true });
const lines = (list) => list.map((v) => '  ' + JSON.stringify(v)).join(',\n');
const source = 'node mars/crates/link/data/generate.mjs';
const seeds = SEED_STRINGS.map((s) => ({ string: s, seed: seedFromString(s) }));
writeFileSync(
  new URL('persona.json', vectors),
  `{"source": ${JSON.stringify(source)},\n"seeds": ${JSON.stringify(seeds)},\n"profiles": [\n${lines(profiles())}\n],\n"identities": [\n${lines(identities())}\n]}\n`,
);
console.log('wrote devices.json and tests/vectors/persona.json');
