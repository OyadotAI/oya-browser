/**
 * The docs page as a server renders it, for tests of its Markdown twins: they
 * fetch /docs from their own server, and this stands in for that answer.
 */
import { renderToStaticMarkup } from 'react-dom/server';
import { vi } from 'vitest';
import DocsPage from '@/app/docs/page';

/** Makes fetch answer with the real docs page's HTML; returns the fake. */
export function serveDocsHtml() {
  const html = `<!DOCTYPE html><html><body>${renderToStaticMarkup(<DocsPage />)}</body></html>`;
  const fn = vi.fn(async () => new Response(html, { headers: { 'content-type': 'text/html' } }));
  vi.stubGlobal('fetch', fn);
  return fn;
}
