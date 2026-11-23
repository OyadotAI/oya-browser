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
    <td className="px-2 py-3 text-right whitespace-nowrap max-md:space-x-2">
      <button
        className="btn-icon h-6 w-6 text-text-dim hover:text-text max-md:h-11 max-md:w-11"
        title="Connect (code, Playwright, MCP)"
        aria-label={`Connect to ${r.name}`}
        onClick={(e) => {
          e.stopPropagation();
          onConnect(r.id);
        }}
      >
        <Plug className="h-3 w-3 max-md:h-4 max-md:w-4" />
      </button>
      <button
        className="btn-icon h-6 w-6 text-text-dim hover:text-text max-md:h-11 max-md:w-11"
        title="Stop"
        aria-label={`Stop ${r.name}`}
        onClick={(e) => {
          e.stopPropagation();
          onStop([r.id]);
        }}
      >
        <Square className="h-3 w-3 max-md:h-4 max-md:w-4" />
      </button>
    </td>
  );
}
