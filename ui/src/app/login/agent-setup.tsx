/**
 * The Agent tab of the sign-in page. An agent does not sign in here: it signs
 * itself up through the API (see llms.txt), so this shows the one line to
 * paste into it, pointed at this deployment rather than a hard-coded host.
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

/** What to tell an agent: it signs itself up from llms.txt, no key or person needed first. */
const promptFor = (origin: string) =>
  `Sign yourself up for Oya Browser: read ${origin}/llms.txt and follow "Agent self-signup", using my email. Then connect the Oya desktop app and use it as your browser.`;

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

/** The prompt, and what the agent can do before its person claims its key. */
export function AgentSetup() {
  const origin = useOrigin();
  return (
    <div className="text-sm text-text-muted">
      <p>Agents sign themselves up. Paste this into yours:</p>
      <PromptBox text={promptFor(origin)} />
      <p className="text-xs">
        It gets its own key and can use the free desktop app right away. For Oya Cloud browsers it sends you a claim
        link to open while signed in.{' '}
        <a href="/llms.txt" className="font-medium text-accent hover:text-accent-hover">
          What agents read
        </a>
      </p>
    </div>
  );
}
