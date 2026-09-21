import { Oya } from "@oya-ai/browser";

const oya = new Oya();                                    // OYA_API_KEY
await using browser = await oya.browser.start({ persona: "auto", captcha: "auto" });
await browser.goto("https://example.com");
