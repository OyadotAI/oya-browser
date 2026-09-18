// CAPTCHAs: the vendor's own solver when it has one, otherwise your CapSolver or 2Captcha key.
import { Oya } from '@oya-ai/browser';

const oya = new Oya();
await using browser = await oya.browser.start(); // or start({ captcha: 'auto' }) to clear them on every goto()
await browser.goto('https://www.google.com/recaptcha/api2/demo');
console.log(await browser.solveCaptcha()); // { present: true, solved: true, method: 'provider' | 'solver' }
