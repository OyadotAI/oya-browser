/**
 * The route an agent calls when it has finished with a browser it shares: an SDK browser that
 * reused a person's desktop ends its lane, so the desktop closes the tab the lane worked in and
 * the person is left with the tabs they had.
 */
import { Status } from '../../../platform/http-status.ts';
import { currentLane, endLane } from '../connection/lane.ts';

/** The lanes an SDK browser names for itself; only those are a caller's to end here. */
const SDK_LANE = /^sdk-/;

/**
 * Ends the SDK lane the caller names (X-Oya-Agent or ?agent=). A lane is what a desktop gives a
 * tab for, so ending it is how the tab goes back. Only an SDK lane: an MCP agent's lane, or a
 * chat's tab, is never ended from outside, so one client cannot close another agent's tab.
 */
export function endCallerLane(req, res) {
  const lane = currentLane();
  if (!lane || !SDK_LANE.test(lane))
    return res.status(Status.BAD_REQUEST).json({ error: 'Name an SDK lane (sdk-…) to end in the X-Oya-Agent header' });
  endLane(req.params.browserId, lane);
  res.json({ ok: true, lane });
}
