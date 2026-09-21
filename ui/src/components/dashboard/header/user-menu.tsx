/**
 * The account button in the header and its menu: profile settings and log out.
 */
'use client';

import { useCallback, useRef, useState } from 'react';
import { LogOut, Settings, User } from 'lucide-react';
import { useAuth } from '@/components/auth-provider';
import { useOutsideClick } from '../hooks/use-outside-click';

/** What the menu opens. */
interface Props {
  /** Opens the profile dialog. */
  onOpenProfile: () => void;
}

/** The open menu: who is signed in, then the actions. Each action closes the menu. */
function AccountMenu({ onOpenProfile, onClose }: Props & { /** Closes the menu. */ onClose: () => void }) {
  const { user, logout } = useAuth();
  return (
    <div className="absolute right-0 top-full mt-1.5 w-52 rounded-lg border border-border bg-bg-card shadow-xl shadow-black/40 z-50 overflow-hidden">
      <div className="px-3 py-2.5 border-b border-border">
        <div className="text-sm font-medium text-text">{user?.display_name || user?.email || 'User'}</div>
        <div className="text-xs text-text-dim">{user?.email}</div>
      </div>
      <div className="p-1">
        <button
          onClick={() => {
            onOpenProfile();
            onClose();
          }}
          className="w-full flex items-center gap-2 px-3 py-2 text-sm text-text hover:bg-text/5 rounded-md transition-colors"
        >
          <Settings className="w-4 h-4" />
          Profile settings
        </button>
        <button
          onClick={() => {
            logout();
            onClose();
          }}
          className="w-full flex items-center gap-2 px-3 py-2 text-sm text-red-400 hover:bg-red-500/10 rounded-md transition-colors"
        >
          <LogOut className="w-4 h-4" />
          Log out
        </button>
      </div>
    </div>
  );
}

/** The avatar button; a click outside the menu closes it. */
export default function UserMenu({ onOpenProfile }: Props) {
  const [show, setShow] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const hide = useCallback(() => setShow(false), []);
  useOutsideClick(ref, hide);
  return (
    <div className="relative" ref={ref}>
      <button
        onClick={() => setShow(!show)}
        className="flex items-center p-1 hover:bg-text/5 rounded-md transition-colors"
      >
        <div className="w-7 h-7 rounded-full bg-indigo-500/10 flex items-center justify-center">
          <User className="w-4 h-4 text-indigo-400" />
        </div>
      </button>
      {show && <AccountMenu onOpenProfile={onOpenProfile} onClose={hide} />}
    </div>
  );
}
