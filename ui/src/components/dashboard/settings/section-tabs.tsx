/**
 * The settings dialog's side navigation: a tab per section, with arrow-key
 * navigation, and the "Run setup again" link.
 */
'use client';

import type { KeyboardEvent } from 'react';
import { RotateCcw } from 'lucide-react';
import { SECTIONS, type Section } from './constants';
import { nextTab } from './tabs';

/** Moves the selection and focus when an arrow, Home or End key is pressed on the tab list. */
function onTabKey(e: KeyboardEvent<HTMLDivElement>, section: Section, select: (s: Section) => void) {
  const next = nextTab(
    e.key,
    SECTIONS.findIndex((s) => s.id === section),
    SECTIONS.length,
  );
  if (next === null) return;
  e.preventDefault();
  select(SECTIONS[next].id);
  e.currentTarget.querySelectorAll<HTMLButtonElement>('[role="tab"]')[next].focus();
}

/** One section tab. */
function Tab({
  s,
  active,
  onSelect,
}: {
  /** The section. */
  s: (typeof SECTIONS)[number];
  /** It is the open one. */
  active: boolean;
  /** Opens it. */
  onSelect: () => void;
}) {
  const tone = active
    ? 'bg-bg-card text-text shadow-sm ring-1 ring-border'
    : 'text-text-muted hover:bg-text/5 hover:text-text';
  return (
    <button
      role="tab"
      id={`settings-tab-${s.id}`}
      aria-selected={active}
      aria-controls={`settings-panel-${s.id}`}
      tabIndex={active ? 0 : -1}
      onClick={onSelect}
      className={`flex flex-1 items-center gap-2.5 rounded-lg px-3 py-2.5 text-left text-[12.5px] font-medium md:flex-none ${tone}`}
    >
      <s.icon className={`hidden h-4 w-4 shrink-0 sm:block ${active ? 'text-accent' : ''}`} />
      {s.label}
    </button>
  );
}

/** The navigation column. */
export default function SectionTabs({
  section,
  onSelect,
  onRerun,
  saving,
}: {
  /** The open section. */
  section: Section;
  /** Opens a section. */
  onSelect: (s: Section) => void;
  /** Closes the dialog and restarts onboarding, when offered. */
  onRerun?: () => void;
  /** Disables the rerun link while saving. */
  saving: boolean;
}) {
  return (
    <nav
      aria-label="Settings sections"
      className="flex flex-col border-b border-border bg-bg-sunken/60 md:border-b-0 md:border-r"
    >
      <div
        role="tablist"
        aria-label="Settings"
        className="flex gap-1 p-3 md:flex-col md:py-5"
        onKeyDown={(e) => onTabKey(e, section, onSelect)}
      >
        {SECTIONS.map((s) => (
          <Tab key={s.id} s={s} active={section === s.id} onSelect={() => onSelect(s.id)} />
        ))}
      </div>
      {onRerun && (
        <button
          onClick={onRerun}
          disabled={saving}
          className="m-3 mt-auto hidden items-center gap-2 rounded-md px-3 py-2 text-left text-[11.5px] text-text-muted hover:bg-text/5 hover:text-text md:flex"
        >
          <RotateCcw className="h-3.5 w-3.5" />
          Run setup again
        </button>
      )}
    </nav>
  );
}
