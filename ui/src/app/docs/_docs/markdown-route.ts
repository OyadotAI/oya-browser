/**
 * Serves the docs as Markdown: /docs.md, /docs/<slug>.md and /llms-full.txt.
 * Route handlers cannot render the docs' client components, so this asks this
 * same server for /docs and converts the HTML it gets back. A production
 * server renders it once; a dev server every time, so edits show at once.
 */
import { SITE_URL } from '@/lib/site';
import { Status } from '@/lib/http-status';
import { MARKDOWN_MAX_AGE_SECONDS, NEXT_DEFAULT_PORT } from './constants';
import { docsMarkdown, type DocsMarkdown } from './markdown';
import { docPage } from './pages';

/** The Content-Type of a .md twin. */
export const MARKDOWN_TYPE = 'text/markdown; charset=utf-8';

/** The converted docs, once a production server has them. */
let cached: Promise<DocsMarkdown> | null = null;

/**
 * Where this Next.js server listens. Never the request's Host: that is the
 * caller's to choose, and a cached answer fetched from it would be served to
 * everyone. Next.js records its own origin when it starts listening.
 */
function selfOrigin(): string {
  return process.env.__NEXT_PRIVATE_ORIGIN || `http://localhost:${process.env.PORT || NEXT_DEFAULT_PORT}`;
}

/** The docs page as this server renders it, converted. */
async function fetchDocs(): Promise<DocsMarkdown> {
  const res = await fetch(new URL('/docs', selfOrigin()), { headers: { accept: 'text/html' }, cache: 'no-store' });
  if (!res.ok) throw new Error(`/docs answered ${res.status}`);
  return docsMarkdown(await res.text());
}

/** The converted docs; kept by a production server, forgotten again if the render failed. */
function loadDocs(): Promise<DocsMarkdown> {
  const load = cached ?? fetchDocs();
  cached = process.env.NODE_ENV === 'production' ? load : null;
  load.catch(() => (cached = null));
  return load;
}

/** A 200 carrying `body`, with the HTML page it mirrors as its canonical. */
function textResponse(body: string, contentType: string, canonical: string): Response {
  const headers = {
    'content-type': contentType,
    'cache-control': `public, max-age=${MARKDOWN_MAX_AGE_SECONDS}`,
    link: `<${SITE_URL}${canonical}>; rel="canonical"`,
  };
  return new Response(body, { headers });
}

/** What a failed render answers. */
function unavailable(): Response {
  return new Response('The docs could not be rendered just now. Please retry.\n', { status: Status.BAD_GATEWAY });
}

/** The whole docs, as /docs.md (Markdown) or /llms-full.txt (the same text as plain text). */
export async function fullDocsResponse(contentType = MARKDOWN_TYPE): Promise<Response> {
  const docs = await loadDocs().catch(() => null);
  return docs ? textResponse(docs.full, contentType, '/docs') : unavailable();
}

/** The part a path segment such as "sdk.md" names, under its title; 404 for anything else. */
export async function pageResponse(segment: string): Promise<Response> {
  const page = segment.endsWith('.md') ? docPage(segment.slice(0, -'.md'.length)) : undefined;
  if (!page) return new Response('No such docs page. The list is at /llms.txt.\n', { status: Status.NOT_FOUND });
  const docs = await loadDocs().catch(() => null);
  const body = docs?.pages[page.slug];
  if (!body) return unavailable();
  const intro = `# ${page.title}\n\nPart of the Oya Browser docs. All of it: ${SITE_URL}/docs.md\n\n`;
  return textResponse(intro + body, MARKDOWN_TYPE, '/docs');
}
