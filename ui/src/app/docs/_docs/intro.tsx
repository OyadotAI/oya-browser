/**
 * The top of the docs: what Oya is in one line, and cards into the sections a
 * developer needs first, the quickstart leading.
 */
'use client';

import { ArrowUpRight } from 'lucide-react';
import { useDocsNav } from './nav';

/** [section id, title, description] for each card. */
const CARDS = [
  ['quickstart', 'Quickstart', 'A replayed task in three minutes'],
  ['playbooks', 'Playbooks', 'Record once, replay with no model'],
  ['mcp-setup', 'MCP', 'Claude, Cursor and any MCP client'],
  ['sdk', 'SDK', 'Every call, in TypeScript'],
  ['cli', 'CLI', 'The same, from a terminal'],
  ['self-hosting', 'Self-hosting', 'Your network, one command'],
];

/** The headline, pitch and section cards. */
export function DocsIntro() {
  const { navigate } = useDocsNav();
  return (
    <>
      <p className="eyebrow mb-5 text-accent">Oya Browser docs</p>
      <h1 className="text-[38px] sm:text-[48px] leading-[1.1] font-medium tracking-[-.045em] text-text mb-5">
        A real browser for your agents.
      </h1>
      <p className="docs-lede max-w-xl text-text-muted mb-8">
        Your agent does a task once in plain language. Oya saves it as Playwright code and replays it with no model,
        signed in like your staff and never flagged as a bot. Start with the quickstart: three steps, three minutes.
      </p>
      <div className="mb-14 grid gap-3 grid-cols-2 lg:grid-cols-3">
        {CARDS.map(([id, title, description]) => (
          <a
            key={id}
            href={'#' + id}
            onClick={(e) => {
              e.preventDefault();
              navigate(id);
            }}
            className="group rounded-xl border border-border bg-bg-card/40 p-4 hover:border-accent/40"
          >
            <span className="flex items-center justify-between text-[14.5px] font-medium text-text">
              {title}
              <ArrowUpRight size={13} className="text-text-dim group-hover:text-accent" />
            </span>
            <span className="mt-1.5 block text-[13px] leading-snug text-text-dim">{description}</span>
          </a>
        ))}
      </div>
    </>
  );
}
