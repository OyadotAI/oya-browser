// A persona is one device: fingerprint, cookie jar and exit IP, the same on every run.
import { Oya } from '@oya-ai/browser';

const oya = new Oya();
const persona = (await oya.personas.list()).find((p) => p.name === 'us-shopper')
  ?? await oya.personas.create({
    name: 'us-shopper',
    prefs: { platform: 'MacIntel', timezone: 'America/New_York', locale: 'en-US' }, // fixed for life
    proxy: { geo: 'US' },
  });

await using browser = await oya.browser.start({ persona: persona.id }); // or persona: 'auto' to rotate
await browser.goto('https://www.amazon.com');
console.log(persona.name, persona.fingerprint); // identical on the next run

// Another device of the same kind: oya.personas.clone(persona.id) gives a fresh identity with an empty jar.
