import { Oya } from '@oya-ai/browser';

const oya = new Oya();
const persona = await oya.personas.create({ name: 'billing-admin' });
await oya.personas.setMfa(persona.id, { type: 'totp', secret: process.env.TOTP_SECRET! }); // sealed, never read back

await using browser = await oya.browser.start({ persona: persona.id });
await browser.goto(process.env.MFA_URL!); // after your login step: the page asking for the code
const mfa = await browser.completeMfa();   // method: 'totp' | 'gmail' | 'graph' | 'email' | 'sms' | 'handoff'
if (!mfa.completed) console.log('A person can finish it here:', mfa.liveViewUrl);
