/**
 * The admin page's top bar: its name, the customer search and the tabs. It
 * stays at the top while the page scrolls, so search is always one click away.
 */
'use client';

import { SearchBox } from './lookup';
import { TABS, type TabId } from './constants';
import type { LookupState } from './use-lookup';

/** The bar's props. */
interface TopBarProps {
  /** The open tab. */
  tab: TabId;
  /** Opens a tab. */
  open: (tab: TabId) => void;
  /** The customer lookup. */
  l: LookupState;
}

/** The sticky top bar. */
export function TopBar({ tab, open, l }: TopBarProps) {
  return (
    <header className="sticky top-0 z-40 border-b border-border bg-bg/90 backdrop-blur">
      <div className="mx-auto flex max-w-6xl flex-col gap-3 px-4 pt-4 lg:px-6">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <h1 className="font-display text-xl tracking-tight text-text">Admin</h1>
          <SearchBox l={l} onSearch={() => open('customers')} />
        </div>
        <nav role="tablist" aria-label="Admin sections" className="-mb-px flex gap-5 overflow-x-auto">
          {TABS.map((t) => (
            <button
              key={t.id}
              role="tab"
              aria-selected={tab === t.id}
              onClick={() => open(t.id)}
              className={`border-b-2 pb-2.5 text-[13px] whitespace-nowrap transition-colors ${
                tab === t.id
                  ? 'border-accent font-medium text-text'
                  : 'border-transparent text-text-muted hover:text-text'
              }`}
            >
              {t.label}
            </button>
          ))}
        </nav>
      </div>
    </header>
  );
}
