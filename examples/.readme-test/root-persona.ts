import { Oya } from "@oya-ai/browser";

const oya = new Oya();
const p = await oya.personas.create({ name: "acme-ops" });
await oya.personas.setMfa(p.id, { type: "totp", secret: "JBSWY3DPEHPK3PXP" });
await using browser = await oya.browser.start({ persona: p.id });
