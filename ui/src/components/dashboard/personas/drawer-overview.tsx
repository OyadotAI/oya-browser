/**
 * The top of the profile drawer: the account sessions saved to the persona,
 * and the browsers running as it now.
 */
'use client';

import { ago } from '@/lib/api-client';
import { RUNNING_LISTED } from './constants';
import { pair } from './drawer-actions';
import { capLabel, runningAs } from './model';
import type { DrawerCtx } from './use-persona-drawer';

/** Saved sites, and the button that opens the desktop to sign in to more. */
export function SessionsSection(ctx: DrawerCtx) {
  const { p, d } = ctx;
  return (
    <section>
      <h3 className="label">Saved account sessions</h3>
      <p className="mb-3 break-words text-sm text-text-secondary">
        {p.login?.sites.length ? p.login.sites.join(' · ') : 'No account sessions saved yet.'}
      </p>
      <button className="btn-primary" disabled={!!d.busy} onClick={() => pair(ctx)}>
        Sign in on desktop
      </button>
      <p className="mt-2 text-xs text-text-muted">This opens the same Oya desktop window using this profile.</p>
    </section>
  );
}

/** The running browsers against the cap; past a handful, a link to the filtered fleet. */
export function RunningSection({ p, props }: DrawerCtx) {
  const running = runningAs(props.browsers, p.id);
  return (
    <section>
      <div className="flex items-center justify-between">
        <h3 className="label mb-0">Running now</h3>
        <span
          className={`text-[12px] num ${running.length >= (p.maxConcurrent ?? Infinity) ? 'text-yellow' : 'text-text-muted'}`}
        >
          {running.length} / {capLabel(p.maxConcurrent)}
        </span>
      </div>
      {running.length ? (
        <div className="mt-1.5 rounded-md border border-border bg-bg text-[12.5px]">
          {running.slice(0, RUNNING_LISTED).map((b) => (
            <div key={b.id} className="flex items-center gap-2 border-b border-border/60 px-2 py-1 last:border-0">
              <span className={`dot dot-${b.health}`} />
              <span className="truncate text-text">{b.name}</span>
              <span className="ml-auto truncate font-mono text-[11px] text-text-muted">
                {b.currentUrl.replace(/^https?:\/\//, '')}
              </span>
            </div>
          ))}
          {running.length > RUNNING_LISTED && (
            <button
              className="w-full px-2 py-1 text-left text-[12px] text-text-muted hover:text-text"
              onClick={() => props.onShowBrowsers(p.id)}
            >
              and {running.length - RUNNING_LISTED} more, show in fleet
            </button>
          )}
        </div>
      ) : (
        <p className="mt-1 text-[12.5px] text-text-dim">
          Idle{p.lastUsedAt ? ` · last used ${ago(p.lastUsedAt, props.now)} ago` : ''}.
        </p>
      )}
    </section>
  );
}
