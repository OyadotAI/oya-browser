// Two-factor logins: the TOTP seed is sealed on the persona, and completeMfa() generates the code, types it and submits.
import { Oya } from '@oya-ai/browser';

const oya = new Oya();
await using browser = await oya.desktop.connect(); // your own Oya Browser; pairs it on first run
const persona = (await oya.personas.list()).find((p) => [p.id, p.name].includes(browser.persona))!;
const url = new URL(process.env.MFA_URL!);
// Scoped to this site, so the seed never answers a code prompt anywhere else.
await oya.personas.setMfa(persona.id, { type: 'totp', secret: process.env.TOTP_SECRET!, domain: url.hostname });

await browser.goto(url.href);
const inputs = (await browser.analyze()).elements.filter((e) => e.type === 'input');
await browser.type(inputs.find((e) => /mail|user/i.test(`${e.name} ${e.placeholder}`))!.id, process.env.MFA_USER!);
await browser.type(inputs.find((e) => /pass/i.test(`${e.name} ${e.placeholder}`))!.id, process.env.MFA_PASSWORD!);
const mfa = await browser.completeMfa(); // method: 'totp' | 'gmail' | 'graph' | 'email' | 'sms' | 'handoff'
console.log(mfa.completed ? `signed in with ${mfa.method}` : `a person can finish it here: ${mfa.liveViewUrl}`);
