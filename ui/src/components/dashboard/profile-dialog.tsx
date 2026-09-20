/**
 * The account dialog: who you are signed in as, and the one thing you can
 * change. Its state lives in `personas/use-profile-dialog.ts`.
 */
'use client';

import { LogOut, Loader2 } from 'lucide-react';
import Dialog from '@/components/ui/dialog';
import { DISPLAY_NAME_MAX_LENGTH } from './personas/constants';
import { memberSince } from './personas/model';
import { useProfileDialog, type ProfileState } from './personas/use-profile-dialog';

/** One read-only detail: a label and its value. */
function Row({ label, value }: { /** What the detail is. */ label: string; /** Its value. */ value: string }) {
  return (
    <div className="flex items-baseline justify-between gap-4">
      <span className="text-xs text-text-dim">{label}</span>
      <span className="truncate text-xs text-text">{value}</span>
    </div>
  );
}

/** What the dialog needs from the header that opens it. */
interface Props {
  /** Whether it is showing. */
  open: boolean;
  /** Closes it. */
  onClose: () => void;
}

/** Email, role and member-since: shown, never edited. */
function Details({ s }: { /** The dialog's state. */ s: ProfileState }) {
  const since = memberSince(s.user?.created_at);
  return (
    <div className="flex flex-col gap-2 rounded-md border border-border bg-bg-elevated/40 p-3">
      <Row label="Email" value={s.user?.email || '—'} />
      <Row label="Role" value={s.user?.role || 'member'} />
      {since && <Row label="Member since" value={since} />}
    </div>
  );
}

/**
 * Your account: who you are signed in as, and the one thing you can change.
 *
 * Email is the login and role is an authority grant, so both are shown but
 * neither is editable — a form that could raise its own role would be a
 * privilege escalation with a text input in front of it.
 */
export default function ProfileDialog({ open, onClose }: Props) {
  const s = useProfileDialog(open);
  return (
    <Dialog open={open} onClose={onClose} title="Your account" size="sm">
      <div className="flex flex-col gap-4">
        <label className="flex flex-col gap-1.5">
          <span className="text-xs font-medium text-text-muted">Display name</span>
          <input
            value={s.name}
            onChange={(e) => s.edit(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') void s.save();
            }}
            maxLength={DISPLAY_NAME_MAX_LENGTH}
            placeholder="Your name"
            className="h-9 rounded-md border border-border bg-bg px-3 text-sm text-text outline-none focus:border-accent"
          />
        </label>
        <Details s={s} />
        {s.error && <p className="text-xs text-red-400">{s.error}</p>}
        {s.saved && !s.error && <p className="text-xs text-emerald-400">Saved.</p>}
        <div className="flex items-center justify-between gap-3 pt-1">
          <button
            onClick={() => {
              s.logout();
              onClose();
            }}
            className="flex items-center gap-2 rounded-md px-3 py-2 text-sm text-red-400 transition-colors hover:bg-red-500/10"
          >
            <LogOut className="h-4 w-4" />
            Log out
          </button>
          <button
            onClick={() => void s.save()}
            disabled={!s.dirty || s.saving || !s.name.trim()}
            className="flex items-center gap-2 rounded-md bg-accent px-3 py-2 text-sm font-medium text-white transition-colors disabled:cursor-default disabled:opacity-40"
          >
            {s.saving && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
            Save
          </button>
        </div>
      </div>
    </Dialog>
  );
}
