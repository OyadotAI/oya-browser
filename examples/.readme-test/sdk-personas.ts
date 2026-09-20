import { Oya } from '@oya-ai/browser';

const oya = new Oya();
const persona = await oya.personas.create({
  name: 'us-shopper',
  prefs: { platform: 'MacIntel', timezone: 'America/New_York', locale: 'en-US' }, // fixed for life
  proxy: { geo: 'US' },
});
await using browser = await oya.browser.start({ persona: persona.id }); // or persona: 'auto' to rotate
await browser.goto('https://example.com');
console.log(persona.fingerprint.platform, persona.fingerprint.timezone); // the same on every run
