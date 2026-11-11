/** /docs/api: the API part of the docs as its own page. */
import { TopicPage, topicMetadata } from '../_docs/topic-page';
import { ApiDocs } from '../_sections/api';

/** Its own title, description and canonical address. */
export const metadata = topicMetadata('api');

/** The API docs page. */
export default function ApiTopic() {
  return (
    <TopicPage slug="api">
      <ApiDocs />
    </TopicPage>
  );
}
