/** Workflow selection stays inside React rather than a document-wide kit listener. */
'use client';
import { useState } from 'react';
import type { KeyboardEvent } from 'react';
import { WORKFLOWS } from './content';

/** Keyboard destinations keep horizontal and vertical arrows compatible with the old tabs. */
const DESTINATIONS: Record<string, (index: number, count: number) => number> = {
  ArrowRight: (index, count) => (index + 1) % count,
  ArrowDown: (index, count) => (index + 1) % count,
  ArrowLeft: (index, count) => (index - 1 + count) % count,
  ArrowUp: (index, count) => (index - 1 + count) % count,
  Home: () => 0,
  End: (_index, count) => count - 1,
};

/** The event, current selection, bounded task count and React selection callback. */
type NavigationArgs = [KeyboardEvent<HTMLButtonElement>, number, number, (index: number) => void];

/** Moves selection and focus together, wrapping at either end of the workflow list. */
function navigate(...[event, active, count, select]: NavigationArgs) {
  if (!Object.hasOwn(DESTINATIONS, event.key)) return;
  event.preventDefault();
  const next = DESTINATIONS[event.key](active, count);
  select(next);
  event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>('[role="tab"]')[next]?.focus();
}

/** Returns one active workflow shared by its description, form and tab state. */
export function useWorkflow(count = WORKFLOWS.length) {
  const [active, select] = useState(0);
  return {
    active,
    select,
    onKeyDown: (event: KeyboardEvent<HTMLButtonElement>) => navigate(event, active, count, select),
  };
}
