/**
 * The table's header row: select-all, then a sort button per column.
 */
import { ArrowDown, ArrowUp } from 'lucide-react';
import { COLUMNS, type Column, type SortKey } from './constants';
import type { Sort } from './rows';

/** The sort and the select-all state. */
interface Props {
  /** Current sort. */
  sort: Sort;
  /** Sorts by a column, or flips it if it is already the sort. */
  onSort: (key: SortKey) => void;
  /** Whether every shown row is checked. */
  allChecked: boolean;
  /** Checks or unchecks every shown row. */
  onToggleAll: () => void;
}

/** The aria-sort a column announces. */
const ariaSort = (active: boolean, dir: 1 | -1) => (active ? (dir === 1 ? 'ascending' : 'descending') : 'none');

/** A column, the sort, and how to change it. */
interface HeadingProps {
  /** The column headed. */
  c: Column;
  /** Current sort. */
  sort: Sort;
  /** Sorts by a column. */
  onSort: (key: SortKey) => void;
}

/** One column heading; the health column has no label and so no button. */
function Heading({ c, sort, onSort }: HeadingProps) {
  const active = sort.key === c.key;
  const align = c.className.includes('text-right') ? 'text-right' : 'text-left';
  return (
    <th
      aria-sort={ariaSort(active, sort.dir)}
      className={`${c.className} px-2 py-3 ${align} text-[11px] font-medium uppercase tracking-[0.1em] text-text-muted select-none`}
    >
      {c.label ? (
        <button className="inline-flex items-center gap-1 hover:text-text" onClick={() => onSort(c.key)}>
          {c.label}
          {active && (sort.dir === 1 ? <ArrowUp className="h-3 w-3" /> : <ArrowDown className="h-3 w-3" />)}
        </button>
      ) : null}
    </th>
  );
}

/** Sticky header with the select-all box and sortable columns. */
export default function TableHead({ sort, onSort, allChecked, onToggleAll }: Props) {
  return (
    <thead className="sticky top-0 z-10 bg-bg-elevated">
      <tr className="border-b border-border">
        <th className="w-8 px-2 py-3">
          <input
            type="checkbox"
            checked={allChecked}
            onChange={onToggleAll}
            aria-label="Select all shown"
            className="accent-accent"
          />
        </th>
        {COLUMNS.map((c) => (
          <Heading key={c.key} c={c} sort={sort} onSort={onSort} />
        ))}
        <th className="w-[80px]" />
      </tr>
    </thead>
  );
}
