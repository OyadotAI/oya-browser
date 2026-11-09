/** Enterprise buyers see every capability summary and can select the supporting technical detail. */
'use client';
import Link from 'next/link';
import { ArrowUpRight, Check } from 'lucide-react';
import { CAPABILITY_GROUPS } from './enterprise-content';
import { useWorkflow } from './use-workflow';
import styles from './control-plane.module.css';

/** The platform's three jobs remain visible while the selected group reveals its depth. */
export function EnterpriseCapabilities() {
  const { active, select, onKeyDown } = useWorkflow(CAPABILITY_GROUPS.length);
  return (
    <section className="section" id="capabilities" aria-labelledby="capabilities-h">
      <div className="wrap" id="audit">
        <div className={styles.sectionHeading}>
          <h2 id="capabilities-h">Control how the work runs.</h2>
          <p>Choose how tasks are queued, who can act and what your team can review.</p>
        </div>
        <div className={styles.explorer}>
          <div
            className={styles.capabilityTabs}
            role="tablist"
            aria-label="Control plane capabilities"
            aria-orientation="vertical"
          >
            {CAPABILITY_GROUPS.map((group, index) => (
              <button
                key={group.id}
                type="button"
                role="tab"
                id={`cap-tab-${group.id}`}
                aria-selected={active === index}
                aria-controls={`cap-panel-${group.id}`}
                tabIndex={active === index ? 0 : -1}
                onClick={() => select(index)}
                onKeyDown={onKeyDown}
              >
                <span>
                  {group.title}
                  <ArrowUpRight size={19} aria-hidden="true" />
                </span>
                <span>{group.summary}</span>
              </button>
            ))}
          </div>
          {CAPABILITY_GROUPS.map((group, index) => (
            <CapabilityPanel key={group.id} group={group} selected={active === index} />
          ))}
        </div>
      </div>
    </section>
  );
}

/** One selected capability group exposes source-backed detail and the corresponding documentation. */
function CapabilityPanel({
  group,
  selected,
}: {
  /** The selected capability's data. */ group: (typeof CAPABILITY_GROUPS)[number];
  /** Whether this panel is active. */ selected: boolean;
}) {
  return (
    <div
      className={styles.capabilityPanel}
      role="tabpanel"
      id={`cap-panel-${group.id}`}
      aria-labelledby={`cap-tab-${group.id}`}
      hidden={!selected}
    >
      <p className={styles.panelLead}>{group.detail}</p>
      <ul>
        {group.items.map(([title, body]) => (
          <li key={title}>
            <Check size={17} aria-hidden="true" />
            <div>
              <h3>{title}</h3>
              <p>{body}</p>
            </div>
          </li>
        ))}
      </ul>
      <Link className="link" href={group.href}>
        Read technical details
      </Link>
    </div>
  );
}
