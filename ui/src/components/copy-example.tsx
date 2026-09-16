'use client';

import { useState } from 'react';

export default function CopyExample({ code }: { code: string }) {
  const [status, setStatus] = useState<'idle' | 'copied' | 'failed'>('idle');

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
        {status === 'copied' ? 'Example copied.' : status === 'failed' ? 'Could not copy. Select the code to copy it manually.' : ''}
      </span>
      {status === 'failed' && <span className="block mt-1 text-xs">Select the code to copy it manually.</span>}
    </span>
  );
}
