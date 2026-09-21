/**
 * Submitting a switcher form: a command map from the open form to what it
 * does, wrapped in the shared busy and failure handling.
 */
import { createApiKey, importApiKey } from '@/lib/api';
import { busyRun, close, remove, show } from './actions';
import { call, message, projectIdFor, projectRequest } from './project-api';
import { loadProjects, openProject } from './session';
import type { JoinAnswer, PickerContext, Project, SubmitForm } from './types';

/** Renames the target, then returns to its options. */
async function rename(c: PickerContext) {
  const { target, name } = c.ui;
  if (!target) return;
  await projectRequest(c.session.token!, target.id, 'PATCH', { name: name.trim() });
  await loadProjects(c.session);
  c.patch({ target: { ...target, name: name.trim() } });
  c.session.toast('Project renamed', 'success');
  show(c, 'manage', { ...target, name: name.trim() });
}

/** Deletes the target and goes back to the list. */
async function destroy(c: PickerContext) {
  if (!c.ui.target) return;
  await remove(c, c.ui.target);
  show(c, null);
}

/** Creates a project and shows its new key. */
async function create(c: PickerContext) {
  const data = await createApiKey(c.session.token!, c.ui.name.trim() || undefined);
  // Show the key before anything else can fail, so it is never lost behind an error.
  c.patch({ name: '', copied: false, revealedKey: data.key, form: 'key' });
  await loadProjects(c.session);
  if (data.project) await openProject(c.session, data.project).catch((e) => c.session.toast(message(e), 'error'));
  c.session.toast('Project created. Copy your API key below.', 'success');
}

/** Restoring: a different key would silently add a second project instead of repairing this one. */
async function checkRestoreKey(target: Project | null, key: string) {
  if (!key) throw new Error('Paste the API key first');
  if (target && (await projectIdFor(key)) !== target.id)
    throw new Error(`That key doesn’t belong to ${target.name}. Paste the key it was created with.`);
}

/** Adds a project by its API key, or restores access to the target with the key it was created with. */
async function importKey(c: PickerContext) {
  const { target, secret, name } = c.ui;
  await checkRestoreKey(target, secret.trim());
  const data = await importApiKey(c.session.token!, secret.trim(), name.trim() || undefined);
  await loadProjects(c.session);
  if (data.project) await openProject(c.session, data.project);
  c.session.toast(target ? `Access to ${target.name} restored` : 'Project added', 'success');
  close(c);
}

/** The error when an invitation is refused without a reason. */
const JOIN_FAILED = 'That invitation could not be used';

/** Joins a project with an invitation code and opens it. */
async function join(c: PickerContext) {
  const init = { method: 'POST', body: JSON.stringify({ code: c.ui.secret.trim() }) };
  const data = await call<JoinAnswer>(c.session.token!, '/auth/projects/join', init, JOIN_FAILED);
  const list = await loadProjects(c.session);
  await openProject(c.session, data.project);
  c.session.toast(`Joined ${list.find((p) => p.id === data.project)?.name || 'project'}`, 'success');
  close(c);
}

/** What each form's submit button does. */
const SUBMITTERS: Record<SubmitForm, (c: PickerContext) => Promise<void>> = {
  rename,
  delete: destroy,
  new: create,
  import: importKey,
  join,
};

/** Submits the open form; a failure lands in the alert, with a repair path when there is one. */
export async function submit(c: PickerContext) {
  if (!c.session.token) return;
  const form = c.ui.form;
  await busyRun(c, c.ui.target, async () => {
    if (form && Object.hasOwn(SUBMITTERS, form)) await SUBMITTERS[form as SubmitForm](c);
  });
}
