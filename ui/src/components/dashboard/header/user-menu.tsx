/**
 * The account button in the header and its menu: profile settings, the admin
 * page for Oya staff, and log out.
 */
'use client';

import { useCallback, useRef, useState, type ReactNode } from 'react';
import Link from 'next/link';
import { CreditCard, LogOut, Settings, Shield, User } from 'lucide-react';
import { BILLING_PAGE } from '../billing/constants';
import { ADMIN_DOMAIN } from '@/components/admin/constants';
import { useAuth } from '@/components/auth-provider';
import { useOutsideClick } from '../hooks/use-outside-click';

/** What the menu opens. */
interface Props {
  /** Opens the profile dialog. */
  onOpenProfile: () => void;
}

/** Whether the signed-in address is Oya staff's; the server still decides who sees the admin page. */
const isStaff = (email?: string) =>
  String(email || '')
    .toLowerCase()
    .endsWith(ADMIN_DOMAIN);

/** A menu entry that goes to another page, closing the menu on the way. */
function MenuLink(p: {
  /** Where. */ href: string;
  /** Its icon. */ icon: ReactNode;
  /** Its text. */ label: string;
  /** Closes the menu. */ onClose: () => void;
}) {
  return (
    <Link
      href={p.href}
      onClick={p.onClose}
      className="w-full flex items-center gap-2 px-3 py-2 text-sm text-text hover:bg-text/5 rounded-md transition-colors"
    >
      {p.icon}
      {p.label}
    </Link>
  );
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
        {user && (
          <MenuLink
            href={BILLING_PAGE}
            icon={<CreditCard className="w-4 h-4" />}
            label="Plan & billing"
            onClose={onClose}
          />
        )}
        {isStaff(user?.email) && (
          <MenuLink href="/admin" icon={<Shield className="w-4 h-4" />} label="Admin" onClose={onClose} />
        )}
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
