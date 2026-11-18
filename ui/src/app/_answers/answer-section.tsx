/** Existing answer sections receive flow, choice, control or reading layouts without losing their qualifications. */
import { ArrowRight, Check, Shield } from 'lucide-react';
import { Reveal } from '@/components/ui/reveal';
import type { ReactNode } from 'react';
import type { AnswerSection as ContentSection } from './content';
import { layoutOf } from './presentation';
import { AiBrowserSection } from './ai-browser-sections';
import styles from './answers.module.css';

/** Lists retain every original sentence while icons and grouping make their purpose easier to scan. */
function SectionItems({ section }: { /** Complete original section. */ section: ContentSection }) {
  const layout = layoutOf(section),
    Icon = layout === 'flow' ? ArrowRight : layout === 'controls' ? Shield : Check;
  return (
    <ul className={styles.items} data-layout={layout}>
      {section.items?.map((item, index) => (
        <li key={item}>
          <Reveal preset="public" order={index}>
            <Icon size={20} aria-hidden="true" />
            <span>{item}</span>
          </Reveal>
        </li>
      ))}
    </ul>
  );
}

/** Tables are supplied by the semantic table renderer so their rows and accessibility remain unchanged. */
export function AnswerSection({
  section,
  children,
}: {
  /** Existing section text and data. */ section: ContentSection;
  /** Caller context is retained while composition metadata selects the visual. */ path: string;
  /** A complete comparison table, when applicable. */ children?: ReactNode;
}) {
  return (
    <section className={styles.section} data-section-layout={layoutOf(section)}>
      <Reveal preset="public" className={styles.sectionHeading}>
        <h2>{section.heading}</h2>
      </Reveal>
      <div className={styles.sectionBody}>
        {section.composition ? (
          <AiBrowserSection section={section} />
        ) : (
          <>
            <p>{section.body}</p>
            {section.items && <SectionItems section={section} />}
          </>
        )}
        {children}
      </div>
    </section>
  );
}
