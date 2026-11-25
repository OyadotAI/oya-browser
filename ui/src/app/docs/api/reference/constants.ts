/**
 * The API reference page's fixed values: which Scalar build renders the spec, the
 * hash that pins it, how long to wait for it, and the page's words.
 */

/**
 * Scalar's standalone build, at an exact version. The integrity hash below pins
 * these bytes: if the CDN served anything else the browser refuses to run it, so
 * a compromised CDN cannot put script on a page next to a console credential.
 * Bump both together.
 */
export const SCALAR_SRC = 'https://cdn.jsdelivr.net/npm/@scalar/api-reference@1.76.0/dist/browser/standalone.js';

/** The SHA-384 of SCALAR_SRC's bytes, as Subresource Integrity wants it. */
export const SCALAR_INTEGRITY = 'sha384-oH1C0RwqcwbVXkC+r8A+4qRoyEUQdtxAzOwumkxZtQBBcsy61FOoPt7B50OVJYYQ';

/** How long the reference gets to render before the page says it did not load. */
export const LOAD_TIMEOUT_MS = 10_000;

/** Where the spec is read from: the API's origin when it is on another one, else this one. */
export const specUrl = (apiUrl = process.env.NEXT_PUBLIC_API_URL) =>
  apiUrl?.startsWith('http') ? `${new URL(apiUrl).origin}/openapi.json` : '/openapi.json';

/** The page's words. */
export const REFERENCE_TEXT = {
  title: 'API reference',
  description:
    'Every public Oya Browser API route, rendered from the OpenAPI spec: browsers, runs, playbooks, personas, proxies and the project.',
  intro: 'Every public route, generated from /openapi.json. Send requests with an',
  header: 'Authorization: Bearer YOUR_API_KEY',
  download: 'header. Download the raw spec at',
  loading: 'Loading the API reference…',
  failed: "The interactive reference didn't load.",
  open: 'Open /openapi.json',
  retry: 'Try again',
} as const;

/**
 * Scalar's colours mapped onto the site's tokens, so the reference reads as part of
 * the docs in both themes. Its default fonts are off, since they load from a host the
 * page's font-src does not allow; it inherits the site's instead.
 */
export const SCALAR_CSS = `
.scalar-app, .light-mode, .dark-mode {
  --scalar-background-1: var(--color-bg);
  --scalar-background-2: var(--color-bg-card);
  --scalar-background-3: var(--color-bg-elevated);
  --scalar-border-color: var(--color-border);
  --scalar-color-1: var(--color-text);
  --scalar-color-2: var(--color-text-muted);
  --scalar-color-3: var(--color-text-dim);
  --scalar-color-accent: var(--color-accent);
  --scalar-color-green: var(--color-accent);
  --scalar-color-red: var(--color-red);
  --scalar-color-yellow: var(--color-yellow);
  --scalar-color-orange: var(--color-yellow);
  --scalar-color-blue: var(--color-text-secondary);
  --scalar-color-purple: var(--color-text-secondary);
  --scalar-font: inherit;
  --scalar-font-code: ui-monospace, SFMono-Regular, Menlo, monospace;
}`;
