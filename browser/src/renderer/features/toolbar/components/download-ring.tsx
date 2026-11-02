/**
 * The download chip's progress ring, in the Downloads button's icon slot: how
 * much of what is running has arrived, or a turning quarter when a site gave
 * no size. Decorative: the chip's accessible name says the same in words.
 */
import { RING_CENTRE, RING_INDETERMINATE, RING_LENGTH, RING_RADIUS, RING_SIZE } from '../model/constants.ts';

/** What the ring is given. */
export interface DownloadRingProps {
  /** The share arrived, from 0 to 1, or null when unknown. */
  share: number | null;
}

/** A 14px ring: a faint track and an accent arc from twelve o'clock, clockwise. */
export function DownloadRing({ share }: DownloadRingProps) {
  const [length, centre] = [RING_LENGTH, RING_CENTRE];
  const shown = share ?? RING_INDETERMINATE;
  return (
    <svg
      className={`download-ring${share === null ? ' turning' : ''}`}
      viewBox={`0 0 ${RING_SIZE} ${RING_SIZE}`}
      aria-hidden="true"
    >
      <circle className="download-ring-track" cx={centre} cy={centre} r={RING_RADIUS} />
      <circle
        className="download-ring-arc"
        cx={centre}
        cy={centre}
        r={RING_RADIUS}
        strokeDasharray={`${length * shown} ${length}`}
        transform={`rotate(-90 ${centre} ${centre})`}
      />
    </svg>
  );
}
