/**
 * A run's card in the conversation (`.chat-run`): the plan with its ticked
 * steps, the timeline of narrated actions, and either the live step line
 * (while the agent works) or the summary it folds behind (once it is done).
 */
import { useViewModel } from '../../../hooks/index.ts';
import { ASK_TEXT } from '../model/constants.ts';
import { progressMeta } from '../model/steps.ts';
import type { RunCard, RunViewModel } from '../view-models/run-view-model.ts';
import type { RunItem } from '../view-models/ask-view-model.ts';

/** The plan and the timeline of a card. */
function RunBody({ card }: { /** The card drawn. */ card: RunCard }) {
  return (
    <>
      <ol className="run-plan" hidden={!card.plan.length}>
        {card.plan.map((s, i) => (
          <li key={i} className={s.done ? 'done' : undefined}>
            {String(s.step || '')}
          </li>
        ))}
      </ol>
      <ol className="run-steps">
        {card.steps.map((s, i) => (
          <li key={i} className="run-step">
            <span className="run-step-line">{s.line}</span>
            <span className="run-step-time">{s.time}</span>
          </li>
        ))}
      </ol>
    </>
  );
}

/** The run in flight, with its live line: the latest step and how long it has taken. */
export function LiveRun({ run }: { /** The live run. */ run: RunViewModel }) {
  const { card, label, count, seconds } = useViewModel(run);
  if (!card) return null;
  return (
    <div className="chat-run">
      <RunBody card={card} />
      <div className="chat-thinking">
        <div className="chat-dots">
          <span />
          <span />
          <span />
        </div>
        <span className="chat-step-label">{label || ASK_TEXT.thinking}</span>
        <span className="chat-step-meta">{progressMeta(count, seconds)}</span>
      </div>
    </div>
  );
}

/** What a finished card is given. */
interface FinishedRunProps {
  /** The card's entry in the conversation. */
  item: RunItem;
  /** Opens or folds its steps. */
  onToggle: () => void;
}

/** A finished run: done or failed, folded behind its summary when it had steps. */
export function FinishedRunCard({ item, onToggle }: FinishedRunProps) {
  const { run, folded } = item;
  return (
    <div className={`chat-run ${run.outcome}${folded ? ' folded' : ''}`}>
      {run.summary && (
        <button type="button" className="run-summary" aria-expanded={!folded} onClick={onToggle}>
          {run.summary}
        </button>
      )}
      <RunBody card={run} />
    </div>
  );
}
