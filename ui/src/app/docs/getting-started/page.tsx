/** /docs/getting-started: the getting started part of the docs as its own page. */
import { TopicPage, topicMetadata } from '../_docs/topic-page';
import { GetStartedDocs } from '../_sections/getting-started';

/** Its own title, description and canonical address. */
export const metadata = topicMetadata('getting-started');

/** The getting started docs page. */
export default function GetStartedTopic() {
  return (
    <TopicPage slug="getting-started">
      <GetStartedDocs />
    </TopicPage>
  );
}
