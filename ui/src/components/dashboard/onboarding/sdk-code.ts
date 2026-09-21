/**
 * The six-line SDK example onboarding shows, filled in with this server and
 * the chosen profile.
 */
import { apiOrigin } from '@/lib/api';

/** The example for `key` (a placeholder on screen, the real key when copied) and `profileId`. */
export function sdkCode(key: string, profileId: string) {
  return `import { Oya } from "@oya-ai/browser";\nconst oya = new Oya({ apiKey: ${JSON.stringify(key)}, baseUrl: ${JSON.stringify(apiOrigin())} });\nconst browser = await oya.browser.start({ profile: ${JSON.stringify(profileId)}, captcha: "auto" });\nawait browser.goto("https://example.com");\nconst mfa = await browser.completeMfa();\nconsole.log(await browser.analyze());`;
}
