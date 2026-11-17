/**
 * /docs/api/reference: every public API route, rendered from /openapi.json by Scalar.
 * /docs/api stays the hand-written guide (commands, WebSocket); this is the
 * reference beside it. A server component, so it can read the request's CSP nonce
 * and export its own metadata.
 */
import type { Metadata } from 'next';
import Link from 'next/link';
import { headers } from 'next/headers';
import { ArrowLeft } from 'lucide-react';
import { DocsHeader } from '../../_docs/docs-header';
import ApiReference from './api-reference';
import { REFERENCE_TEXT, specUrl } from './constants';

/** Its own title, description and address. */
export const metadata: Metadata = {
  title: REFERENCE_TEXT.title,
  description: REFERENCE_TEXT.description,
  alternates: { canonical: '/docs/api/reference', types: { 'application/json': '/openapi.json' } },
};

/** The heading and the line on how to call the API, with the raw spec's link for readers without JavaScript. */
function Intro({ spec }: { /** The raw spec's address. */ spec: string }) {
  return (
    <>
      <Link
        href="/docs/api"
        className="mb-5 inline-flex items-center gap-1.5 text-[12px] text-text-dim hover:text-text"
      >
        <ArrowLeft size={13} /> API guide
      </Link>
      <p className="eyebrow text-accent mb-3">Oya Browser docs</p>
      <h1 className="text-[30px] sm:text-[40px] leading-[1.1] font-medium tracking-[-.045em] text-text mb-5">
        {REFERENCE_TEXT.title}
      </h1>
      <p className="docs-lede mb-10 max-w-xl text-text-muted">
        {REFERENCE_TEXT.intro} <code>{REFERENCE_TEXT.header}</code> {REFERENCE_TEXT.download}{' '}
        <a href={spec} className="text-accent hover:text-accent-hover">
          /openapi.json
        </a>
        .
      </p>
    </>
  );
}

/** The reference page: the docs header, the intro, and the interactive reference. */
export default async function ApiReferencePage() {
  const nonce = (await headers()).get('x-nonce') ?? undefined;
  const spec = specUrl();
  return (
    <div className="docs-page min-h-screen bg-bg">
      <DocsHeader />
      <main className="min-w-0 px-5 pt-28 pb-24 sm:px-10 lg:pt-32">
        <div className="mx-auto max-w-[1200px]">
          <Intro spec={spec} />
          <ApiReference nonce={nonce} spec={spec} />
        </div>
      </main>
    </div>
  );
}
