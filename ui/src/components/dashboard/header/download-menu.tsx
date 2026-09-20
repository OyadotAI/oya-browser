/**
 * The "Download browser" menu: one link per platform, built on <details> so it
 * needs no open state of its own.
 */
'use client';

import type { FocusEvent, KeyboardEvent } from 'react';
import { Download } from 'lucide-react';
import { browserDownloads } from '@/lib/browser-downloads';

/** Escape closes the menu and puts focus back on its button. */
function closeOnEscape(event: KeyboardEvent<HTMLDetailsElement>) {
  if (event.key !== 'Escape') return;
  event.currentTarget.open = false;
  event.currentTarget.querySelector('summary')?.focus();
}

/** Focus leaving the menu closes it. */
function closeOnBlur(event: FocusEvent<HTMLDetailsElement>) {
  if (!event.currentTarget.contains(event.relatedTarget)) event.currentTarget.open = false;
}

/** The download button and its platform list. */
export default function DownloadMenu() {
  return (
    <details className="relative shrink-0" onKeyDown={closeOnEscape} onBlur={closeOnBlur}>
      <summary
        className="btn-primary cursor-pointer list-none [&::-webkit-details-marker]:hidden"
        aria-label="Download Oya Browser"
      >
        <Download className="h-4 w-4" aria-hidden="true" />
        <span>
          Download<span className="hidden md:inline"> browser</span>
        </span>
      </summary>
      <nav
        aria-label="Browser downloads"
        className="absolute right-0 top-full z-50 mt-2 w-60 rounded-lg border border-border bg-bg-card p-2 shadow-lg"
      >
        {browserDownloads.map(({ platform, architecture, href }) => (
          <a
            key={platform}
            href={href}
            download
            className="flex items-center justify-between gap-3 rounded-md px-3 py-2 text-sm text-text hover:bg-text/5"
          >
            {platform}
            <span className="text-[11px] text-text-muted">{architecture}</span>
          </a>
        ))}
        <a
          href="/docs#download"
          className="mt-1 block border-t border-border px-3 pt-3 pb-1 text-xs text-text-muted hover:text-text"
        >
          Installation instructions
        </a>
      </nav>
    </details>
  );
}
