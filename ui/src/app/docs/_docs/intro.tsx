/**
 * The top of the docs: what Oya is in one line, the one line each existing stack changes to run on Oya (most
 * readers arrive with an agent already written), and cards into the sections a developer needs first.
 */
'use client';

import { ArrowUpRight } from 'lucide-react';
import { useDocsNav } from './nav';
import { DocsStack } from './stack';
import { InlineLink } from './blocks';

/** [section id, title, description] for each card. */
const CARDS = [
  ['quickstart', 'Quickstart', 'A replayed task in three minutes'],
  ['playbooks', 'Playbooks', 'Record once, replay with no model'],
  ['mcp-setup', 'MCP', 'Claude, Cursor and any MCP client'],
  ['sdk', 'SDK', 'Every call, in TypeScript'],
  ['cli', 'CLI', 'The same, from a terminal'],
  ['self-hosting', 'Self-hosting', 'Your network, one command'],
];

/** Give coding-agent readers a local setup path before the API-oriented quickstart. */
function AgentEntryLinks() {
  return (
    <p className="text-[15px] leading-relaxed text-text-muted mb-5">
      Using Claude Code, Codex or Cursor? Start with{' '}
      <InlineLink href="/browser-for-claude-code">Oya for coding agents</InlineLink> or{' '}
      <InlineLink href="#mcp-setup">MCP setup</InlineLink> to connect your desktop app without a cloud key.
    </p>
  );
}

/** The headline, pitch and section cards. */
export function DocsIntro() {
  const { navigate } = useDocsNav();
  return (
    <>
      <div className="mb-5 flex items-center justify-between gap-4">
        <p className="eyebrow text-accent">Oya Browser docs</p>
        <a
          href="/docs.md"
          data-md="skip"
          className="text-[12px] text-text-dim hover:text-text transition-colors"
          title="These docs as Markdown, for AI agents and LLMs"
        >
          View as Markdown
        </a>
      </div>
      <h1 className="text-[38px] sm:text-[48px] leading-[1.1] font-medium tracking-[-.045em] text-text mb-5">
        A real browser for your agents.
      </h1>
      <p className="docs-lede max-w-xl text-text-muted mb-8">
        Your agent does a task once in plain language. Oya saves it as Playwright code and replays it with no model,
        using signed-in sessions. Sites can still require verification or block automation. Start with the quickstart to
        record and replay a task.
      </p>
      <AgentEntryLinks />
      <DocsStack />
      <div data-md="skip" className="mb-14 grid gap-3 grid-cols-2 lg:grid-cols-3">
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
