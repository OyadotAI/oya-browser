/**
 * The workspace panel's panes.
 */

/** Every pane, by name: Ask, Routines, Record (the studio) and the Inspect tools. */
export const PANES = ['chat', 'playbooks', 'routines', 'record', 'actions', 'network', 'source'] as const;

/** A pane's name. */
export type Pane = (typeof PANES)[number];

/** Panes grouped under the Inspect tab. */
export const INSPECT_PANES: readonly Pane[] = ['actions', 'network', 'source'];
