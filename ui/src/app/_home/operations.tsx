/** Groups the platform map, operating controls and deployment choices into one reading stage. */
import { Reveal } from '@/components/ui/reveal';
import { ControlPlaneOverview } from './control-plane-overview';
import { EnterpriseCapabilities } from './enterprise-capabilities';
import { Integrations } from './integrations';
import styles from './landing.module.css';
import controlPlane from './control-plane.module.css';

/** Keeps the existing sections and anchors while making their shared purpose clear. */
export function Operations() {
  return (
    <div className={styles.operations}>
      <section className="section" aria-labelledby="architecture-h">
        <div className="wrap">
          <div className={controlPlane.sectionHeading}>
            <h2 id="architecture-h">Scale automatically.</h2>
            <p>
              <mark className={controlPlane.highlight}>
                Go from 1 to thousands of browsers in less than 30 seconds.
              </mark>
            </p>
            <p>
              Capacity depends on your plan and deployment. Manage connections, access rules and run history in Oya’s
              control plane.
            </p>
          </div>
          <Reveal>
            <ControlPlaneOverview />
          </Reveal>
        </div>
      </section>
      <EnterpriseCapabilities />
      <Integrations />
    </div>
  );
}
