/**
 * The Oya orb (styled by orb.css): the spinning halo and rim, the tilted
 * orbit, the glass lens with the Oya mark, and the pulse ring. Its `state`
 * says what the agent is doing; `pulse` sends one ring of light out.
 */
import { useEffect, useRef } from 'react';
import './orb.css';

/** The orb's states, as orb.css draws them. */
export const ORB_STATES = ['idle', 'thinking', 'acting', 'done', 'failed'] as const;

/** What the orb shows. */
export type OrbState = (typeof ORB_STATES)[number];

/** The orb's sizes: extra small to extra large; '' is medium. */
export type OrbSize = 'xs' | 'sm' | '' | 'lg' | 'xl';

/** The mark's geometry inside the lens: two rings, the back one offset down and right. */
const MARK = {
  viewBox: '0 0 100 100',
  r: 19,
  width: 9.5,
  rings: [
    { name: 'mark-back', c: 53 },
    { name: 'mark-front', c: 47 },
  ],
} as const;

/** A known state, or idle for anything else. */
export const orbState = (state: string | undefined): OrbState =>
  ORB_STATES.includes(state as OrbState) ? (state as OrbState) : 'idle';

/** What an orb is told. */
export interface OrbProps {
  /** Its size. */
  size?: OrbSize;
  /** What it shows (an unknown state rests it). */
  state?: string;
  /** The element's id, for the places styled or tested by it. */
  id?: string;
  /** Extra classes. */
  className?: string;
  /** Changes to this send one pulse ring out (a counter; 0 sends none). */
  pulse?: number;
}

/** Restarts the pulse ring's animation each time `pulse` changes. */
function usePulse(pulse: number) {
  const ring = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    const el = ring.current;
    if (!pulse || !el) return;
    el.classList.remove('go');
    void el.offsetWidth; // a reflow forgets the finished animation
    el.classList.add('go');
  }, [pulse]);
  return ring;
}

/** The Oya mark inside the lens (it turns while Oya thinks, so it is drawn here). */
function OrbMark() {
  return (
    <svg className="oya-orb-mark" viewBox={MARK.viewBox}>
      {MARK.rings.map(({ name, c }) => (
        <circle key={name} className={name} cx={c} cy={c} r={MARK.r} fill="none" strokeWidth={MARK.width} />
      ))}
    </svg>
  );
}

/** One orb. */
export function Orb({ size = '', state, id, className, pulse = 0 }: OrbProps) {
  const ring = usePulse(pulse);
  return (
    <span
      id={id}
      className={className ? `oya-orb ${className}` : 'oya-orb'}
      data-size={size || undefined}
      data-state={orbState(state)}
      aria-hidden="true"
    >
      <span className="oya-orb-spin">
        <span className="oya-orb-halo" />
        <span className="oya-orb-rim" />
      </span>
      <span className="oya-orb-orbit">
        <i />
      </span>
      <span className="oya-orb-lens">
        <span className="oya-orb-core">
          <i />
          <i />
        </span>
        <OrbMark />
      </span>
      <span className="oya-orb-pulse" ref={ring} />
    </span>
  );
}
