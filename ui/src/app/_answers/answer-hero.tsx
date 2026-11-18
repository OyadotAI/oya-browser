/** A task-led opening gives the reader a reason to follow the story beside its existing illustration. */
import { Reveal } from '@/components/ui/reveal';
import type { AnswerPageContent } from './content';
import { leadParagraphs, presentationOf } from './presentation';
import { AnswerVisual } from './answer-visual';
import { AnswerAction } from './answer-action';
import styles from './answers.module.css';

/** Visible narrative can change while the established search title and description remain stable. */
export function AnswerHero({ content }: { /** Existing factual page content. */ content: AnswerPageContent }) {
  const presentation = presentationOf(content.path);
  return (
    <header className={styles.hero} data-answer-hero>
      <div className={styles.heroCopy}>
        <Reveal preset="intro">
          <span className={styles.eyebrow}>{presentation?.eyebrow ?? 'Oya Browser'}</span>
          <h1>{content.hero?.headline ?? content.title}</h1>
        </Reveal>
        <Reveal preset="intro" order={1} className={styles.lead}>
          {leadParagraphs(content).map((paragraph) => (
            <p key={paragraph} data-lead>
              {paragraph}
            </p>
          ))}
        </Reveal>
        <AnswerAction content={content} place="hero" />
      </div>
      {presentation && <AnswerVisual content={content} family={presentation.family} />}
    </header>
  );
}
