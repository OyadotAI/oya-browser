/**
 * The edits the playbook table offers: promote a healed draft, delete a
 * playbook or its draft, and rename one. Each reports by toast and refreshes.
 */
import { useState } from 'react';
import { ApiError, errorMessage } from '@/lib/api-client';
import { Status } from '@/lib/http-status';
import { useToast } from '../toast';
import { deletePlaybook, exportPlaybook, importPlaybook, promotePlaybook, renamePlaybook } from './api';
import { downloadExport, readExport } from './transfer';
import { DRAFT_SUFFIX } from './constants';
import type { PlaybookInfo } from './types';

/** What every action needs. */
interface ActionContext {
  /** The caller's API key. */
  apiKey: string;
  /** Reloads the list after a change. */
  refresh: () => void;
  /** Reports the outcome. */
  toast: ReturnType<typeof useToast>;
  /** Marks a delete or rename in flight. */
  setBusy: (busy: boolean) => void;
}

/** Replaces a playbook's steps with its healed draft. */
async function promote(ctx: ActionContext, name: string) {
  try {
    await promotePlaybook(ctx.apiKey, name);
    ctx.toast(`${name} now uses the healed steps`, 'success');
    ctx.refresh();
  } catch (err) {
    ctx.toast(errorMessage(err), 'error');
  }
}

/** Runs `work` with the busy flag up; a failure is toasted. `after` runs last either way. */
async function withBusy(ctx: ActionContext, work: () => Promise<void>, after?: () => void) {
  ctx.setBusy(true);
  try {
    await work();
  } catch (err) {
    ctx.toast(errorMessage(err), 'error');
  } finally {
    settle(ctx, after);
  }
}

/** Drops the busy flag, then runs `after`. */
function settle(ctx: ActionContext, after?: () => void) {
  ctx.setBusy(false);
  after?.();
}

/** Deletes the playbook (or draft) awaiting confirmation, then clears the confirmation. */
function remove(ctx: ActionContext, name: string, done: () => void) {
  return withBusy(ctx, () => deleteAndReport(ctx, name), done);
}

/** Deletes a playbook or its draft, says which, and refreshes. */
async function deleteAndReport(ctx: ActionContext, name: string) {
  await deletePlaybook(ctx.apiKey, name);
  ctx.toast(name.endsWith(DRAFT_SUFFIX) ? 'Draft discarded' : `Deleted ${name}`, 'success');
  ctx.refresh();
}

/** Renames a playbook; the dialog closes only on success. */
function rename(ctx: ActionContext, from: string, name: string, done: () => void) {
  return withBusy(ctx, async () => {
    await renamePlaybook(ctx.apiKey, from, name);
    ctx.toast(`Renamed to ${name}`, 'success');
    done();
    ctx.refresh();
  });
}

/** Downloads a playbook's export. */
async function exportOne(ctx: ActionContext, name: string) {
  try {
    downloadExport(name, await exportPlaybook(ctx.apiKey, name));
  } catch (err) {
    ctx.toast(errorMessage(err), 'error');
  }
}

/** What an import that clashes with a name here should tell the person to do. */
const CLASH = 'A playbook with that name already exists here. Rename or delete it, then import again.';

/** Imports a chosen export file and refreshes; a name already taken is said plainly. */
function importFile(ctx: ActionContext, file: File) {
  return withBusy(ctx, async () => {
    const saved = await importPlaybook(ctx.apiKey, await readExport(file)).catch(explainClash);
    ctx.toast(`Imported ${saved.name}`, 'success');
    ctx.refresh();
  });
}

/** A clash on the name, said as what to do about it; any other failure as it came. */
function explainClash(err: unknown): never {
  throw err instanceof ApiError && err.status === Status.CONFLICT ? new Error(CLASH) : err;
}

/** Which playbook (or `name:draft`) is waiting for delete confirmation, and the delete. */
function useRemoval(ctx: ActionContext) {
  const [removing, setRemoving] = useState<string | null>(null);
  const confirmRemove = () => (removing ? remove(ctx, removing, () => setRemoving(null)) : undefined);
  return { removing, setRemoving, confirmRemove };
}

/** Which playbook the rename dialog is open for, and the rename. */
function useRenaming(ctx: ActionContext) {
  const [renaming, setRenaming] = useState<PlaybookInfo | null>(null);
  const applyRename = (name: string) =>
    renaming ? rename(ctx, renaming.name, name, () => setRenaming(null)) : undefined;
  return { renaming, setRenaming, applyRename };
}

/** Promote, delete, rename, export and import, with the dialogs' state and a shared busy flag. */
export function usePlaybookActions(apiKey: string, refresh: () => void) {
  const toast = useToast();
  const [busy, setBusy] = useState(false);
  const ctx: ActionContext = { apiKey, refresh, toast, setBusy };
  const removal = useRemoval(ctx);
  const renaming = useRenaming(ctx);
  const transfer = { exportOne: (name: string) => exportOne(ctx, name), importFile: (f: File) => importFile(ctx, f) };
  return { busy, promote: (name: string) => promote(ctx, name), ...transfer, ...removal, ...renaming };
}
