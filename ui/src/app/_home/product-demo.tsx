/** A single product scene lets technical buyers inspect three existing healthcare workflows. */
'use client';
import { DEMO_EXAMPLES } from './demo-content';
import { DemoScene } from './demo-scene';
import { useWorkflow } from './use-workflow';
import styles from './demo.module.css';

/** One task controls the selected tab, portal, result and all on-demand evidence. */
export function ProductDemo() {
  const selection = useWorkflow(DEMO_EXAMPLES.length);
  return (
    <section className={styles.demo} id="workflows" aria-labelledby="demo-h">
      <div className={styles.demoHeading} id="playbooks">
        <h2 id="demo-h">Your first task becomes a reusable playbook.</h2>
      </div>
      <div className={styles.taskTabs} role="tablist" aria-label="Workflows">
        {DEMO_EXAMPLES.map(({ task, label }, index) => (
          <button
            key={task.file}
            id={`demo-tab-${index}`}
            type="button"
            role="tab"
            aria-selected={selection.active === index}
            aria-controls={`demo-panel-${index}`}
            tabIndex={selection.active === index ? 0 : -1}
            onClick={() => selection.select(index)}
            onKeyDown={selection.onKeyDown}
          >
            {label}
          </button>
        ))}
      </div>
      {DEMO_EXAMPLES.map((example, index) => (
        <div
          key={example.task.file}
          id={`demo-panel-${index}`}
          role="tabpanel"
          aria-labelledby={`demo-tab-${index}`}
          hidden={selection.active !== index}
        >
          {selection.active === index && <DemoScene key={example.task.file} example={example} />}
        </div>
      ))}
    </section>
  );
}
