/** /docs/dashboard: the dashboard part of the docs as its own page. */
import { TopicPage, topicMetadata } from '../_docs/topic-page';
import { DashboardDocs } from '../_sections/dashboard';

/** Its own title, description and canonical address. */
export const metadata = topicMetadata('dashboard');

/** The dashboard docs page. */
export default function DashboardTopic() {
  return (
    <TopicPage slug="dashboard">
      <DashboardDocs />
    </TopicPage>
  );
}
