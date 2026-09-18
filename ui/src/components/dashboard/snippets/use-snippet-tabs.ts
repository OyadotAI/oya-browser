/**
 * State for the snippets dialog: which tab is open, whether the key shows,
 * and the brief "Copied" confirmation.
 */
import { useState } from 'react';
import { COPIED_RESET_MS } from './constants';
import type { Snippet } from './types';

/** The open snippet and the actions on it. Copying always copies the real key. */
export function useSnippetTabs(snippets: Snippet[], apiKey: string) {
  const [active, setActive] = useState(snippets[0]?.id);
  const [reveal, setReveal] = useState(false);
  const [copied, setCopied] = useState(false);
  const snippet = snippets.find((x) => x.id === active) || snippets[0];
  const choose = (id: string) => (setActive(id), setCopied(false));
  const copy = () =>
    navigator.clipboard.writeText(snippet.code(apiKey)).then(() => (setCopied(true), flashReset(setCopied)));
  return { snippet, reveal, copied, choose, copy, toggleReveal: () => setReveal(!reveal) };
}

/** Clears the "Copied" state after a moment. */
function flashReset(setCopied: (v: boolean) => void) {
  setTimeout(() => setCopied(false), COPIED_RESET_MS);
}
