/**
 * The landing page's interactive diagram of how playbooks work: a tab per
 * stage (record, replay, repair) with arrow-key navigation between tabs.
 */
'use client';

import { useRef, useState, type KeyboardEvent } from 'react';
import { ArrowDown, ArrowRight, Braces, Check, FileCode2, WandSparkles } from 'lucide-react';
import styles from './workflow-diagram.module.css';
import SyntaxCode from '@/components/ui/syntax-code';
import { REPAIR_STAGE, REPAIRED_STEP, stages } from './workflow-stages';

/** Where each key moves the selected tab, given the current index and the count. */
const TAB_KEYS: Record<string, (index: number, count: number) => number> = {
  ArrowRight: (index, count) => (index + 1) % count,
  ArrowLeft: (index, count) => (index + count - 1) % count,
  Home: () => 0,
  End: (_, count) => count - 1,
};

/** The selected stage and roving-tabindex keyboard navigation between tabs. */
function useStageTabs() {
  const [active, setActive] = useState(0);
  const tabs = useRef<Array<HTMLButtonElement | null>>([]);
  /** Moves the selection for an arrow, Home or End key. */
  function navigate(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    if (!Object.hasOwn(TAB_KEYS, event.key)) return;
    const next = TAB_KEYS[event.key](index, stages.length);
    event.preventDefault();
    setActive(next);
    tabs.current[next]?.focus();
  }
  return { active, setActive, tabs, navigate };
}

/** StageCanvas's props. */
interface CanvasProps {
  /** The selected stage's index. */
  active: number;
}

/** The selected stage drawn as input → steps → output. */
function StageCanvas({ active }: CanvasProps) {
  const stage = stages[active];
  const StageIcon = stage.icon;
  const repair = active === REPAIR_STAGE;
  return (
    <div className={styles.canvas} key={stage.name}>
      <div className={styles.canvasHeading}>
        <StageIcon size={16} aria-hidden="true" />
        {stage.kicker}
      </div>
      <div className={styles.input}>
        <Braces size={18} aria-hidden="true" />
        <span>{stage.input}</span>
      </div>
      <div className={styles.connector}>
        <ArrowDown size={17} aria-hidden="true" />
      </div>
      <ol className={styles.flow}>
        {stage.steps.map((step, index) => (
          <li key={step}>
            <span className={`${styles.node} ${repair && index === REPAIRED_STEP ? styles.repair : ''}`}>
              {repair && index === REPAIRED_STEP ? (
                <WandSparkles size={18} aria-hidden="true" />
              ) : (
                <Check size={18} aria-hidden="true" />
              )}
            </span>
            <span>{step}</span>
            {index < stage.steps.length - 1 && <ArrowRight className={styles.flowArrow} size={15} aria-hidden="true" />}
          </li>
        ))}
      </ol>
      <div className={styles.connector}>
        <ArrowDown size={17} aria-hidden="true" />
      </div>
      <div className={styles.output}>
        <FileCode2 size={24} aria-hidden="true" />
        <div>
          <strong>{stage.output}</strong>
          <span>portal-request-review{repair ? ':draft' : ''}</span>
        </div>
      </div>
    </div>
  );
}

/** The diagram: tabs, the selected stage's canvas, and its code. */
export default function WorkflowDiagram() {
  const { active, setActive, tabs, navigate } = useStageTabs();
  const stage = stages[active];
  return (
    <div className={styles.diagram}>
      <div className={styles.tabs} role="tablist" aria-label="How playbooks work">
        {stages.map(({ name, icon: Icon }, index) => (
          <button
            key={name}
            ref={(el) => {
              tabs.current[index] = el;
            }}
            type="button"
            role="tab"
            id={`workflow-tab-${index}`}
            aria-selected={active === index}
            aria-controls="workflow-panel"
            tabIndex={active === index ? 0 : -1}
            onClick={() => setActive(index)}
            onKeyDown={(event) => navigate(event, index)}
          >
            <Icon size={15} aria-hidden="true" />
            {name}
          </button>
        ))}
      </div>
      <div id="workflow-panel" role="tabpanel" aria-labelledby={`workflow-tab-${active}`} tabIndex={0}>
        <StageCanvas active={active} />
        <div className={styles.explanation}>
          <SyntaxCode code={stage.code} language="typescript" />
          <p>{stage.note}</p>
        </div>
      </div>
      <span className={styles.caption}>Interactive diagram · fictional task</span>
    </div>
  );
}
