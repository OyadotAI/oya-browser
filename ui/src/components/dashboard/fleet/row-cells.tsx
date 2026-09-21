/**
 * The data cells of one fleet row, from the health dot to the uptime.
 */
import { ago, shortId } from '@/lib/api-client';
import type { BrowserRow } from '../types';
import { providerLabel } from '../types';

/** The row and what clicking its name does. */
interface Props {
  /** The browser shown. */
  r: BrowserRow;
  /** Toggles this row's selection. */
  onPick: () => void;
  /** Clock the relative times are measured from. */
  now: number;
}

/** Health, name and id, persona, provider, page, counters, seen and up. */
export default function RowCells({ r, onPick, now }: Props) {
  return (
    <>
      <td className="px-2 py-3">
        <span className={`dot dot-${r.health}`} title={r.health} />
      </td>
      <td className="px-2 py-3">
        <button
          className="block w-full truncate text-left font-medium text-text outline-offset-4 hover:text-accent"
          onClick={(e) => {
            e.stopPropagation();
            onPick();
          }}
          title={r.name}
        >
          {r.name}
        </button>
        <div className="truncate font-mono text-[11px] text-text-dim">{shortId(r.id)}</div>
      </td>
      <td className="truncate px-2 py-3 text-text-secondary" title={r.persona || ''}>
        {r.personaName || (r.persona ? shortId(r.persona) : '—')}
      </td>
      <td className="px-2 py-3 text-text-secondary">{providerLabel(r.provider)}</td>
      <td className="truncate px-2 py-3 font-mono text-[12px] text-text-secondary" title={r.currentUrl}>
        {r.currentUrl ? r.currentUrl.replace(/^https?:\/\//, '') : <span className="text-text-dim">—</span>}
      </td>
      <td className="px-2 py-3 text-right num text-text-secondary">
        {r.commands} · <span className={r.errors ? 'text-red' : ''}>{r.errors}</span>
        {r.pending > 0 && <span className="ml-1 text-yellow">+{r.pending}</span>}
      </td>
      <td className="px-2 py-3 text-right num text-text-muted">{ago(r.lastSeen, now)}</td>
      <td className="px-2 py-3 text-right num text-text-muted">{ago(r.connectedAt, now)}</td>
    </>
  );
}
