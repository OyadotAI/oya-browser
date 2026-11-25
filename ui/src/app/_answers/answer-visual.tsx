/** Six source-backed visual families make every answer-page hero a composition rather than an article heading. */
import {
  Bot,
  Boxes,
  FileCode2,
  FileText,
  Hand,
  KeyRound,
  ListChecks,
  MousePointer2,
  PanelTop,
  Repeat2,
} from 'lucide-react';
import { Reveal } from '@/components/ui/reveal';
import type { AnswerPageContent } from './content';
import type { VisualFamily } from './presentation';
import { Connector, VisualHub, VisualNode } from './visual-parts';
import { VISUAL_CRITERIA_LIMIT } from './constants';
import styles from './visuals.module.css';

/** Separate personal and agent tabs connect to one saved artifact. */
function DesktopVisual() {
  return (
    <div className={styles.desktop}>
      <div className={styles.tabPair}>
        <VisualNode label="Your tabs" icon={PanelTop} />
        <VisualNode label="Agent tab" icon={MousePointer2} order={1} />
      </div>
      <Connector />
      <VisualHub />
      <Connector order={1} />
      <VisualNode label="Saved steps" icon={FileCode2} order={2} />
    </div>
  );
}
/** Agent infrastructure keeps human control visible as an independent branch. */
function FleetVisual() {
  return (
    <div className={styles.fleet}>
      <VisualNode label="Agents" icon={Bot} />
      <Connector />
      <VisualHub />
      <Connector order={1} />
      <VisualNode label="Browser fleet" icon={Boxes} order={2} />
      <div className={styles.human}>
        <VisualNode label="Human control" icon={Hand} order={3} />
      </div>
    </div>
  );
}
/** Edition scope stays beside the three controls instead of implying a universal HIPAA offering. */
function ControlsVisual() {
  return (
    <div className={styles.controls}>
      <VisualHub />
      <div className={styles.pillars}>
        <VisualNode label="BAA" icon={FileText} />
        <VisualNode label="Access" icon={KeyRound} order={1} />
        <VisualNode label="Audit" icon={ListChecks} order={2} />
      </div>
      <p>On Oya Cloud and the enterprise edition.</p>
    </div>
  );
}
/** Evidence documents describe requestable materials without invented seals or document contents. */
function DocumentsVisual() {
  const labels = ['Business Associate Agreement', 'Security Rule evidence', 'Risk Assessment'];
  return (
    <div className={styles.documents}>
      {labels.map((label, index) => (
        <article key={label}>
          <Reveal preset="document" order={index}>
            <FileText size={32} aria-hidden="true" />
            <strong>{label}</strong>
            <span className={styles.rule} aria-hidden="true" />
          </Reveal>
        </article>
      ))}
      <p>Available on request</p>
    </div>
  );
}
/** Healthcare tasks use the product's actual record-and-replay loop without invented success rates. */
function WorkflowVisual({ content }: VisualProps) {
  return (
    <div className={styles.workflow}>
      <span className={styles.task}>{content.label}</span>
      <VisualNode label="Run once" icon={MousePointer2} />
      <Connector />
      <VisualNode label="Save playbook" icon={FileCode2} order={1} />
      <Connector order={1} />
      <VisualNode label="Replay" icon={Repeat2} order={2} />
      <p>Oya Cloud or enterprise edition for the BAA offering.</p>
    </div>
  );
}
/** Vendor names and comparison criteria come directly from the page's existing evidence table. */
function ComparisonVisual({ content }: VisualProps) {
  const table = content.sections.find((section) => section.table)?.table;
  return (
    <div className={styles.comparison}>
      <span className={styles.comparisonLabel}>Compare approaches</span>
      <div className={styles.vendors}>
        {table?.columns.filter(Boolean).map((name, index) => (
          <Reveal key={name} preset="branch" order={index}>
            <span>{name}</span>
          </Reveal>
        ))}
      </div>
      <div className={styles.criteria}>
        {table?.rows.slice(0, VISUAL_CRITERIA_LIMIT).map(([criterion]) => (
          <div key={criterion}>
            <span aria-hidden="true" />
            {criterion}
          </div>
        ))}
      </div>
      <p>Criteria from the comparison below.</p>
    </div>
  );
}
/** A visual can read facts without importing server page behavior into a client island. */
interface VisualProps {
  /** Existing page data remains the only source of factual labels. */ content: AnswerPageContent;
}
/** A closed renderer map avoids path-based conditional chains. */
const VISUALS: Record<VisualFamily, (props: VisualProps) => React.ReactNode> = {
  desktop: DesktopVisual,
  fleet: FleetVisual,
  controls: ControlsVisual,
  documents: DocumentsVisual,
  workflow: WorkflowVisual,
  comparison: ComparisonVisual,
};

/** Meaning is present as text before hydration; decorative icons and connectors can then assemble. */
export function AnswerVisual({
  content,
  family,
}: VisualProps & { /** The explicit subject family. */ family: VisualFamily }) {
  const Visual = Object.hasOwn(VISUALS, family) ? VISUALS[family] : null;
  return (
    <figure
      className={styles.stage}
      data-visual-family={family}
      aria-label={`${content.label}: conceptual illustration`}
    >
      {Visual && <Visual content={content} />}
      <figcaption>Conceptual illustration</figcaption>
    </figure>
  );
}
