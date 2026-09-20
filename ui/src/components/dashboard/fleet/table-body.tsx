/**
 * The table's rows, or a "no match" row when the filter hides them all.
 */
import type { RefObject } from 'react';
import type { BrowserRow } from '../types';
import type { FleetFilter } from '../fleet-strip';
import { NO_FILTER, TABLE_COL_SPAN } from './constants';
import FleetRow, { type FleetRowProps } from './fleet-row';

/** The rows to paint, plus what each row needs. */
interface Props extends Omit<FleetRowProps, 'r' | 'selected' | 'checked'> {
  /** Rows painted. */
  shown: BrowserRow[];
  /** Id of the row open in the side panel. */
  selectedId: string | null;
  /** Ticked row ids. */
  checked: Set<string>;
  /** Merges a change into the filter; used to clear it. */
  onFilter: (next: Partial<FleetFilter>) => void;
  /** The body element, for keeping the selection in view. */
  bodyRef: RefObject<HTMLTableSectionElement | null>;
}

/** One FleetRow per shown browser. */
export default function TableBody({ shown, selectedId, checked, onFilter, bodyRef, ...row }: Props) {
  return (
    <tbody ref={bodyRef}>
      {!shown.length && (
        <tr>
          <td colSpan={TABLE_COL_SPAN} className="py-16 text-center text-text-muted">
            No browsers match these filters.
            <button className="btn-ghost mx-auto mt-3" onClick={() => onFilter(NO_FILTER)}>
              Clear filters
            </button>
          </td>
        </tr>
      )}
      {shown.map((r) => (
        <FleetRow key={r.id} r={r} selected={r.id === selectedId} checked={checked.has(r.id)} {...row} />
      ))}
    </tbody>
  );
}
