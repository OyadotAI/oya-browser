'use client';

import { useRef, useState, type KeyboardEvent } from 'react';
import { ArrowDown, ArrowRight, Braces, Check, FileCode2, MousePointer2, Repeat2, WandSparkles, Wrench } from 'lucide-react';
import styles from './workflow-diagram.module.css';
import SyntaxCode from '@/components/ui/syntax-code';

const stages = [
  {
    name: 'Record', icon: MousePointer2, kicker: 'A task becomes a workflow',
    input: '“Prepare a portal request.”',
    steps: ['Navigate', 'Fill', 'Review'],
    output: 'Save the successful run',
    code: 'browser.toPlaybook("portal-request-review")',
    note: 'The agent does the task. You keep the steps.',
  },
  {
    name: 'Replay', icon: Repeat2, kicker: 'Same workflow. New inputs.',
    input: '{ requestId: "DEMO-0002" }',
    steps: ['Navigate', 'Fill', 'Review'],
    output: 'Run the recorded steps',
    code: 'browser.play("portal-request-review", data)',
    note: 'Recorded steps replay without an LLM in the loop.',
  },
  {
    name: 'Repair', icon: Wrench, kicker: 'A changed page is a review point',
    input: 'The portal form changed.',
    steps: ['Step breaks', 'Agent repairs', 'You review'],
    output: 'Keep the fix as a draft',
    code: 'oya.playbooks.promote("portal-request-review")',
    note: 'Auto-healing saves a draft. Promote it after review.',
  },
];

export default function WorkflowDiagram() {
  const [active, setActive] = useState(0);
  const tabs = useRef<Array<HTMLButtonElement | null>>([]);
  const stage = stages[active];
  const StageIcon = stage.icon;

  function navigate(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    let next: number;
    if (event.key === 'ArrowRight') next = (index + 1) % stages.length;
    else if (event.key === 'ArrowLeft') next = (index + stages.length - 1) % stages.length;
    else if (event.key === 'Home') next = 0;
    else if (event.key === 'End') next = stages.length - 1;
    else return;
    event.preventDefault();
    setActive(next);
    tabs.current[next]?.focus();
  }

  return (
    <div className={styles.diagram}>
      <div className={styles.tabs} role="tablist" aria-label="How playbooks work">
        {stages.map(({ name, icon: Icon }, index) => (
          <button key={name} ref={el => { tabs.current[index] = el; }} type="button"
            role="tab" id={`workflow-tab-${index}`} aria-selected={active === index}
            aria-controls="workflow-panel" tabIndex={active === index ? 0 : -1}
            onClick={() => setActive(index)} onKeyDown={event => navigate(event, index)}>
            <Icon size={15} aria-hidden="true" />{name}
          </button>
        ))}
      </div>
      <div id="workflow-panel" role="tabpanel" aria-labelledby={`workflow-tab-${active}`} tabIndex={0}>
        <div className={styles.canvas} key={stage.name}>
          <div className={styles.canvasHeading}><StageIcon size={16} aria-hidden="true" />{stage.kicker}</div>
          <div className={styles.input}><Braces size={18} aria-hidden="true" /><span>{stage.input}</span></div>
          <div className={styles.connector}><ArrowDown size={17} aria-hidden="true" /></div>
          <ol className={styles.flow}>
            {stage.steps.map((step, index) => (
              <li key={step}>
                <span className={`${styles.node} ${active === 2 && index === 1 ? styles.repair : ''}`}>
                  {active === 2 && index === 1 ? <WandSparkles size={18} aria-hidden="true" /> : <Check size={18} aria-hidden="true" />}
                </span>
                <span>{step}</span>
                {index < 2 && <ArrowRight className={styles.flowArrow} size={15} aria-hidden="true" />}
              </li>
            ))}
          </ol>
          <div className={styles.connector}><ArrowDown size={17} aria-hidden="true" /></div>
          <div className={styles.output}><FileCode2 size={24} aria-hidden="true" /><div><strong>{stage.output}</strong><span>portal-request-review{active === 2 ? ':draft' : ''}</span></div></div>
        </div>
        <div className={styles.explanation}>
          <SyntaxCode code={stage.code} language="typescript" />
          <p>{stage.note}</p>
        </div>
      </div>
      <span className={styles.caption}>Interactive diagram · fictional task</span>
    </div>
  );
}
