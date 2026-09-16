'use client';

import { useState, useEffect, useRef } from "react";
import { Download, Settings, LogOut, User } from 'lucide-react';
import { OyaWordmark } from '@/components/oya-logo';
import ThemeToggle from '@/components/theme-toggle';
import { useAuth } from '@/components/auth-provider';
import ProjectSwitcher from './project-switcher';
import ProfileDialog from '@/components/dashboard/profile-dialog';
import { apiUrl } from '@/lib/api';
import { browserDownloads } from '@/lib/browser-downloads';

interface HeaderProps {
  apiKey: string;
  setApiKey: (credential: string, project: string | null) => void;
  onOpenSettings: () => void;
}

export default function Header({ apiKey, setApiKey, onOpenSettings }: HeaderProps) {
  const { user, logout } = useAuth();

  const [healthStatus, setHealthStatus] = useState('connecting...');
  const [healthOk, setHealthOk] = useState(false);
  const [showUserMenu, setShowUserMenu] = useState(false);
  const [showProfile, setShowProfile] = useState(false);
  const userMenuRef = useRef<HTMLDivElement>(null);

  // Fetch health
  useEffect(() => {
    const fetchHealth = async () => {
      try {
        const res = await fetch(apiUrl('/health'));
        if (!res.ok) throw new Error();
        const data = await res.json();
        setHealthStatus(data.status === 'ok' ? 'healthy' : data.status);
        setHealthOk(data.status === 'ok');
      } catch {
        setHealthStatus('offline');
        setHealthOk(false);
      }
    };
    fetchHealth();
    const interval = setInterval(fetchHealth, 5000);
    return () => clearInterval(interval);
  }, []);

  // Click outside handler
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (userMenuRef.current && !userMenuRef.current.contains(e.target as Node)) {
        setShowUserMenu(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, []);

  return (
    // relative z-50 keeps the account menu and log out reachable. Selecting a
    // browser renders the detail panel as `fixed inset-0 z-40` below the lg
    // breakpoint, and an unpositioned header sits under it — the buttons were
    // still there, the overlay was just swallowing every click.
    <header className="relative z-50 flex flex-wrap items-center gap-3 px-4 py-2 lg:px-6 min-h-[52px] shrink-0 bg-bg border-b border-border">
      <OyaWordmark href="/dashboard" />

      {/* Health */}
      <div
        className={`inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium ${
          healthOk
            ? 'bg-accent/10 text-accent'
            : 'bg-red-500/10 text-red-400'
        }`}
      >
        <span className={`w-2 h-2 rounded-full mr-1.5 ${healthOk ? 'bg-accent' : 'bg-red-400'}`} />
        <span className="hidden sm:inline">{healthStatus}</span>
      </div>

      <div className="flex-1" />

      <ProjectSwitcher apiKey={apiKey} setApiKey={setApiKey} />
      <details className="relative shrink-0" onKeyDown={(event) => {
        if (event.key === 'Escape') {
          event.currentTarget.open = false;
          event.currentTarget.querySelector('summary')?.focus();
        }
      }} onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) event.currentTarget.open = false;
      }}>
        <summary className="btn-primary cursor-pointer list-none [&::-webkit-details-marker]:hidden" aria-label="Download Oya Browser">
          <Download className="h-4 w-4" aria-hidden="true" /><span>Download<span className="hidden md:inline"> browser</span></span>
        </summary>
        <nav aria-label="Browser downloads" className="absolute right-0 top-full z-50 mt-2 w-60 rounded-lg border border-border bg-bg-card p-2 shadow-lg">
          {browserDownloads.map(({ platform, architecture, href }) => (
            <a key={platform} href={href} download className="flex items-center justify-between gap-3 rounded-md px-3 py-2 text-sm text-text hover:bg-text/5">
              {platform}<span className="text-[11px] text-text-muted">{architecture}</span>
            </a>
          ))}
          <a href="/docs#download" className="mt-1 block border-t border-border px-3 pt-3 pb-1 text-xs text-text-muted hover:text-text">Installation instructions</a>
        </nav>
      </details>
      <ThemeToggle />

      {/* Settings */}
      <button
        onClick={onOpenSettings}
        className="p-2 hover:bg-text/5 rounded-md text-text-dim hover:text-text transition-colors"
        title="Settings"
      >
        <Settings className="w-4 h-4" />
      </button>

      {/* User Menu */}
      <div className="relative" ref={userMenuRef}>
        <button
          onClick={() => setShowUserMenu(!showUserMenu)}
          className="flex items-center p-1 hover:bg-text/5 rounded-md transition-colors"
        >
          <div className="w-7 h-7 rounded-full bg-indigo-500/10 flex items-center justify-center">
            <User className="w-4 h-4 text-indigo-400" />
          </div>
        </button>

        {showUserMenu && (
          <div className="absolute right-0 top-full mt-1.5 w-52 rounded-lg border border-border bg-bg-card shadow-xl shadow-black/40 z-50 overflow-hidden">
            <div className="px-3 py-2.5 border-b border-border">
              <div className="text-sm font-medium text-text">{user?.display_name || user?.email || 'User'}</div>
              <div className="text-xs text-text-dim">{user?.email}</div>
            </div>
            <div className="p-1">
              <button
                onClick={() => { setShowProfile(true); setShowUserMenu(false); }}
                className="w-full flex items-center gap-2 px-3 py-2 text-sm text-text hover:bg-text/5 rounded-md transition-colors"
              >
                <Settings className="w-4 h-4" />
                Profile settings
              </button>
              <button
                onClick={() => { logout(); setShowUserMenu(false); }}
                className="w-full flex items-center gap-2 px-3 py-2 text-sm text-red-400 hover:bg-red-500/10 rounded-md transition-colors"
              >
                <LogOut className="w-4 h-4" />
                Log out
              </button>
            </div>
          </div>
        )}
      </div>

      <ProfileDialog open={showProfile} onClose={() => setShowProfile(false)} />
    </header>
  );
}
