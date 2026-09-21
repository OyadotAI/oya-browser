/**
 * The switcher's home panel: search, your projects, shared ones, and the
 * create, join and add-key actions.
 */
'use client';

import { ChevronRight, Folder, Import, LogIn, Plus, Search } from 'lucide-react';
import { show } from './actions';
import { ACTION_CLASS, FIELD_CLASS } from './constants';
import ProjectRow from './project-row';
import type { PanelProps, PickerState, Project } from './types';

/** A group's heading and its projects. */
interface GroupProps extends PanelProps {
  /** The heading. */
  label: string;
  /** The projects in it. */
  projects: Project[];
}
import { matchingProjects } from './view';

/** A labelled group of rows; nothing when the group is empty. */
function Group({ c, label, projects }: GroupProps) {
  if (!projects.length) return null;
  return (
    <>
      <p className="px-3 pb-1 pt-3 text-[10px] font-medium uppercase tracking-[0.12em] text-text-dim">{label}</p>
      {projects.map((p) => (
        <ProjectRow key={p.id} c={c} project={p} />
      ))}
    </>
  );
}

/** What shows when nothing matches: a hint to search again, or to create the first project. */
function Empty({ query }: Pick<PickerState, 'query'>) {
  return (
    <div className="px-4 py-8 text-center">
      <Folder className="mx-auto mb-3 h-6 w-6 text-text-dim" />
      <p className="text-sm text-text-muted">{query ? 'No matching projects' : 'Your first project starts here'}</p>
      <p className="mt-1 text-xs text-text-dim">
        {query ? 'Try another name.' : 'Create a project to connect your browsers.'}
      </p>
    </div>
  );
}

/** The create, join and add-key buttons under the list. */
function Actions({ c }: PanelProps) {
  const busy = c.ui.busy;
  return (
    <div className="border-t border-border p-2">
      <button disabled={busy} onClick={() => show(c, 'new')} className={`${ACTION_CLASS} font-medium text-accent`}>
        <Plus className="h-4 w-4" />
        Create project
        <ChevronRight className="ml-auto h-3.5 w-3.5 opacity-50" />
      </button>
      <div className="flex gap-1 px-1 pb-1">
        <button
          disabled={busy}
          onClick={() => show(c, 'join')}
          className="flex flex-1 items-center justify-center gap-2 rounded-md py-2 text-xs text-text-muted hover:bg-text/5"
        >
          <LogIn className="h-3.5 w-3.5" />
          Join with invite
        </button>
        <span className="my-2 w-px bg-border" />
        <button
          disabled={busy}
          onClick={() => show(c, 'import')}
          className="flex flex-1 items-center justify-center gap-2 rounded-md py-2 text-xs text-text-muted hover:bg-text/5"
        >
          <Import className="h-3.5 w-3.5" />
          Add API key
        </button>
      </div>
    </div>
  );
}

/** Search, grouped projects and actions. */
export default function ProjectList({ c }: PanelProps) {
  const { matches, owned, shared } = matchingProjects(c);
  return (
    <>
      <div className="relative mx-3 mb-1 mt-3">
        <Search className="pointer-events-none absolute left-3 top-2.5 h-4 w-4 text-text-dim" />
        <input
          autoFocus
          aria-label="Search projects"
          placeholder="Find a project…"
          value={c.ui.query}
          onChange={(e) => c.patch({ query: e.target.value })}
          className={`${FIELD_CLASS} border-transparent bg-text/4 pl-9`}
        />
      </div>
      <div className="max-h-[min(340px,45dvh)] overflow-y-auto px-2 pb-2">
        <Group c={c} label="Your projects" projects={owned} />
        <Group c={c} label="Shared with you" projects={shared} />
        {!matches.length && <Empty query={c.ui.query} />}
      </div>
      <Actions c={c} />
    </>
  );
}
