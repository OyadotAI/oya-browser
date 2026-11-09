/** A static, accessible diagram keeps the integration example visible without a script mount. */
import { DIAGRAM } from './content';
import { HUB_LOGO } from './constants';
import { OyaLogo } from '@/components/oya-logo';
import styles from './integration-diagram.module.css';

/** The existing diagram lists its agent inputs before its portal destinations. */
const AGENT_COUNT = 3;

/** One side of the connection diagram, using the original labels and letter marks. */
function Column({ spokes }: { /** Existing diagram endpoints on this side of Oya. */ spokes: typeof DIAGRAM.spokes }) {
  return (
    <div className={styles.column}>
      {spokes.map(({ label, icon }) => (
        <div key={label} className={styles.endpoint}>
          <span className={styles.mark} aria-hidden="true">
            {icon.replace('text:', '')}
          </span>
          <span>{label}</span>
        </div>
      ))}
    </div>
  );
}

/** Existing agents connect through Oya to the same portal destinations. */
export function IntegrationDiagram() {
  return (
    <div className={styles.diagram} role="img" aria-label={DIAGRAM.alt}>
      <Column spokes={DIAGRAM.spokes.slice(0, AGENT_COUNT)} />
      <div className={styles.hub}>
        <OyaLogo size={HUB_LOGO} />
        <span>{DIAGRAM.center.label}</span>
      </div>
      <Column spokes={DIAGRAM.spokes.slice(AGENT_COUNT)} />
    </div>
  );
}
