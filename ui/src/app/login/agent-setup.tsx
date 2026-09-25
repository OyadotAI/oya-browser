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

/** What to tell an agent: the SDK quickstart in llms.txt, which signs it up and pairs the desktop app with the person's logins. */
const promptFor = (origin: string) =>
  `Set up Oya Browser for me: read ${origin}/llms.txt and follow "Agent quickstart" with my email. Use the @oya-ai/browser SDK, not MCP, and connect my Oya desktop app so you browse with my logins.`;

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
        It gets its own key and pairs your Oya desktop app. Click Connect when the app asks, and keep &quot;Also import
        my logins&quot; ticked so it is signed in where you are. For Oya Cloud browsers it sends you a claim link.{' '}
        <a href="/llms.txt" className="font-medium text-accent hover:text-accent-hover">
          What agents read
        </a>
      </p>
    </div>
  );
}
