/**
 * The edits the playbook table offers: promote a healed draft, delete a
 * playbook or its draft, and rename one. Each reports by toast and refreshes.
 */
import { useState } from 'react';
import { errorMessage } from '@/lib/api-client';
import { useToast } from '../toast';
import { deletePlaybook, promotePlaybook, renamePlaybook } from './api';
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

/** Promote, delete and rename, with the dialogs' state and a shared busy flag. */
export function usePlaybookActions(apiKey: string, refresh: () => void) {
  const toast = useToast();
  const [busy, setBusy] = useState(false);
  const ctx: ActionContext = { apiKey, refresh, toast, setBusy };
  const removal = useRemoval(ctx);
  const renaming = useRenaming(ctx);
  return { busy, promote: (name: string) => promote(ctx, name), ...removal, ...renaming };
}
