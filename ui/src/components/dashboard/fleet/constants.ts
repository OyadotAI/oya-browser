/**
 * Every number and fixed table the fleet strip and fleet table run on, by name.
 */
import type { Health } from '../types';

/** Columns the fleet table can be sorted by. */
export type SortKey =
  'name' | 'health' | 'persona' | 'provider' | 'currentUrl' | 'commands' | 'errors' | 'lastSeen' | 'connectedAt';

/** One sortable column header: which field, its heading, and its width and alignment. */
export interface Column {
  /** Field the column sorts by. */
  key: SortKey;
  /** Heading text; empty for the health dot column. */
  label: string;
  /** Width and alignment classes. */
  className: string;
}

/** Order the health chips appear in, healthy first. */
export const HEALTH_ORDER: Health[] = ['ok', 'stale', 'errors', 'dead'];
/** Sort rank per health: the browsers that need attention come first. */
export const HEALTH_RANK: Record<Health, number> = { errors: 0, dead: 1, stale: 2, ok: 3 };
/** Rows painted per page; a thousand rows at once would stall the table. */
export const PAGE_SIZE = 500;
/** Personas given their own chip; the rest are summed as "+N more". */
export const TOP_PERSONAS = 5;
/** An error rate at or above this percentage is shown in red. */
export const ERROR_RATE_ALERT_PCT = 5;
/** Decimal places the error rate is shown with. */
export const ERROR_RATE_DECIMALS = 1;
/** Columns the "no match" row spans: checkbox, the sortable columns and the actions. */
export const TABLE_COL_SPAN = 10;

/** The sortable columns, in display order. */
export const COLUMNS: Column[] = [
  { key: 'health', label: '', className: 'w-6' },
  { key: 'name', label: 'Browser', className: 'w-[22%]' },
  { key: 'persona', label: 'Persona', className: 'w-[140px]' },
  { key: 'provider', label: 'Provider', className: 'w-[120px]' },
  { key: 'currentUrl', label: 'Current page', className: '' },
  { key: 'commands', label: 'Cmds · Err', className: 'w-[100px] text-right' },
  { key: 'lastSeen', label: 'Seen', className: 'w-[64px] text-right' },
  { key: 'connectedAt', label: 'Up', className: 'w-[72px] text-right' },
];

/** The filter with everything cleared. */
export const NO_FILTER = { health: null, provider: null, persona: null, text: '' } as const;
