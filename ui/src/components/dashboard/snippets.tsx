/**
 * The snippets dialog, and the facade for the snippet builders in
 * `snippets/`: the dashboard imports `browserSnippets`, `fleetSnippets` and
 * `origins` from here.
 */
'use client';

import Dialog from '@/components/ui/dialog';
import SyntaxCode from '@/components/ui/syntax-code';
import { DEFAULT_LANGUAGE, MASK, SNIPPET_LANGUAGE } from './snippets/constants';
import SnippetToolbar from './snippets/snippet-toolbar';
import type { Snippet } from './snippets/types';
import { useSnippetTabs } from './snippets/use-snippet-tabs';

export { origins } from './snippets/origins';
export { browserSnippets } from './snippets/browser';
export { fleetSnippets } from './snippets/fleet';
export type { Snippet } from './snippets/types';

/** What the dialog shows. */
interface Props {
  /** Whether the dialog is open. */
  open: boolean;
  /** Closes the dialog. */
  onClose: () => void;
  /** The key copied into the code. */
  apiKey: string;
  /** Dialog title. */
  title: string;
  /** Line under the title. */
  description?: string;
  /** The tabs to offer. */
  snippets: Snippet[];
}

/** The highlighting language for a snippet id. */
const languageOf = (id: string) => (Object.hasOwn(SNIPPET_LANGUAGE, id) ? SNIPPET_LANGUAGE[id] : DEFAULT_LANGUAGE);

/**
 * Code that works when pasted. The key is masked until asked for, and copying
 * always copies the real thing — nobody wants to paste a placeholder.
 */
export default function SnippetsDialog({ open, onClose, apiKey, title, description, snippets }: Props) {
  const { snippet: s, reveal, copied, choose, copy, toggleReveal } = useSnippetTabs(snippets, apiKey);
  if (!s) return null;
  return (
    <Dialog open={open} onClose={onClose} title={title} description={description} size="lg">
      <SnippetToolbar
        snippets={snippets}
        activeId={s.id}
        reveal={reveal}
        copied={copied}
        onChoose={choose}
        onToggleReveal={toggleReveal}
        onCopy={copy}
      />
      <div className="overflow-hidden rounded-lg border border-border bg-bg">
        <div className="flex items-center justify-between border-b border-border bg-bg-elevated/40 px-3 py-1.5">
          <span className="font-mono text-[11px] text-text-dim">{s.file}</span>
        </div>
        <pre className="overflow-x-auto px-5 py-5 font-mono text-[12.5px] leading-[1.9] text-text-secondary">
          <SyntaxCode code={s.code(reveal ? apiKey : MASK)} language={languageOf(s.id)} />
        </pre>
      </div>
      {s.note && <p className="mt-3 text-[12.5px] text-text-muted">{s.note}</p>}
    </Dialog>
  );
}
