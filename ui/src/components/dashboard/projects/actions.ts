/**
 * What the switcher's buttons do: switch panels, open a project, copy a key,
 * close the popover, and show failures. Each takes the PickerContext of the
 * render that triggered it.
 */
import { forget, loadProjects, openAny, openProject } from './session';
import { message, projectRequest } from './project-api';
import type { Failure, Form, KeyAnswer, PickerContext, PickerState, Project } from './types';

/** A step run while the popover is busy. */
type Work = () => Promise<void>;
/** Changes to the popover's state. */
type Patch = Partial<PickerState>;

/** Show a failure; an unreadable key on a project you own comes with the way to repair it. */
export function fail(c: PickerContext, e: unknown, project: Project | null = null) {
  if ((e as Failure).code !== 'project_key_unavailable') return c.patch({ error: message(e), restore: null });
  c.patch({
    restore: project?.owner ? project : null,
    error: project?.owner
      ? `${project.name} was saved with a different server secret, so its key can’t be read here. Paste its original API key to restore access.`
      : 'This project’s key can’t be read on this server. Ask the project owner to restore access.',
  });
}

/** Opens a panel (null is the list) for `project`, clearing what the last one left behind. */
export function show(c: PickerContext, next: Form | null, project: Project | null = null) {
  const name = next === 'rename' ? project?.name || '' : '';
  c.patch({ form: next, target: project, name, secret: '', revealedKey: '', error: '', restore: null, copied: false });
}

/** Closes the popover, resets it and hands focus back to the switcher button. */
export function close(c: PickerContext) {
  c.patch({ open: false, form: null, target: null, revealedKey: '', secret: '', error: '', restore: null, query: '' });
  c.focusTrigger();
}

/** The switcher button: closes an open popover, or opens it on the project list. */
export function toggle(c: PickerContext) {
  if (c.ui.open) return close(c);
  show(c, null);
  c.patch({ query: '', open: true });
}

/** Runs `work` with the popover busy; a failure lands in the alert, about `project`. */
export async function busyRun(c: PickerContext, project: Project | null, work: Work, extra: Patch = {}) {
  c.patch({ busy: true, ...extra, error: '', restore: null });
  try {
    await work();
  } catch (e) {
    fail(c, e, project);
  } finally {
    c.patch({ busy: false, ...(extra.pending ? { pending: null } : {}) });
  }
}

/** Switches to `project`; choosing the open one just closes the popover. */
export function select(c: PickerContext, project: Project) {
  if (project.id === c.currentId) return close(c);
  const work = async () => {
    if (await openProject(c.session, project.id)) c.session.toast(`Switched to ${project.name}`, 'info');
    close(c);
  };
  return busyRun(c, project, work, { pending: project.id });
}

/** Copies `key`, or says to copy it by hand when the clipboard refuses. */
async function copyOrHint(c: PickerContext, key: string) {
  try {
    await navigator.clipboard.writeText(key);
    c.patch({ copied: true });
  } catch {
    c.session.toast('Select and copy the API key below.', 'info');
  }
}

/** Fetches an owned project's key, shows it, and copies it when the clipboard allows. */
export function copyKey(c: PickerContext, project: Project) {
  return busyRun(c, project, async () => {
    const data = await projectRequest<KeyAnswer>(c.session.token!, project.id, 'POST', {}, '/key');
    c.patch({ revealedKey: data.key, form: 'key' });
    await copyOrHint(c, data.key);
  });
}

/** Copies the key already on screen. */
export async function copyRevealed(c: PickerContext) {
  try {
    await navigator.clipboard.writeText(c.ui.revealedKey);
    c.patch({ copied: true });
  } catch {
    c.patch({ error: 'Select the key above and copy it manually.' });
  }
}

/** Deletes a project; if it was open, the console moves to another. */
export async function remove(c: PickerContext, project: Project) {
  // The confirmation already says running browsers stop with the project.
  await projectRequest(c.session.token!, project.id, 'DELETE', { stopBrowsers: true });
  c.patch({ revealedKey: '' });
  const wasOpen = c.currentId === project.id;
  if (wasOpen) forget(c.session);
  const list = await loadProjects(c.session);
  c.session.toast(`${project.name} deleted`, 'success');
  if (wasOpen) await openAny(c.session, list);
}
