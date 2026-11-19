/** Repair paths and portal documents replace long prose with qualified, illustrative product stories. */
import Link from 'next/link';
import { ArrowUpRight, FileText, GitBranch, Sparkles } from 'lucide-react';
import { Reveal } from '@/components/ui/reveal';
import { Disclosure } from '@/components/ui/disclosure';
import { MAINTENANCE_BODY, PORTAL_CONTEXT, PORTAL_SOURCES } from './workflow-story-content';
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
        <p>
          If a page change stops a replay, an agent can try to repair the playbook’s steps. Server repairs can be saved
          and reused on later runs. On desktop, review suggested changes before use.
        </p>
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

/** Document categories are illustrated without fabricated payer content or coverage promises. */
function PortalDocuments() {
  const documents = ['Denial letters', 'Prior-authorization communications', 'Appeal documents'];
  return (
    <figure className={styles.documents} aria-label="Illustrative portal document categories">
      <div className={styles.sheets}>
        {documents.map((label, index) => (
          <article key={label} className={styles.sheet}>
            <Reveal preset="document" order={index}>
              <FileText size={28} aria-hidden="true" />
              <strong>{label}</strong>
              <span aria-hidden="true" className={styles.documentLines} />
            </Reveal>
          </article>
        ))}
      </div>
      <figcaption>Availability varies by payer and plan.</figcaption>
    </figure>
  );
}

/** The short framing remains next to its availability condition and expandable primary sources. */
function PortalStory() {
  return (
    <div className={styles.row}>
      <Reveal>
        <h2>Find denial letters and prior-authorization documents.</h2>
        <p>
          Open available denial letters, prior-authorization communications and appeal documents in the payer portal, so
          your team can review the details and decide what to do next.
        </p>
        <Link className="link" href="/claim-status-automation">
          Explore claim-status workflows <ArrowUpRight size={16} />
        </Link>
      </Reveal>
      <div>
        <PortalDocuments />
        <Disclosure summary="Context and sources">
          <p>{PORTAL_CONTEXT.body}</p>
          <div className={styles.sources} aria-label="Portal context sources">
            {PORTAL_SOURCES.links.map(({ label, url }) => (
              <a key={url} href={url}>
                {label}
              </a>
            ))}
          </div>
        </Disclosure>
      </div>
    </div>
  );
}

/** The two visual compositions share one editorial stage and preserve their full explanations. */
export function WorkflowStory() {
  return (
    <section className={styles.story} aria-label="From first workflow to ongoing operation">
      <div className="wrap">
        <MaintenanceStory />
        <PortalStory />
      </div>
    </section>
  );
}
