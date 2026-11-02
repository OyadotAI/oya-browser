/**
 * How a persona group reads in the console: its member profiles, and how many
 * browsers run across them against the sum of their caps. Worked out from the
 * profile list the tab already has, so the dialog needs no extra request.
 */
import { rowStats } from './model';
import type { BrowserRow, Persona } from '../types';
import type { PersonaGroup } from './group-api';

/** A group's members, running count, total cap and whether every member is full. */
export interface GroupStats {
  /** The member profiles that are in the list. */
  members: Persona[];
  /** Browsers running across them. */
  running: number;
  /** The sum of their caps; Infinity when any is uncapped. */
  cap: number;
  /** Whether every member is at its cap. */
  full: boolean;
}

/** The stats for one group. */
export function groupStats(group: PersonaGroup, personas: Persona[], browsers: BrowserRow[]): GroupStats {
  const members = personas.filter((p) => group.personaIds.includes(p.id));
  return { members, ...totals(members.map((p) => rowStats(p, browsers))) };
}

/** Running and cap summed over the members' stats, and whether all of them are full. */
function totals(stats: ReturnType<typeof rowStats>[]) {
  const running = stats.reduce((n, s) => n + s.running, 0);
  const cap = stats.reduce((n, s) => n + s.cap, 0);
  return { running, cap, full: stats.length > 0 && stats.every((s) => s.at) };
}

/** "3/4", with ∞ for an uncapped total. */
export const runningLabel = (s: Pick<GroupStats, 'running' | 'cap'>) =>
  `${s.running}/${Number.isFinite(s.cap) ? s.cap : '∞'}`;

/** The colour of a running count: yellow when full, accent when anything runs, else muted. */
export const runningClass = (s: Pick<GroupStats, 'running' | 'full'>) =>
  s.full ? 'text-yellow' : s.running > 0 ? 'text-accent' : 'text-text-muted';

/** The profiles a group may hold: every one but the default, by name. */
export const eligible = (personas: Persona[]) =>
  personas.filter((p) => !p.isDefault).sort((a, b) => a.name.localeCompare(b.name));
