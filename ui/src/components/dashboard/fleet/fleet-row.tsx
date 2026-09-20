/**
 * One browser in the fleet table. Clicking selects it; right-click opens its menu.
 */
import type { MouseEvent } from 'react';
import type { BrowserRow } from '../types';
import RowCells from './row-cells';
import RowButtons from './row-buttons';

/** The row, its state, and its callbacks. */
export interface FleetRowProps {
  /** The browser shown. */
  r: BrowserRow;
  /** Whether it is open in the side panel. */
  selected: boolean;
  /** Whether its checkbox is ticked. */
  checked: boolean;
  /** Selects a browser, or clears the selection with null. */
  onSelect: (id: string | null) => void;
  /** Ticks or unticks its checkbox. */
  onToggle: (id: string) => void;
  /** Opens the right-click menu for it. */
  onMenu: (e: MouseEvent, r: BrowserRow) => void;
  /** Opens the connect dialog. */
  onConnect: (id: string) => void;
  /** Stops the given browsers. */
  onStop: (ids: string[]) => void;
  /** Clock the relative times are measured from. */
  now: number;
}

/** Checkbox, data cells, then the row's buttons. */
export default function FleetRow({
  r,
  selected,
  checked,
  onSelect,
  onToggle,
  onMenu,
  onConnect,
  onStop,
  now,
}: FleetRowProps) {
  const pick = () => onSelect(selected ? null : r.id);
  return (
    <tr
      data-id={r.id}
      onClick={pick}
      onContextMenu={(e) => onMenu(e, r)}
      className={`group cursor-pointer border-b border-border/60 transition-colors ${
        selected ? 'bg-accent/[0.08]' : 'hover:bg-text/[0.035]'
      }`}
      aria-selected={selected}
    >
      <td className="px-2 py-3" onClick={(e) => e.stopPropagation()}>
        <input
          type="checkbox"
          checked={checked}
          onChange={() => onToggle(r.id)}
          aria-label={`Select ${r.name}`}
          className="accent-accent"
        />
      </td>
      <RowCells r={r} onPick={pick} now={now} />
      <RowButtons r={r} onConnect={onConnect} onStop={onStop} />
    </tr>
  );
}
