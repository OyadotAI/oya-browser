/**
 * A started run: its status, what it needs from a person when paused, and how
 * it ended.
 */
'use client';

import { ExternalLink } from 'lucide-react';
import { ATTENTION } from './constants';
import { replyPlaceholder, statusClass, statusLabel } from './format';
import type { RunDialogState } from './use-run';
import type { RunAttention, RunInfo, RunResult } from './types';

/** Props for the attention box. */
interface AttentionProps {
  /** What the run needs. */
  attention: RunAttention;
  /** The reply box and send. */
  reply: RunDialogState['reply'];
}

/** What the paused run needs, and the reply box. */
function AttentionBox({ attention, reply }: AttentionProps) {
  return (
    <div className="flex flex-col gap-3 rounded-lg border border-yellow/40 bg-yellow/10 p-3">
      <div className="flex items-center gap-2">
        <span className="rounded border border-yellow/40 px-1.5 text-[11px] uppercase tracking-wider text-yellow">
          {ATTENTION[attention.reason]}
        </span>
        {attention.liveViewUrl && (
          <a
            className="ml-auto inline-flex items-center gap-1 text-[12px] text-accent hover:underline"
            href={attention.liveViewUrl}
            target="_blank"
            rel="noreferrer"
          >
            Open live view <ExternalLink className="h-3 w-3" />
          </a>
        )}
      </div>
      <p className="text-text-secondary">{attention.message}</p>
      <div className="flex gap-2">
        <input
          className="field flex-1"
          value={reply.reply}
          onChange={(e) => reply.setReply(e.target.value)}
          aria-label="Reply"
          placeholder={replyPlaceholder(attention.reason)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') reply.respond();
          }}
        />
        <button className="btn-primary" onClick={reply.respond}>
          {attention.reason === 'agent' ? 'Reply' : 'Done, continue'}
        </button>
      </div>
    </div>
  );
}

/** Props for the outcome. */
interface OutcomeProps {
  /** How the run went. */
  result: RunResult;
}

/** How a successful run went. */
function Outcome({ result }: OutcomeProps) {
  return (
    <div className="flex flex-col gap-1 text-text-secondary">
      {result.total !== undefined && (
        <p>
          Replayed {result.steps} of {result.total} steps without the LLM.
        </p>
      )}
      {result.healed && (
        <p className="text-yellow">
          The page had changed. The agent finished the run and saved its fix as a draft for review.
        </p>
      )}
      {result.fellBack && !result.healed && <p className="text-yellow">A person finished the run.</p>}
      {result.text && <p className="whitespace-pre-wrap">{result.text}</p>}
    </div>
  );
}

/** Props for the status view. */
interface StatusProps {
  /** The started run. */
  run: RunInfo;
  /** The reply box and send. */
  reply: RunDialogState['reply'];
}

/** The run's status, attention and outcome. */
export default function RunStatusView({ run, reply }: StatusProps) {
  const result = run.result;
  return (
    <div className="flex flex-col gap-4 text-sm">
      <p>
        Status: <span className={statusClass(run.status)}>{statusLabel(run.status)}</span>
      </p>
      {run.attention && <AttentionBox attention={run.attention} reply={reply} />}
      {run.status === 'succeeded' && result && <Outcome result={result} />}
      {run.status === 'failed' && <p className="text-red">{run.error}</p>}
    </div>
  );
}
