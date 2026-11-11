/** /docs/self-hosting: the self-hosting part of the docs as its own page. */
import { TopicPage, topicMetadata } from '../_docs/topic-page';
import { SelfHostingDocs } from '../_sections/self-hosting';

/** Its own title, description and canonical address. */
export const metadata = topicMetadata('self-hosting');

/** The self-hosting docs page. */
export default function SelfHostingTopic() {
  return (
    <TopicPage slug="self-hosting">
      <SelfHostingDocs />
    </TopicPage>
  );
}
