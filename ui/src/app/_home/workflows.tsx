/** Revenue-cycle workflows share one selected tab, description and complete portal example. */
'use client';
import { WORKFLOWS } from './content';
import { PortalPane } from './replay-showcase';
import { useWorkflow } from './use-workflow';
import styles from './landing.module.css';

/** A form is complete once the submit step and its result have both been replayed. */
const FINISHED = 2;

/** Props shared by the tab rail and its selected example. */
type Selection = ReturnType<typeof useWorkflow>;

/** The workflow rail supports click, roving focus and wrapped arrow-key navigation. */
function Tabs({ active, select, onKeyDown }: Selection) {
  return (
    <div className="tabs" role="tablist" aria-label="Workflows">
      {WORKFLOWS.map((w, i) => (
        <button
          key={w.id}
          className="tab"
          type="button"
          role="tab"
          id={`wf-tab-${w.id}`}
          aria-selected={i === active}
          aria-controls={`wf-panel-${w.id} wf-screen-${w.id}`}
          tabIndex={i === active ? 0 : -1}
          onClick={() => select(i)}
          onKeyDown={onKeyDown}
        >
          {w.tab}
        </button>
      ))}
    </div>
  );
}

/** The complete form remains in normal flow instead of being cut by its presentation frame. */
function Screens({ active }: { /** The workflow chosen by its tab. */ active: number }) {
  return (
    <div className="frame frame--split span-6 start-7">
      {WORKFLOWS.map((w, i) => (
        <div key={w.id} id={`wf-screen-${w.id}`} className={styles.portal} hidden={i !== active}>
          <PortalPane task={w.form} step={w.form.fields.length + FINISHED} />
        </div>
      ))}
    </div>
  );
}

/** Each job uses the same browser and replay model, with its own readable example. */
export function Workflows() {
  const selection = useWorkflow();
  return (
    <section className="section" id="workflows" aria-labelledby="workflows-h">
      <div className="wrap">
        <div className="grid grid--top">
          <div className="span-5">
            <div className="section-head">
              <h2 id="workflows-h">Revenue cycle work your staff do by hand</h2>
              <p className="muted">Each one runs on the portal as your staff would, and replays without a model.</p>
            </div>
            <Tabs {...selection} />
            {WORKFLOWS.map((w, i) => (
              <div
                key={w.id}
                id={`wf-panel-${w.id}`}
                role="tabpanel"
                aria-labelledby={`wf-tab-${w.id}`}
                hidden={i !== selection.active}
              >
                <p>{w.body}</p>
              </div>
            ))}
          </div>
          <Screens active={selection.active} />
        </div>
      </div>
    </section>
  );
}
