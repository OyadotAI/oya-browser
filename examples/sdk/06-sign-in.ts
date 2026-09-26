// Sign in with a stored login: the password is sealed on the persona, and the agent's sign_in tool types it without ever seeing it.
import { Oya } from '@oya-ai/browser';

const oya = new Oya();
await using browser = await oya.desktop.connect(); // your own Oya Browser; pairs it on first run
const persona = (await oya.personas.list()).find((p) => [p.id, p.name].includes(browser.persona))!;
const url = new URL(process.env.LOGIN_URL!);
await oya.personas.setCredentials(persona.id, {
  domain: url.hostname,
  username: process.env.LOGIN_USER!,
  password: process.env.LOGIN_PASSWORD!,
});

await browser.goto(url.href);
console.log(await browser.ask('Sign in to this site, then tell me what the page says.'));
