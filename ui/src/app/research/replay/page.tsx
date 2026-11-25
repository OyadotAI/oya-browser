/** /research/replay: how well a recorded run replays with no model and healing off. */
import { PostPage, postMetadata } from '../post-page';
import { REPLAY_POST } from '../posts';

/** Its title, its summary as the description, and its canonical address. */
export const metadata = postMetadata(REPLAY_POST);

/** The replay benchmark post. */
export default function ReplayResearch() {
  return <PostPage post={REPLAY_POST} />;
}
