/** Human/agent ownership is demonstrated explicitly, without sending any real browser command. */
'use client';
import { RotateCcw, UserRound } from 'lucide-react';
import type { ExampleOwner } from './use-handoff';
import styles from './demo.module.css';

/** Copy describes the same pause-on-release contract as Oya's control API. */
const LABELS: Record<
  ExampleOwner,
  {
    /** Current ownership shown to the visitor. */ state: string;
    /** The only next ownership action. */ action: string;
  }
> = {
  running: { state: 'Agent running', action: 'Try human takeover' },
  human: { state: 'Human control · Agent paused', action: 'Release control' },
  paused: { state: 'Agent paused', action: 'Resume agent' },
};

/** A bounded example timeline shows the causal ownership changes. */
interface Props {
  /** Current simulated owner. */ owner: ExampleOwner;
  /** Manual pause is independent of which actor owns control. */ executionPaused: boolean;
  /** Recent synthetic ownership events, distinct from hash-chain rows. */ events: string[];
  /** Applies the current state's next action. */ advance: () => void;
  /** Restarts the illustration without affecting a real browser. */ restart: () => void;
}

/** Manual controls remain usable with reduced motion and explain why release does not resume work. */
export function HandoffControls({ owner, executionPaused, events, advance, restart }: Props) {
  return (
    <div className={styles.handoff}>
      <div className={styles.handoffBar}>
        <span>
          <UserRound size={17} aria-hidden="true" />
          {owner === 'running' && executionPaused ? 'Agent paused' : LABELS[owner].state}
        </span>
        <div>
          <button type="button" onClick={advance}>
            {LABELS[owner].action}
          </button>
          <button type="button" onClick={restart}>
            <RotateCcw size={14} aria-hidden="true" />
            Restart example
          </button>
        </div>
      </div>
      {events.length > 0 && (
        <ol className={styles.ownershipEvents} aria-label="Example ownership events">
          {events.map((event, index) => (
            <li key={`${index}-${event}`}>{event}</li>
          ))}
        </ol>
      )}
    </div>
  );
}
