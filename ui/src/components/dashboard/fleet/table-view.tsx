/**
 * The scrolling table area: the empty state, or the table and its "show more".
 */
import type { MouseEvent } from 'react';
import type { BrowserRow } from '../types';
import type { FleetTableProps } from '../fleet-table';
import type { FleetTableState } from './use-fleet-table';
import Empty from './empty';
import ShowMore from './show-more';
import TableHead from './table-head';
import TableBody from './table-body';

/** The table's props, its derived state, and the row menu opener. */
interface Props extends FleetTableProps {
  /** State from `useFleetTable`. */
  view: FleetTableState;
  /** Opens the right-click menu for a row. */
  onMenu: (e: MouseEvent, r: BrowserRow) => void;
}

/** Table, or the empty state when nothing runs. */
export default function FleetTableView({ view, onMenu, rows, onStart, ...p }: Props) {
  const { visible, limit, setLimit } = view;
  return (
    <div className="data-scroll mx-4 mb-4 min-h-0 flex-initial overflow-auto rounded-xl border border-border bg-bg-card/25 lg:mx-6 lg:mb-6">
      {rows.length === 0 ? (
        <Empty onStart={onStart} />
      ) : (
        <table className="data-table fleet-data w-full table-fixed border-collapse text-[13px]" aria-label="Browsers">
          <TableHead
            sort={view.sort}
            onSort={view.toggleSort}
            allChecked={view.allChecked}
            onToggleAll={view.toggleAll}
          />
          <TableBody
            shown={view.shown}
            bodyRef={view.bodyRef}
            selectedId={p.selectedId}
            checked={p.checked}
            onFilter={p.onFilter}
            onSelect={p.onSelect}
            onToggle={view.toggle}
            onMenu={onMenu}
            onConnect={p.onConnect}
            onStop={p.onStop}
            now={p.now}
          />
        </table>
      )}
      {visible.length > limit && <ShowMore limit={limit} total={visible.length} setLimit={setLimit} />}
    </div>
  );
}
