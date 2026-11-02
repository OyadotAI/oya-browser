/**
 * Which playbook a draft is on the server: the name it was saved as, but only on
 * the server and project it was saved to. The Record pane's "Rename and save"
 * and the save itself both ask this one rule, so the card never offers a rename
 * the save would not do, and a draft never renames another project's playbook
 * that happens to share its old name.
 */
import type { Draft } from '../../workflow/index.ts';

/** The fields that say where a draft was saved. */
type Publication = Pick<Draft, 'publishedName' | 'publishedTo'>;

/** The playbook name `draft` was saved as on `account`, or null when it was never saved there by name. */
export function savedAs(draft: Publication | undefined, account: string): string | null {
  const name = draft?.publishedName;
  return typeof name === 'string' && !!account && draft?.publishedTo === account ? name : null;
}

/** `next` with where `current` was saved: an undo or redo restores the edit, not the server's state. */
export function withPublication(next: Draft, current: Draft): Draft {
  if (next.id !== current.id) return next;
  return { ...next, publishedName: current.publishedName, publishedTo: current.publishedTo };
}
