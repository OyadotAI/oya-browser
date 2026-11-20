/** /vs/firecrawl: Oya next to Firecrawl, from each vendor's own public pages. */
import { AnswerPage, answerMetadata } from '../../_answers/answer-page';
import { VS_FIRECRAWL } from '../../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(VS_FIRECRAWL);

/** The Oya vs Firecrawl page. */
export default function VsFirecrawl() {
  return <AnswerPage content={VS_FIRECRAWL} />;
}
