/**
 * Derived text for the switcher: panel titles, submit labels, the trigger
 * label and the filtered project list.
 */
import type { Form, PickerContext, PickerState, SubmitForm } from './types';

/** The import form with a target restores that project's access rather than adding a new one. */
export const isRestoring = (ui: PickerState) => ui.form === 'import' && !!ui.target;

/** Each panel's title, as a function of the popover's state. */
const TITLES: Record<Form, (ui: PickerState) => string> = {
  new: () => 'Create a project',
  join: () => 'Join a project',
  import: (ui) => (isRestoring(ui) ? 'Restore access' : 'Add an API key'),
  rename: () => 'Rename project',
  delete: () => 'Delete project',
  manage: (ui) => ui.target?.name || 'Project options',
  key: () => 'Your API key',
};

/** The heading and dialog label: the panel's title, or "Projects" for the list. */
export const titleOf = (ui: PickerState) =>
  ui.form && Object.hasOwn(TITLES, ui.form) ? TITLES[ui.form](ui) : 'Projects';

/** Each form's submit button. */
const SUBMIT_LABELS: Record<SubmitForm, string> = {
  rename: 'Save name',
  delete: 'Delete project',
  new: 'Create project',
  join: 'Join project',
  import: 'Add project',
};

/** The submit button's text; restoring overrides the import label. */
export const submitLabel = (ui: PickerState) =>
  isRestoring(ui) ? 'Restore access' : SUBMIT_LABELS[ui.form as SubmitForm];

/** What the switcher button says: the open project, or a prompt to pick one. */
export const triggerLabel = (c: PickerContext, apiKey: string) =>
  c.projects.find((p) => p.id === c.currentId)?.name || (apiKey ? 'Project' : 'Select project');

/** Projects matching the search, split into yours and shared ones. */
export function matchingProjects(c: PickerContext) {
  const matches = c.projects.filter((p) => p.name.toLowerCase().includes(c.ui.query.trim().toLowerCase()));
  return { matches, owned: matches.filter((p) => p.owner), shared: matches.filter((p) => !p.owner) };
}
