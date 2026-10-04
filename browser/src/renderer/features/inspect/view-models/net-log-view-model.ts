/**
 * The Activity pane: every message between this browser and the server, with
 * direction and type filters, the newest few hundred kept. Rows open and close
 * on a click, except a click that only ended a text selection.
 */
import { ViewModel } from '../../../core/view-model.ts';
import { RendererConstants as C } from '../../../core/constants.ts';
import type { Payload } from '../../../../shared/ipc.ts';
import { DEFAULT_ENTRY_TYPE, DIRECTIONS, NET_FILTERS } from '../model/constants.ts';
import type { InspectServices } from '../model/services.ts';

/** One message in the log. */
export interface LogEntry {
  /** Its key in the list. */
  key: number;
  /** When, in epoch milliseconds. */
  ts: number;
  /** 'in' (from the server) or 'out'. */
  dir: string;
  /** Its message type ("cmd: click", "auth"). */
  type: string;
  /** Its body. */
  data: string;
}

/** What the Activity pane shows. */
export interface NetLogState {
  /** The entries kept, oldest first. */
  entries: readonly LogEntry[];
  /** The active filter. */
  filter: string;
  /** Keys of the open rows. */
  expanded: readonly number[];
}

/** Whether `filter` shows `entry` (an unknown filter shows everything). */
export const shows = (filter: string, entry: Pick<LogEntry, 'dir' | 'type'>): boolean =>
  !Object.hasOwn(NET_FILTERS, filter) || NET_FILTERS[filter](entry);

/** How a direction reads ("From server", "To server"). */
export const directionLabel = (dir: string): string => (dir === 'in' ? DIRECTIONS.in : DIRECTIONS.out);

/** `n` zero-padded to `width` digits. */
const pad = (n: number, width: number = C.CLOCK_DIGITS): string => String(n).padStart(width, '0');

/** A timestamp as HH:MM:SS.mmm, local time. */
export function fmtTime(ts: number): string {
  const d = new Date(ts);
  const clock = [d.getHours(), d.getMinutes(), d.getSeconds()].map((n) => pad(n)).join(':');
  return `${clock}.${pad(d.getMilliseconds(), C.MS_DIGITS)}`;
}

/** The activity log. */
export class NetLogViewModel extends ViewModel<NetLogState> {
  /** The next entry's key. */
  private nextKey = 1;

  /** Empty, showing everything, hearing every message; registered as the pane's Clear. */
  constructor(services: Pick<InspectServices, 'bridge' | 'panel'>) {
    super({ entries: [], filter: 'all', expanded: [] });
    this.own(services.bridge.onDevLog((raw) => this.add(raw)));
    this.own(services.panel.onClear('network', () => this.clear()));
  }

  /** Adds an entry, dropping the oldest past the log's limit. */
  add(raw: Payload | null | undefined): void {
    const entry = {
      key: this.nextKey++,
      ts: Number(raw?.ts) || 0,
      dir: String(raw?.dir ?? ''),
      type: String(raw?.type || DEFAULT_ENTRY_TYPE),
      data: String(raw?.data || ''),
    };
    this.set({ entries: [...this.state.entries, entry].slice(-C.NET_LOG_LIMIT) });
  }

  /** Switches the filter. */
  setFilter(filter: string): void {
    this.set({ filter });
  }

  /** Opens or closes row `key`, unless the click only ended the text selection `selection`. */
  toggleRow(key: number, selection = ''): void {
    if (selection) return;
    const { expanded } = this.state;
    this.set({ expanded: expanded.includes(key) ? expanded.filter((k) => k !== key) : [...expanded, key] });
  }

  /** Empties the log. */
  clear(): void {
    this.set({ entries: [], expanded: [] });
  }
}
