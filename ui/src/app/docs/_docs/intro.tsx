/**
 * The top of the docs: the headline, the pitch, and four cards into the most
 * read sections.
 */
'use client';

import { ArrowUpRight } from 'lucide-react';
import { useDocsNav } from './nav';

/** [section id, title, description] for each card. */
const CARDS = [
  ['control-plane', 'Control Plane', 'Architecture & model'],
  ['comparison', 'Why Oya (10x)', 'Comparison vs raw runners'],
  ['routing-failover', 'Routing & Failover', 'Zero-rewrite resilience'],
  ['sdk', 'TypeScript SDK', 'Build from code'],
];

/** The headline, pitch and section cards. */
export function DocsIntro() {
  const { navigate } = useDocsNav();
  return (
    <>
      <p className="eyebrow mb-5 text-accent">Browser Infrastructure</p>
      <h1 className="text-[40px] sm:text-[52px] leading-[1.1] font-medium tracking-[-.05em] text-text mb-5">
        Your browser, the Control Plane.
      </h1>
      <p className="max-w-xl text-text-muted mb-8 text-[16px] leading-7">
        Orchestrate Oya Cloud, Browserbase, Steel, Anchor, Browser Use, and private Chrome behind one API. Deterministic
        personas, zero-rewrite failover, and sub-second live takeover.
      </p>
      <div className="mb-12 grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
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
            <span className="flex items-center justify-between text-[13px] font-medium">
              {title}
              <ArrowUpRight size={13} className="text-text-dim group-hover:text-accent" />
            </span>
            <span className="mt-2 block text-[12px] text-text-dim">{description}</span>
          </a>
        ))}
      </div>
    </>
  );
}
