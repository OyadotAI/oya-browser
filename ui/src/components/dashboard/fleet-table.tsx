/**
 * The fleet table: every browser, filtered, sorted and selectable. The parts
 * (toolbar, head, rows, row menu) and their state live in `fleet/`.
 */
'use client';

import type { RefObject } from 'react';
import ContextMenu from '@/components/ui/context-menu';
import type { BrowserRow } from './types';
import type { FleetFilter } from './fleet-strip';
import Toolbar from './fleet/toolbar';
import FleetTableView from './fleet/table-view';
import { rowMenuItems } from './fleet/row-menu';
import { useRowMenu } from './fleet/use-row-menu';
import { useFleetTable } from './fleet/use-fleet-table';

/** What the page hands the table. */
export interface FleetTableProps {
  /** Every browser. */
  rows: BrowserRow[];
  /** Browser open in the side panel. */
  selectedId: string | null;
  /** Selects a browser, or clears the selection with null. */
  onSelect: (id: string | null) => void;
  /** Ticked browser ids. */
  checked: Set<string>;
  /** Replaces the ticked set. */
  onChecked: (next: Set<string>) => void;
  /** Current filter, shared with the fleet strip. */
  filter: FleetFilter;
  /** Merges a change into the filter. */
  onFilter: (next: Partial<FleetFilter>) => void;
  /** Stops the given browsers. */
  onStop: (ids: string[]) => void;
  /** Opens the start-browser dialog. */
  onStart: () => void;
  /** Opens the connect dialog for a browser. */
  onConnect: (id: string) => void;
  /** Takes a screenshot of a browser. */
  onScreenshot: (id: string) => void;
  /** Opens the fleet's code snippets. */
  onCode: () => void;
  /** The project key. */
  apiKey: string;
  /** Lets the page's `/` shortcut focus the filter. */
  filterRef: RefObject<HTMLInputElement | null>;
  /** Clock the relative times are measured from. */
  now: number;
}

/**
 * The fleet. Dense on purpose: a thousand rows should fit the eye, not the
 * scroll bar. Rows paint lazily; the selection is keyboard-driven.
 */
export default function FleetTable(props: FleetTableProps) {
  const { rows, filter, onFilter, filterRef, checked, onStop, onStart, onCode, apiKey } = props;
  const { menu, openAt, close } = useRowMenu();
  const view = useFleetTable(props);
  return (
    <div className="workspace-list flex h-full min-h-0 min-w-0 flex-col">
      <Toolbar
        {...{ filter, onFilter, filterRef, rows, checked, onStop, onStart, onCode, apiKey }}
        visibleCount={view.visible.length}
      />
      <ContextMenu
        at={menu?.at ?? null}
        items={menu ? rowMenuItems(menu.row, props) : []}
        onClose={close}
        label={menu ? `Actions for ${menu.row.name}` : 'Actions'}
      />
      <FleetTableView {...props} view={view} onMenu={openAt} />
    </div>
  );
}
