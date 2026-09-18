/**
 * Above the table: the filter, the row count, and the fleet-wide actions.
 */
import type { RefObject } from 'react';
import { X } from 'lucide-react';
import type { FleetFilter } from '../fleet-strip';
import { NO_FILTER } from './constants';
import { anyFilter } from './rows';
import FilterInput from './filter-input';
import ToolbarActions, { type ToolbarActionsProps } from './toolbar-actions';

/** Everything the toolbar shows and calls. */
interface Props extends ToolbarActionsProps {
  /** Current filter. */
  filter: FleetFilter;
  /** Merges a change into the filter. */
  onFilter: (next: Partial<FleetFilter>) => void;
  /** Lets the page's `/` shortcut focus the filter. */
  filterRef: RefObject<HTMLInputElement | null>;
  /** Rows that pass the filter. */
  visibleCount: number;
}

/** Filter, clear, count, then the actions on the right. */
export default function Toolbar({ filter, onFilter, filterRef, visibleCount, ...actions }: Props) {
  const total = actions.rows.length;
  return (
    <div className="flex shrink-0 flex-wrap items-center gap-3 px-4 py-4 lg:px-6">
      <FilterInput filter={filter} onFilter={onFilter} filterRef={filterRef} />
      {anyFilter(filter) && (
        <button className="btn-ghost h-9 px-2 text-[12px]" onClick={() => onFilter(NO_FILTER)}>
          <X className="h-3 w-3" /> Clear
        </button>
      )}
      <span className="ml-1 text-[12px] num text-text-muted">
        {visibleCount}
        {visibleCount !== total ? ` of ${total}` : ''}
      </span>
      <ToolbarActions {...actions} />
    </div>
  );
}
