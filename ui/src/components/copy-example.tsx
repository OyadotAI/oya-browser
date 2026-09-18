/**
 * The copy button beside a code example, with a spoken status and a manual
 * fallback when the clipboard is refused.
 */
'use client';

import { useState } from 'react';

/** CopyExample's props. */
interface Props {
  /** The text to copy. */
  code: string;
}

/** Copies `code` and says whether it worked. */
export default function CopyExample({ code }: Props) {
  const [status, setStatus] = useState<'idle' | 'copied' | 'failed'>('idle');

  /** Writes the code to the clipboard. */
  async function copy() {
    try {
      await navigator.clipboard.writeText(code);
      setStatus('copied');
    } catch {
      setStatus('failed');
    }
  }

  return (
    <span>
      <button type="button" onClick={copy} aria-label="Copy example">
        {status === 'copied' ? 'Copied!' : 'Copy'}
      </button>
      <span role="status" className="sr-only">
        {status === 'copied'
          ? 'Example copied.'
          : status === 'failed'
            ? 'Could not copy. Select the code to copy it manually.'
            : ''}
      </span>
      {status === 'failed' && <span className="block mt-1 text-xs">Select the code to copy it manually.</span>}
    </span>
  );
}
