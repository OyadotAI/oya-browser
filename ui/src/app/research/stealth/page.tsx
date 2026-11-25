/** /research/stealth: the same headless Chrome, bare and with a persona, against public bot detectors. */
import { PostPage, postMetadata } from '../post-page';
import { STEALTH_POST } from '../posts';

/** Its title, its summary as the description, and its canonical address. */
export const metadata = postMetadata(STEALTH_POST);

/** The stealth measurement post. */
export default function StealthResearch() {
  return <PostPage post={STEALTH_POST} />;
}
