/**
 * The fleet strip's filter chips, one group per dimension: health, provider,
 * persona. Clicking a chip filters the table; clicking it again clears that filter.
 */
import type { Fleet } from '../types';
import { HEALTH_LABEL, providerLabel } from '../types';
import type { FleetFilter } from '../fleet-strip';
import { HEALTH_ORDER } from './constants';
import { chipClass } from './chip';

/** The filter and where changes go. */
interface FilterProps {
  /** Current filter. */
  filter: FleetFilter;
  /** Merges a change into the filter. */
  onFilter: (next: Partial<FleetFilter>) => void;
}

/** A count group: its `[value, count]` entries, largest first. */
interface CountProps extends FilterProps {
  /** `[value, count]` pairs to show. */
  entries: [string, number][];
}

/** One chip per health, always all four, zero or not. */
export function HealthChips({ b, filter, onFilter }: FilterProps & { /** Latest counts. */ b?: Fleet['browsers'] }) {
  return (
    <div className="flex max-w-full flex-wrap items-center gap-1.5" role="group" aria-label="Filter by health">
      {HEALTH_ORDER.map((h) => {
        const n = b?.byHealth?.[h] ?? 0;
        const active = filter.health === h;
        return (
          <button
            key={h}
            className={chipClass(active)}
            onClick={() => onFilter({ health: active ? null : h })}
            title={`${n} ${HEALTH_LABEL[h]}`}
            aria-pressed={active}
          >
            <span className={`dot dot-${h}`} />
            <span>{n}</span>
            <span className="text-text-muted">{HEALTH_LABEL[h]}</span>
          </button>
        );
      })}
    </div>
  );
}

/** One chip per provider in use. */
export function ProviderChips({ entries, filter, onFilter }: CountProps) {
  return (
    <div className="flex max-w-full flex-wrap items-center gap-1.5" role="group" aria-label="Filter by provider">
      {entries.map(([p, n]) => {
        const active = filter.provider === p;
        return (
          <button
            key={p}
            className={chipClass(active)}
            onClick={() => onFilter({ provider: active ? null : p })}
            aria-pressed={active}
          >
            <span>{n}</span>
            <span className="text-text-muted">{providerLabel(p)}</span>
          </button>
        );
      })}
    </div>
  );
}

/** The busiest personas, then how many more there are. */
export function PersonaChips({
  entries,
  rest,
  filter,
  onFilter,
}: CountProps & { /** Personas not shown. */ rest: number }) {
  return (
    <div className="flex max-w-full flex-wrap items-center gap-1.5" role="group" aria-label="Filter by persona">
      {entries.map(([p, n]) => {
        const active = filter.persona === p;
        return (
          <button
            key={p}
            className={chipClass(active, 'max-w-[180px]')}
            onClick={() => onFilter({ persona: active ? null : p })}
            aria-pressed={active}
            title={p}
          >
            <span>{n}</span>
            <span className="truncate text-text-muted">{p}</span>
          </button>
        );
      })}
      {rest > 0 && <span className="text-[12px] text-text-dim">+{rest} more</span>}
    </div>
  );
}
