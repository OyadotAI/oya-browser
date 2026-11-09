/** Code and audit evidence stay available without filling the page before they are requested. */
'use client';
import { useState } from 'react';
import type { KeyboardEvent } from 'react';
import { Code2, ListChecks, X } from 'lucide-react';
import type { ReplayTask } from './content';
import { CodePane } from './replay-showcase';
import { Ledger } from './ledger';
import styles from './demo.module.css';

/** Props for a selected example and its current replay step. */
interface DetailsProps {
  /** The selected task's own code and example audit records. */
  task: ReplayTask;
  /** The replay step shared with the visible form. */
  step: number;
}

/** Keeps the currently requested evidence open and restores focus when it is dismissed. */
function useDetails() {
  const [open, setOpen] = useState<'code' | 'audit' | null>(null);
  const close = () => {
    document.getElementById(`demo-${open}-trigger`)?.focus();
    setOpen(null);
  };
  const escape = (event: KeyboardEvent) => {
    if (event.key === 'Escape' && open) {
      event.preventDefault();
      close();
    }
  };
  return { open, close, escape, toggle: (kind: 'code' | 'audit') => setOpen(open === kind ? null : kind) };
}

/** Details belong to the current scene and reset when the task changes. */
export function DemoDetails({ task, step }: DetailsProps) {
  const details = useDetails();
  return (
    <div className={styles.details} onKeyDown={details.escape}>
      <div className={styles.detailActions}>
        <button
          id="demo-code-trigger"
          type="button"
          aria-expanded={details.open === 'code'}
          aria-controls="demo-code"
          onClick={() => details.toggle('code')}
        >
          <Code2 size={16} aria-hidden="true" />
          View code
        </button>
        <button
          id="demo-audit-trigger"
          type="button"
          aria-expanded={details.open === 'audit'}
          aria-controls="demo-audit"
          onClick={() => details.toggle('audit')}
        >
          <ListChecks size={16} aria-hidden="true" />
          View audit trail
        </button>
        <span>Interactive simulation · Synthetic data</span>
      </div>
      <div className={styles.drawer} hidden={!details.open}>
        <button className={styles.close} type="button" aria-label="Close details" onClick={details.close}>
          <X size={16} aria-hidden="true" />
        </button>
        <div id="demo-code" role="region" aria-label="Generated playbook" tabIndex={0} hidden={details.open !== 'code'}>
          <CodePane task={task} step={step} />
        </div>
        <div
          id="demo-audit"
          role="region"
          aria-label="Example audit trail"
          tabIndex={0}
          hidden={details.open !== 'audit'}
        >
          <Ledger task={task} step={step} />
        </div>
      </div>
    </div>
  );
}
