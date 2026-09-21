/**
 * Keeps the selected row in view as the selection moves by keyboard.
 */
import { useEffect, useRef } from 'react';

/** Returns the ref for the table body; scrolls the row with `data-id={selectedId}` into view. */
export function useScrollToSelected(selectedId: string | null) {
  const bodyRef = useRef<HTMLTableSectionElement>(null);
  useEffect(() => {
    if (!selectedId) return;
    bodyRef.current?.querySelector<HTMLElement>(`[data-id="${selectedId}"]`)?.scrollIntoView({ block: 'nearest' });
  }, [selectedId]);
  return bodyRef;
}
