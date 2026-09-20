/**
 * Everything the fleet table computes from its props: the sorted, filtered,
 * paged rows, the checkbox state, and the scroll-follow of the selection.
 */
import type { BrowserRow } from '../types';
import type { FleetFilter } from '../fleet-strip';
import { useFleetView } from './use-fleet-view';
import { useRowChecks } from './use-row-checks';
import { useScrollToSelected } from './use-scroll-to-selected';

/** The props the table's state is derived from. */
export interface FleetTableInputs {
  /** Every browser. */
  rows: BrowserRow[];
  /** Current filter. */
  filter: FleetFilter;
  /** Browser open in the side panel. */
  selectedId: string | null;
  /** Ticked browser ids. */
  checked: Set<string>;
  /** Replaces the ticked set. */
  onChecked: (next: Set<string>) => void;
}

/** View, checks and the body ref, together. */
export function useFleetTable({ rows, filter, selectedId, checked, onChecked }: FleetTableInputs) {
  const view = useFleetView(rows, filter);
  const checks = useRowChecks(view.shown, checked, onChecked);
  const bodyRef = useScrollToSelected(selectedId);
  return { ...view, ...checks, bodyRef };
}

/** What `useFleetTable` returns. */
export type FleetTableState = ReturnType<typeof useFleetTable>;
