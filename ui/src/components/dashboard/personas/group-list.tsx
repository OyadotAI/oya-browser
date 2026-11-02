/**
 * The persona groups table: each group's name, its member profiles, how many
 * browsers run across them, and Edit and Delete. The empty state says what a
 * group is for and how code uses one.
 */
'use client';

import { Layers, Pencil, Trash2 } from 'lucide-react';
import { groupStats, runningClass, runningLabel } from './group-stats';
import type { BrowserRow, Persona } from '../types';
import type { PersonaGroup } from './group-api';

/** Props for the list. */
interface Props {
  /** The groups. */
  groups: PersonaGroup[];
  /** The key's profiles, for names and counts. */
  personas: Persona[];
  /** Running browsers, for the counts. */
  browsers: BrowserRow[];
  /** Opens the editor on a group. */
  onEdit: (g: PersonaGroup) => void;
  /** Asks to delete a group. */
  onDelete: (g: PersonaGroup) => void;
}

/** How code starts a browser on a group. */
export function CodeLine({ name }: { /** The group shown in the snippet. */ name: string }) {
  return (
    <p className="text-[12px] text-text-muted">
      Start one with{' '}
      <span className="font-mono text-text-secondary">{`oya.browser.start({ personaGroup: '${name}' })`}</span>
    </p>
  );
}

/** No groups yet. */
export function EmptyGroups() {
  return (
    <div className="flex flex-col items-center gap-2 py-8 text-center">
      <div className="flex h-10 w-10 items-center justify-center rounded-xl border border-border bg-bg-card">
        <Layers className="h-4 w-4 text-text-muted" />
      </div>
      <p className="text-[14px] font-medium text-text">No groups yet</p>
      <p className="max-w-sm text-[12px] text-text-muted">
        A group shares browser starts between profiles, so no single profile runs too many at once.
      </p>
    </div>
  );
}

/** One group's row. */
function GroupRow({
  group,
  personas,
  browsers,
  onEdit,
  onDelete,
}: Omit<Props, 'groups'> & { /** The group. */ group: PersonaGroup }) {
  const s = groupStats(group, personas, browsers);
  return (
    <tr className="border-b border-border/60 last:border-0">
      <td className="max-w-0 px-3 py-2.5">
        <div className="truncate font-mono font-medium text-text" title={group.name}>
          {group.name}
        </div>
      </td>
      <td className="max-w-0 px-3 py-2.5">
        {s.members.length ? (
          <>
            <div className="num text-text-secondary">{s.members.length}</div>
            <div className="truncate text-[11px] text-text-dim" title={s.members.map((p) => p.name).join(', ')}>
              {s.members.map((p) => p.name).join(', ')}
            </div>
          </>
        ) : (
          <span className="text-yellow">No profiles</span>
        )}
      </td>
      <td className={`num px-3 py-2.5 text-right ${runningClass(s)}`}>{runningLabel(s)}</td>
      <td className="px-2 py-2.5">
        <div className="flex justify-end gap-1">
          <button className="btn-ghost h-7 w-7 p-0" aria-label={`Edit ${group.name}`} onClick={() => onEdit(group)}>
            <Pencil className="mx-auto h-3.5 w-3.5" />
          </button>
          <button className="btn-ghost h-7 w-7 p-0" aria-label={`Delete ${group.name}`} onClick={() => onDelete(group)}>
            <Trash2 className="mx-auto h-3.5 w-3.5" />
          </button>
        </div>
      </td>
    </tr>
  );
}

/** The groups as a table. */
export default function GroupList({ groups, ...rest }: Props) {
  return (
    <div className="overflow-x-auto rounded-xl border border-border">
      <table className="w-full table-fixed border-collapse text-[13px]" aria-label="Groups">
        <thead className="bg-bg-elevated">
          <tr className="border-b border-border text-left text-[11px] font-medium uppercase tracking-[0.1em] text-text-muted">
            <th className="w-[34%] px-3 py-2.5">Group</th>
            <th className="px-3 py-2.5">Profiles</th>
            <th className="w-[90px] px-3 py-2.5 text-right">Running</th>
            <th className="w-[80px] px-2 py-2.5" aria-label="Actions" />
          </tr>
        </thead>
        <tbody>
          {groups.map((g) => (
            <GroupRow key={g.name} group={g} {...rest} />
          ))}
        </tbody>
      </table>
    </div>
  );
}
