// Sign in from a prompt: pass the login as secrets, and the agent types {{password}} and {{seed|totp}} without ever reading them.
import { Oya } from '@oya-ai/browser';

const oya = new Oya();
await using browser = await oya.desktop.connect(); // your own Oya Browser; pairs it on first run
await browser.goto(process.env.MFA_URL!);
const answer = await browser.ask(
  'Sign in as {{user}} with {{password}}. If it asks for a one-time code, type {{seed|totp}}. Then say what the page shows.',
  { secrets: { user: process.env.MFA_USER!, password: process.env.MFA_PASSWORD!, seed: process.env.TOTP_SECRET! } },
);
console.log(answer);
