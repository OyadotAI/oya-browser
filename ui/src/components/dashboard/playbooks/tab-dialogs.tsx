/**
 * The tab's two simple overlays: a playbook's Playwright code, and the delete
 * (or discard-draft) confirmation.
 */
'use client';

import Dialog, { Confirm } from '@/components/ui/dialog';
import SyntaxCode from '@/components/ui/syntax-code';
import { DRAFT_SUFFIX } from './constants';
import type { PlaybookBody } from './types';

/** Props for the code dialog. */
interface CodeProps {
  /** The playbook or draft shown; null when closed. */
  code: PlaybookBody | null;
  /** Closes it. */
  onClose: () => void;
}

/** The Playwright module for a playbook or its draft; closed while `code` is null. */
export function CodeDialog({ code, onClose }: CodeProps) {
  return (
    <Dialog
      open={!!code}
      onClose={onClose}
      size="lg"
      title={code?.name}
      description="The same flow as a Playwright module. Pass the variables as vars."
    >
      {code && (
        <pre className="max-h-[60vh] overflow-auto rounded-lg border border-border bg-bg-card px-4 py-3 text-[12px]">
          <SyntaxCode code={code.code} language="typescript" />
        </pre>
      )}
    </Dialog>
  );
}

/** Props for the delete confirmation. */
interface RemoveProps {
  /** The playbook, or `name:draft`, awaiting confirmation; null when closed. */
  removing: string | null;
  /** A delete is in flight. */
  busy: boolean;
  /** Cancels. */
  onClose: () => void;
  /** Deletes. */
  onConfirm: () => void;
}

/** Confirms deleting a playbook, or discarding only its healed draft. */
export function RemoveConfirm({ removing, busy, onClose, onConfirm }: RemoveProps) {
  const draft = removing?.endsWith(DRAFT_SUFFIX);
  return (
    <Confirm
      open={!!removing}
      onClose={onClose}
      onConfirm={onConfirm}
      danger
      busy={busy}
      title={draft ? 'Discard the healed draft?' : `Delete ${removing}?`}
      confirmLabel={draft ? 'Discard' : 'Delete'}
      body={
        draft
          ? 'The playbook keeps its current steps. The next replay that breaks will heal again.'
          : 'The playbook and any healed draft are gone. Code calling play() with this name will fail.'
      }
    />
  );
}
