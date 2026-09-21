/**
 * The playbooks section: what happens after a run works, and what happens
 * when the portal moves under it.
 */
import WorkflowDiagram from '@/components/workflow-diagram';
import styles from '../page.module.css';

/** Record, replay, repair, with the diagram that walks through all three. */
export function Playbooks() {
  return (
    <section className={styles.playbooks} aria-labelledby="playbooks-title">
      <div className={styles.playbooksIntro}>
        <p className={styles.eyebrow}>Playbooks</p>
        <h2 id="playbooks-title">
          The tenth run
          <br />
          <span>should not cost a model call.</span>
        </h2>
        <p>
          Save the run that worked. Replay it with new inputs and no LLM in the loop. When the portal moves, the agent
          repairs the step and leaves a draft for a person to promote.
        </p>
      </div>
      <WorkflowDiagram />
    </section>
  );
}
