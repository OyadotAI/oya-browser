/**
 * The dashboard's top bar: server health, the project switcher, downloads,
 * theme, settings and the account menu. Its pieces live in `header/`.
 */
'use client';

import { useState } from 'react';
import { Settings } from 'lucide-react';
import { OyaWordmark } from '@/components/oya-logo';
import ThemeToggle from '@/components/theme-toggle';
import ProjectSwitcher from './project-switcher';
import ProfileDialog from '@/components/dashboard/profile-dialog';
import HealthBadge from './header/health-badge';
import DownloadMenu from './header/download-menu';
import UserMenu from './header/user-menu';

/** What the page hands the header. */
interface HeaderProps {
  /** The console credential for the open project. */
  apiKey: string;
  /** Replaces the console credential when the project changes. */
  setApiKey: (credential: string, project: string | null) => void;
  /** Opens the settings dialog. */
  onOpenSettings: () => void;
}

/** The top bar. */
export default function Header({ apiKey, setApiKey, onOpenSettings }: HeaderProps) {
  const [showProfile, setShowProfile] = useState(false);
  return (
    // relative z-50 keeps the account menu and log out reachable. Selecting a
    // browser renders the detail panel as `fixed inset-0 z-40` below the lg
    // breakpoint, and an unpositioned header sits under it, the buttons were
    // still there, the overlay was just swallowing every click.
    <header className="relative z-50 flex flex-wrap items-center gap-3 px-4 py-2 lg:px-6 min-h-[52px] shrink-0 bg-bg border-b border-border">
      <OyaWordmark href="/dashboard" />
      <HealthBadge />
      <div className="flex-1" />
      <ProjectSwitcher apiKey={apiKey} setApiKey={setApiKey} />
      <DownloadMenu />
      <ThemeToggle />
      <button
        onClick={onOpenSettings}
        className="p-2 hover:bg-text/5 rounded-md text-text-dim hover:text-text transition-colors"
        title="Settings"
      >
        <Settings className="w-4 h-4" />
      </button>
      <UserMenu onOpenProfile={() => setShowProfile(true)} />
      <ProfileDialog open={showProfile} onClose={() => setShowProfile(false)} />
    </header>
  );
}
