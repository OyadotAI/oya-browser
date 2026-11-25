/**
 * The interactive API reference: Scalar, loaded from a pinned and hashed CDN build,
 * rendering /openapi.json. While it loads the page says so; if it does not, the page
 * says that and links the raw spec, which is what an agent or a curl wants anyway.
 */
'use client';

import Script from 'next/script';
import { REFERENCE_TEXT, SCALAR_INTEGRITY, SCALAR_SRC } from './constants';
import { useApiReference } from './use-api-reference';

/** Props for the reference. */
interface Props {
  /** The page's CSP nonce, without which the script is refused. */
  nonce?: string;
  /** Where the spec is read from. */
  spec: string;
}

/** The spec did not render: say so, and offer the raw JSON and a retry. */
function Failed({ spec }: { /** The raw spec's address. */ spec: string }) {
  return (
    <div role="alert" className="rounded-xl border border-border bg-bg-card/40 p-6 text-[14px] text-text-muted">
      <p className="mb-3 text-text">{REFERENCE_TEXT.failed}</p>
      <div className="flex flex-wrap gap-4">
        <a href={spec} className="text-accent hover:text-accent-hover">
          {REFERENCE_TEXT.open}
        </a>
        <button type="button" className="text-accent hover:text-accent-hover" onClick={() => window.location.reload()}>
          {REFERENCE_TEXT.retry}
        </button>
      </div>
    </div>
  );
}

/** Scalar's script, the element it renders into, and the loading and failed states around it. */
export default function ApiReference({ nonce, spec }: Props) {
  const { host, status, onReady, onError } = useApiReference(spec);
  return (
    <>
      <Script
        src={SCALAR_SRC}
        integrity={SCALAR_INTEGRITY}
        crossOrigin="anonymous"
        nonce={nonce}
        strategy="afterInteractive"
        onReady={onReady}
        onError={onError}
      />
      {status === 'loading' && (
        <p role="status" className="grid min-h-[60vh] place-items-center text-[14px] text-text-muted">
          {REFERENCE_TEXT.loading}
        </p>
      )}
      {status === 'failed' && <Failed spec={spec} />}
      <div ref={host} hidden={status !== 'ready'} className="api-reference" />
    </>
  );
}
