/** The repair paths replace long prose with a qualified, illustrative story of what happens when a portal changes. */
import Link from 'next/link';
import { ArrowUpRight, FileText, GitBranch, Sparkles } from 'lucide-react';
import { Reveal } from '@/components/ui/reveal';
import { Disclosure } from '@/components/ui/disclosure';
import { MAINTENANCE_BODY } from './workflow-story-content';
import styles from './workflow-story.module.css';

/** The diagram explains possible repair paths without pretending a live run is in progress. */
function RepairPaths() {
  return (
    <figure className={styles.repair} aria-label="How workflow repairs differ">
      <Sparkles size={32} aria-hidden="true" />
      <div className={styles.branches}>
        <Reveal preset="branch">
          <div>
            <GitBranch aria-hidden="true" />
            <strong>Server</strong>
            <span>Repairs can be saved.</span>
          </div>
        </Reveal>
        <Reveal preset="branch" order={1}>
          <div>
            <FileText aria-hidden="true" />
            <strong>Desktop</strong>
            <span>Review suggested changes before use.</span>
          </div>
        </Reveal>
      </div>
    </figure>
  );
}

/** Technical qualifications remain visible beside the illustrated outcomes. */
function MaintenanceStory() {
  return (
    <div className={styles.row}>
      <Reveal>
        <h2>When the portal changes, Oya autoheals the code.</h2>
        <p>If a page change stops a replay, an agent can try to repair the playbook’s steps.</p>
        <Link className="link" href="/docs/playbooks#healing">
          How playbook repair works <ArrowUpRight size={16} />
        </Link>
      </Reveal>
      <div>
        <RepairPaths />
        <Disclosure summary="Repair behavior">
          <p>{MAINTENANCE_BODY}</p>
        </Disclosure>
      </div>
    </div>
  );
}

/** The maintenance story keeps its own stage so the repair diagram reads as one step of the page's arc. */
export function WorkflowStory() {
  return (
    <section className={styles.story} aria-label="From first workflow to ongoing operation">
      <div className="wrap">
        <MaintenanceStory />
      </div>
    </section>
  );
}
