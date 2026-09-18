import { Oya } from "@oya-ai/browser";

const oya = new Oya();
await using browser = await oya.browser.start();
await browser.goto("https://www.google.com/recaptcha/api2/demo");
console.log(await browser.solveCaptcha());   // { present, solved, method: 'provider' | 'solver' | 'none' }
console.log(await browser.completeMfa());    // TOTP, a code read from Gmail or Microsoft 365, a relay, or a human handoff
