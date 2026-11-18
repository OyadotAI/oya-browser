/** Every public answer gets a subject-specific visual without changing its factual content or metadata. */
import type { AnswerPageContent, AnswerSection } from './content';
import { PRESENTATIONS } from './presentation-config';

/** Visual families explain different product questions rather than swapping names in one illustration. */
export type VisualFamily = 'desktop' | 'fleet' | 'controls' | 'documents' | 'workflow' | 'comparison';
/** Section shapes make existing information easier to scan. */
export type { AnswerLayout as SectionLayout } from './content';
/** Explicit route categories keep visual meaning reviewable as the narrative changes. */
export interface AnswerPresentation {
  /** The conceptual illustration appropriate to the page. */ family: VisualFamily;
  /** A short editorial category, without a new product claim. */ eyebrow: string;
}

/** New or synthetic content retains complete prose until its presentation is deliberately configured. */
export function presentationOf(path: string): AnswerPresentation | undefined {
  return Object.hasOwn(PRESENTATIONS, path) ? PRESENTATIONS[path] : undefined;
}

/** Explicit story paragraphs replace brittle substring boundaries while legacy content stays readable. */
export function leadParagraphs(content: AnswerPageContent): string[] {
  return content.hero?.paragraphs ?? [content.lead];
}

/** Semantic tables always win; every other section is mapped by its actual heading. */
export function layoutOf(section: AnswerSection): import('./content').AnswerLayout {
  if (section.table) return 'comparison';
  return section.layout ?? 'prose';
}
