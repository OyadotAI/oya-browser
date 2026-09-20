/**
 * The last cell of a fleet row: connect and stop, without selecting the row.
 */
import { Plug, Square } from 'lucide-react';
import type { BrowserRow } from '../types';

/** The row and its two actions. */
interface Props {
  /** The browser acted on. */
  r: BrowserRow;
  /** Opens the connect dialog. */
  onConnect: (id: string) => void;
  /** Stops the given browsers. */
  onStop: (ids: string[]) => void;
}

/** Connect and Stop icon buttons. */
export default function RowButtons({ r, onConnect, onStop }: Props) {
  return (
    <td className="px-2 py-3 text-right whitespace-nowrap">
      <button
        className="btn-icon h-6 w-6 text-text-dim hover:text-text"
        title="Connect (code, Playwright, MCP)"
        aria-label={`Connect to ${r.name}`}
        onClick={(e) => {
          e.stopPropagation();
          onConnect(r.id);
        }}
      >
        <Plug className="h-3 w-3" />
      </button>
      <button
        className="btn-icon h-6 w-6 text-text-dim hover:text-text"
        title="Stop"
        aria-label={`Stop ${r.name}`}
        onClick={(e) => {
          e.stopPropagation();
          onStop([r.id]);
        }}
      >
        <Square className="h-3 w-3" />
      </button>
    </td>
  );
}
