/**
 * The side panel with the six-line SDK example and a button that copies it
 * with the real key.
 */
'use client';

import SyntaxCode from '@/components/ui/syntax-code';
import { Check, Copy, Terminal } from 'lucide-react';
import { sdkCode } from './sdk-code';
import { useCopyCode } from './use-copy-code';

/** Whose key and which profile the example uses. */
interface Props {
  /** The key, filled in only when copied. */
  apiKey: string;
  /** The profile the example starts. */
  profileId: string;
}

/** The first-browser example. */
export default function FirstBrowser({ apiKey, profileId }: Props) {
  const { copied, copy } = useCopyCode(apiKey, profileId);
  return (
    <aside className="min-w-0 self-start overflow-hidden rounded-lg border border-border bg-bg-sunken lg:sticky lg:top-6">
      <div className="flex items-center gap-2 border-b border-border px-5 py-4 text-sm">
        <Terminal className="h-4 w-4 text-accent" />
        <span className="font-medium">Your first browser, in six lines</span>
      </div>
      <div className="border-b border-border px-5 py-3 font-mono text-xs text-text-muted">
        npm install @oya-ai/browser
      </div>
      <pre className="overflow-x-auto p-5 font-mono text-xs leading-7 text-text-secondary">
        <SyntaxCode code={sdkCode('<your-api-key>', profileId)} language="typescript" />
      </pre>
      <div className="flex flex-wrap items-center gap-3 border-t border-border px-5 py-4">
        <button className="btn-ghost" onClick={copy}>
          {copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
          {copied ? 'Copied with your key' : 'Copy with your key'}
        </button>
        <a className="ml-auto text-xs text-text-muted hover:text-text" href="/docs/">
          Read the API guide ↗
        </a>
      </div>
      <p className="border-t border-border px-5 py-4 text-xs leading-relaxed text-text-muted">
        A saved profile is reused automatically. Check the MFA result for a human handoff, and call{' '}
        <code>browser.stop()</code> when the job is done.
      </p>
    </aside>
  );
}
