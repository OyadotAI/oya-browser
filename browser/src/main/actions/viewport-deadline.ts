/** Main-process deadlines settle layout reads even when a renderer never completes its animation frame. */
import { withinTime } from '../../shared/within-time.ts';
import { VIEWPORT_READ_MS, VIEWPORT_TIMEOUT_MESSAGE } from './constants.ts';
/** The raced promise is the completion token: late renderer results cannot resume its rejected await. */
export function viewportDeadline<T>(read: Promise<T>): Promise<T> {
  return withinTime(read, VIEWPORT_READ_MS, VIEWPORT_TIMEOUT_MESSAGE);
}
