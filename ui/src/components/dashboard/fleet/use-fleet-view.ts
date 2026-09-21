/**
 * The fleet table's view state: the sort, how many rows are painted, and the
 * rows that result from the filter and sort.
 */
import { useMemo, useState } from 'react';
import type { BrowserRow } from '../types';
import type { FleetFilter } from '../fleet-strip';
import { PAGE_SIZE, type SortKey } from './constants';
import { visibleRows, type Sort } from './rows';

/** The next sort when a heading is clicked: the same column flips, a new one starts ascending. */
export const nextSort = (sort: Sort, key: SortKey): Sort => ({
  key,
  dir: sort.key === key ? (sort.dir === 1 ? -1 : 1) : 1,
});

/** Sorted, filtered rows, painted a page at a time. */
export function useFleetView(rows: BrowserRow[], filter: FleetFilter) {
  const [sort, setSort] = useState<Sort>({ key: 'health', dir: 1 });
  const [limit, setLimit] = useState(PAGE_SIZE);
  const visible = useMemo(() => visibleRows(rows, filter, sort), [rows, filter, sort]);
  const toggleSort = (key: SortKey) => setSort(nextSort(sort, key));
  return { sort, toggleSort, limit, setLimit, visible, shown: visible.slice(0, limit) };
}
