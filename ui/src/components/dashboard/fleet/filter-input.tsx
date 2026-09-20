/**
 * The fleet table's free-text filter. `/` focuses it; Escape clears and leaves it.
 */
import type { RefObject } from 'react';
import { Search } from 'lucide-react';
import Kbd from '@/components/ui/kbd';
import type { FleetFilter } from '../fleet-strip';

/** The filter text and where changes go. */
interface Props {
  /** Current filter; only its text is edited here. */
  filter: FleetFilter;
  /** Merges a change into the filter. */
  onFilter: (next: Partial<FleetFilter>) => void;
  /** Lets the page's `/` shortcut focus the input. */
  filterRef: RefObject<HTMLInputElement | null>;
}

/** Search box over name, url, persona and provider. */
export default function FilterInput({ filter, onFilter, filterRef }: Props) {
  return (
    <div className="relative min-w-[180px] flex-1 max-w-md">
      <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-text-dim" />
      <input
        ref={filterRef}
        value={filter.text}
        onChange={(e) => onFilter({ text: e.target.value })}
        onKeyDown={(e) => {
          if (e.key === 'Escape') {
            onFilter({ text: '' });
            (e.target as HTMLInputElement).blur();
          }
        }}
        placeholder="Filter by name, url, persona, provider…"
        className="field pl-8 pr-12"
        aria-label="Filter browsers"
      />
      <span className="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2">
        <Kbd>/</Kbd>
      </span>
    </div>
  );
}
