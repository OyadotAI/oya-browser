/**
 * The docs page's fixed top bar: wordmark, theme toggle, console link and the
 * mobile menu button.
 */
'use client';

import Link from 'next/link';
import { ArrowUpRight, Menu } from 'lucide-react';
import { OyaWordmark } from '@/components/oya-logo';
import ThemeToggle from '@/components/theme-toggle';

/** DocsHeader's props. */
interface Props {
  /** Opens the mobile sections menu. */
  onOpenMenu: () => void;
}

/** The top bar. */
export function DocsHeader({ onOpenMenu }: Props) {
  return (
    <header className="fixed inset-x-0 top-0 z-40 flex h-[72px] items-center justify-between gap-4 border-b border-border bg-bg/95 px-4 backdrop-blur-md lg:px-8">
      <div className="flex items-center gap-5">
        <OyaWordmark />
        <span className="hidden border-l border-border pl-5 text-[13px] text-text-muted sm:block">Documentation</span>
      </div>
      <div className="flex items-center gap-3">
        <ThemeToggle />
        <Link href="/dashboard" className="btn-ghost h-9 text-[12px]">
          Open console <ArrowUpRight size={14} />
        </Link>
        <button className="btn-icon lg:hidden" onClick={onOpenMenu} aria-label="Open documentation menu">
          <Menu size={19} />
        </button>
      </div>
    </header>
  );
}
