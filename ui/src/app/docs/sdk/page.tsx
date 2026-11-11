/** /docs/sdk: the SDK part of the docs as its own page. */
import { TopicPage, topicMetadata } from '../_docs/topic-page';
import { SdkDocs } from '../_sections/getting-started';

/** Its own title, description and canonical address. */
export const metadata = topicMetadata('sdk');

/** The SDK docs page. */
export default function SdkTopic() {
  return (
    <TopicPage slug="sdk">
      <SdkDocs />
    </TopicPage>
  );
}
