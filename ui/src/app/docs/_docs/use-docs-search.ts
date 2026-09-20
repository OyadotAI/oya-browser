/**
 * The sidebar search box's state: the query, debounced results, and what
 * Escape, clear and picking a result do.
 */
'use client';

import { useRef, useState, type KeyboardEvent, type RefObject } from 'react';
import { SEARCH_DEBOUNCE_MS } from './constants';
import { scrollToSection } from './nav';
import { searchDocs, type SearchHit } from './search-index';

/** The search box's state and what it can do. */
export interface DocsSearch {
  /** What is typed. */
  query: string;
  /** The results; null while hidden. */
  results: SearchHit[] | null;
  /** Typing: updates the query and searches after a pause. */
  onInput: (value: string) => void;
  /** Escape clears and leaves the box. */
  onKeyDown: (e: KeyboardEvent) => void;
  /** Empties the box. */
  clear: () => void;
  /** Goes to a result. */
  pick: (id: string) => void;
}

/** The pending search. */
type Timer = RefObject<ReturnType<typeof setTimeout> | null>;

/** Searches `value` once typing pauses. */
function debounced(timer: Timer, run: () => void) {
  if (timer.current) clearTimeout(timer.current);
  timer.current = setTimeout(run, SEARCH_DEBOUNCE_MS);
}

/** The query, results, input and pending search. */
function useSearchState() {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<SearchHit[] | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  return { query, setQuery, results, setResults, inputRef, timer };
}

/** The search box and its input ref; picking a result also runs `onPick` (closing the mobile menu). */
export function useDocsSearch(onPick: () => void): [DocsSearch, RefObject<HTMLInputElement | null>] {
  const { query, setQuery, results, setResults, inputRef, timer } = useSearchState();
  const clear = () => (setQuery(''), setResults(searchDocs('')));
  const onInput = (v: string) => (setQuery(v), debounced(timer, () => setResults(searchDocs(v))));
  const onKeyDown = (e: KeyboardEvent) => e.key === 'Escape' && (clear(), inputRef.current?.blur());
  const pick = (id: string) => (setQuery(''), setResults(null), onPick(), scrollToSection(id));
  return [{ query, results, onInput, onKeyDown, clear, pick }, inputRef];
}
