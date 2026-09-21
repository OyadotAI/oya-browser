/**
 * The table's checkboxes: one per shown row, plus "select all shown".
 */
import type { BrowserRow } from '../types';

/** Adds `id` to a copy of the set, or removes it if present. */
export function toggled(set: Set<string>, id: string): Set<string> {
  const next = new Set(set);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  return next;
}

/** Checkbox state for the shown rows; `checked` is owned by the page. */
export function useRowChecks(shown: BrowserRow[], checked: Set<string>, onChecked: (next: Set<string>) => void) {
  const allChecked = shown.length > 0 && shown.every((r) => checked.has(r.id));
  const toggleAll = () => onChecked(allChecked ? new Set() : new Set(shown.map((r) => r.id)));
  const toggle = (id: string) => onChecked(toggled(checked, id));
  return { allChecked, toggleAll, toggle };
}
