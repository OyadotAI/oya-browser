/** A permanent architecture map explains the product category while one packet illustrates routing. */
'use client';
import type { CSSProperties } from 'react';
import { ArrowRight, Boxes, Globe2, Layers3, Pause, Play, Waypoints } from 'lucide-react';
import { OyaLogo } from '@/components/oya-logo';
import { CONTROL_STAGE_COUNT } from './constants';
import { useControlPlaneMotion } from './use-control-plane-motion';
import styles from './control-plane.module.css';

/** All system layers remain visible, including before hydration and with reduced motion. */
const LAYERS = [
  { title: 'Agents & apps', detail: 'MCP · TypeScript · Playwright', icon: Waypoints },
  { title: 'Oya control plane', detail: 'Projects · Policies · Queues', icon: Layers3 },
  { title: 'Browser fleet', detail: 'Oya · Other providers', icon: Boxes },
  { title: 'Healthcare portals', detail: 'Payers · EHRs · Registries', icon: Globe2 },
];

/** Architecture is readable first; motion indicates the relationship between its layers. */
export function ControlPlaneOverview() {
  const { ref, phase, paused, still, toggle } = useControlPlaneMotion();
  const progress = { '--route-progress': phase / (CONTROL_STAGE_COUNT - 1) } as CSSProperties;
  return (
    <figure
      ref={ref}
      className={styles.overview}
      id="control-plane"
      aria-label="Oya architecture: agents and apps through the control plane to browser fleets and healthcare portals"
    >
      <div className={styles.toolbar}>
        <span>Your browser workflows. One control plane.</span>
        {!still && (
          <button type="button" onClick={toggle}>
            {paused ? <Play size={14} aria-hidden="true" /> : <Pause size={14} aria-hidden="true" />}
            {paused ? 'Play diagram' : 'Pause diagram'}
          </button>
        )}
      </div>
      <div className={styles.architecture}>
        <div className={styles.route} aria-hidden="true">
          <span className={styles.packet} data-phase={phase} style={progress} />
        </div>
        {LAYERS.map(({ title, detail, icon: Icon }, index) => (
          <div key={title} className={styles.layer} data-center={index === 1} data-active={index === phase}>
            <div className={styles.layerIcon}>
              {index === 1 ? <OyaLogo size={36} /> : <Icon size={27} aria-hidden="true" />}
            </div>
            <h3>{title}</h3>
            <p>{detail}</p>
            {index === 1 && (
              <div className={styles.controlTags}>
                <span>Orchestrate</span>
                <span>Govern</span>
                <span>Observe</span>
              </div>
            )}
            {index < CONTROL_STAGE_COUNT - 1 && <ArrowRight className={styles.arrow} size={18} aria-hidden="true" />}
          </div>
        ))}
      </div>
      <figcaption className={styles.caption}>Illustrated routing · Runtime guarantees vary by provider</figcaption>
    </figure>
  );
}
