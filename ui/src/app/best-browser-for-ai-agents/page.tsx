/** /best-browser-for-ai-agents: Oya next to the cloud browser vendors for AI agents, from each vendor's own pages. */
import { AnswerPage, answerMetadata } from '../_answers/answer-page';
import { BEST_INFRA } from '../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(BEST_INFRA);

/** Oya next to the cloud browser vendors for AI agents, from each vendor's own pages. */
export default function BestBrowserForAiAgents() {
  return <AnswerPage content={BEST_INFRA} />;
}
