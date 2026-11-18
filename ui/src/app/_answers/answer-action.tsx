/** The same next step connects a story's opening and conclusion using existing destinations. */
import Link from 'next/link';
import { ArrowUpRight } from 'lucide-react';
import { Reveal } from '@/components/ui/reveal';
import type { AnswerPageContent } from './content';
import styles from './answers.module.css';

/** Unknown or legacy pages keep the existing documentation entry point. */
const DEFAULT_ACTION = {
  label: 'Explore Oya',
  href: '/docs/getting-started',
  invitation: 'Start with a task you know well.',
};

/** One action retains a clear purpose and measurable conversion context. */
export function AnswerAction({
  content,
  place,
}: {
  /** Existing metadata and optional narrative action. */ content: AnswerPageContent;
  /** The two intentional positions within a single story. */ place: 'hero' | 'closing';
}) {
  const action = content.hero?.action ?? DEFAULT_ACTION;
  return (
    <Link
      href={action.href}
      className={styles.start}
      data-track="cta_clicked"
      data-track-label={`answer_${place}_${content.path}`}
    >
      <span>{action.label}</span>
      <ArrowUpRight size={18} aria-hidden="true" />
    </Link>
  );
}

/** The invitation resolves the final chapter before reference questions and sources. */
export function AnswerInvitation({
  content,
}: {
  /** A page's single configured next step. */ content: AnswerPageContent;
}) {
  const action = content.hero?.action ?? DEFAULT_ACTION;
  return (
    <section className={styles.invitation} aria-label="Next step">
      <Reveal>
        <p>{action.invitation}</p>
        <AnswerAction content={content} place="closing" />
      </Reveal>
    </section>
  );
}
