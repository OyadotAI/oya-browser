/**
 * The Agent tab offers the desktop connection prompt instead of asking people
 * to create a cloud key for local browsing. Links use this deployment's origin.
 */
'use client';

import { useSyncExternalStore } from 'react';
import CopyExample from '@/components/copy-example';

/** The host the snippets name during server rendering, before the page knows its own. */
const DEFAULT_ORIGIN = 'https://oyabrowser.com';

/** Nothing to subscribe to: a page's origin never changes. */
const subscribe = () => () => {};

/** This deployment's origin. */
function useOrigin() {
  return useSyncExternalStore(
    subscribe,
    () => window.location.origin,
    () => DEFAULT_ORIGIN,
  );
}

/** Direct the agent to the current desktop quickstart while preserving the person's connection consent. */
const promptFor = (origin: string) =>
  `Set up Oya Browser for me: read ${origin}/llms.txt and follow "Desktop setup for coding agents". Help me complete welcome and Connect, then tell me to start a new agent session to use the oya-browser tools.`;

/** PromptBox's props. */
interface PromptProps {
  /** The prompt to show and copy. */
  text: string;
}

/** The prompt as wrapped plain text: it is prose, so CodeBlock's highlighting would color its words. */
function PromptBox({ text }: PromptProps) {
  return (
    <div className="my-3 rounded-xl border border-border bg-bg-card p-4">
      <p className="mb-2 font-mono text-[12px] leading-relaxed whitespace-pre-wrap text-text">{text}</p>
      <span className="btn-ghost h-7 text-[11px]">
        <CopyExample code={text} />
      </span>
    </div>
  );
}

/** Explain desktop consent and optional import without conflating local setup with cloud signup. */
export function AgentSetup() {
  const origin = useOrigin();
  return (
    <div className="text-sm text-text-muted">
      <p>Connect your agent to the desktop app. Paste this into yours:</p>
      <PromptBox text={promptFor(origin)} />
      <p className="text-xs">
        No Oya account or cloud key is needed. You confirm Connect and any permission for optional login import; you can
        also sign in within Oya. Keep Oya open and start a new agent session after connecting.{' '}
        <a href="/llms.txt" className="font-medium text-accent hover:text-accent-hover">
          What agents read
        </a>
      </p>
    </div>
  );
}
