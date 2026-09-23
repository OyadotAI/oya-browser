/**
 * The Cloudflare Turnstile site key, read at request time on the Next server
 * like PostHog's settings in layout.tsx: one image serves hosted consoles
 * (captcha on) and self-host ones (no key, no captcha).
 */

/** The site key, or '' when the captcha is off. */
export const turnstileSiteKey = () => process.env.TURNSTILE_SITE_KEY?.trim() || '';
