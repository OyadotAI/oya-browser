/**
 * Below a table cut off at its page size: paint the next page, or all of it.
 */
import { PAGE_SIZE } from './constants';

/** How many rows are painted, how many there are, and how to change it. */
interface Props {
  /** Rows painted now. */
  limit: number;
  /** Rows that pass the filter. */
  total: number;
  /** Sets how many rows are painted. */
  setLimit: (n: number) => void;
}

/** "Showing N of M", then "Show more" and "Show all". */
export default function ShowMore({ limit, total, setLimit }: Props) {
  return (
    <div className="flex items-center justify-center gap-3 py-3 text-[12.5px] text-text-muted">
      Showing {limit} of {total}
      <button className="btn-ghost h-9" onClick={() => setLimit(limit + PAGE_SIZE)}>
        Show {Math.min(PAGE_SIZE, total - limit)} more
      </button>
      <button className="btn-ghost h-9" onClick={() => setLimit(total)}>
        Show all
      </button>
    </div>
  );
}
