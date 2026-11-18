/** Reusable conceptual nodes and connectors keep illustrations semantic and avoid fabricated screenshots. */
import type { LucideIcon } from 'lucide-react';
import { OyaLogo } from '@/components/oya-logo';
import { Reveal } from '@/components/ui/reveal';
import styles from './visuals.module.css';

/** Visible labels carry meaning; icon motion only enhances their presentation. */
export function VisualNode({
  label,
  icon: Icon,
  order = 0,
}: {
  /** Existing product concept being illustrated. */ label: string;
  /** Decorative icon reinforces the label. */ icon: LucideIcon;
  /** Bounded entrance order within the diagram. */ order?: number;
}) {
  return (
    <Reveal preset="branch" order={order} className={styles.node}>
      <Icon size={26} aria-hidden="true" />
      <span>{label}</span>
    </Reveal>
  );
}

/** Oya's own mark is identity, never a certification seal or a live status. */
export function VisualHub() {
  return (
    <Reveal preset="document" className={styles.hub}>
      <OyaLogo size={76} />
      <span>Oya</span>
    </Reveal>
  );
}

/** A connector grows on its own inner layer, preserving the layout's mobile rotation. */
export function Connector({ order = 0 }: { /** Bounded entrance order. */ order?: number }) {
  return (
    <span className={styles.connector} aria-hidden="true">
      <Reveal preset="connector" order={order}>
        <i />
      </Reveal>
    </span>
  );
}
