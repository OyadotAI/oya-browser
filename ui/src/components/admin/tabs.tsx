/** Manual-activation tabs support arrow-key focus without surprising navigation or data refreshes. */
'use client';
import { useRef, useState, type KeyboardEvent } from 'react';
import { SearchBox } from './lookup';
import { TABS, TAB_STEP, type TabId } from './constants';
import type { LookupState } from './use-lookup';

/** Page-owned selection and customer lookup remain stable across tab changes. */
interface TopBarProps {
  /** Selected URL-backed section. */ tab: TabId;
  /** Changes selection only on activation. */ open: (tab: TabId) => void;
  /** Shared customer lookup. */ l: LookupState;
}

/** Only supported navigation keys move roving focus; Enter and Space retain native button activation. */
function focusIndex(event: KeyboardEvent, index: number) {
  const targets = {
    ArrowRight: (index + TAB_STEP) % TABS.length,
    ArrowLeft: (index - TAB_STEP + TABS.length) % TABS.length,
    Home: 0,
    End: TABS.length - TAB_STEP,
  };
  if (!Object.hasOwn(targets, event.key)) return null;
  event.preventDefault();
  return targets[event.key as keyof typeof targets];
}

/** Focused and selected tabs are independent until a user activates the focused button. */
function useRoving(tab: TabId) {
  const [focusState, keepFocus] = useState({ id: tab, selected: tab });
  const focused = focusState.selected === tab ? focusState.id : tab;
  const focus = (id: TabId) => keepFocus({ id, selected: tab });
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  const keyDown = (event: KeyboardEvent, index: number) => {
    const target = focusIndex(event, index);
    if (target !== null) {
      focus(TABS[target].id);
      refs.current[target]?.focus();
    }
  };
  return { focused, focus, refs, keyDown };
}

/** Tab controls stay a single keyboard stop with manual Enter/Space activation. */
function TabStrip({ tab, open }: Pick<TopBarProps, 'tab' | 'open'>) {
  const roving = useRoving(tab);
  return (
    <nav role="tablist" aria-label="Admin sections" className="-mb-px flex gap-5 overflow-x-auto">
      {TABS.map((item, index) => (
        <button
          key={item.id}
          type="button"
          role="tab"
          id={`admin-tab-${item.id}`}
          ref={(element) => {
            roving.refs.current[index] = element;
          }}
          aria-controls="admin-panel"
          tabIndex={roving.focused === item.id ? 0 : -TAB_STEP}
          aria-selected={tab === item.id}
          onFocus={() => roving.focus(item.id)}
          onKeyDown={(event) => roving.keyDown(event, index)}
          onClick={() => open(item.id)}
          className={`min-h-11 whitespace-nowrap border-b-2 pb-2.5 text-[13px] transition-colors focus-visible:outline-accent ${tab === item.id ? 'border-accent font-medium text-text' : 'border-transparent text-text-muted hover:text-text'}`}
        >
          {item.label}
        </button>
      ))}
    </nav>
  );
}

/** Search remains reachable from every section without duplicating customer selection state. */
export function TopBar({ tab, open, l }: TopBarProps) {
  return (
    <header className="sticky top-0 z-40 border-b border-border bg-bg/90 backdrop-blur">
      <div className="mx-auto flex max-w-6xl flex-col gap-3 px-4 pt-4 lg:px-6">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <h1 className="font-display text-xl tracking-tight text-text">Admin</h1>
          <SearchBox l={l} onSearch={() => open('customers')} />
        </div>
        <TabStrip tab={tab} open={open} />
      </div>
    </header>
  );
}
