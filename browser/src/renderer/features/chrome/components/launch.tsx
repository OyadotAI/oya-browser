/**
 * The launch stage (index.html's #launch): up from the first paint, ended
 * by its timer or at once by a click or a key. As it dissolves, its orb
 * glides and scales onto the orb of whatever shows under it (the start
 * page's, or the welcome screen's), so the two read as one.
 */
import { useEffect, useLayoutEffect, useRef } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { Orb } from '../../../ui/index.ts';
import './launch.css';
import type { LaunchViewModel } from '../view-models/launch-view-model.ts';
import { WELCOME_ORB } from '../model/constants.ts';

/** What the launch is given. */
export interface LaunchProps {
  /** The launch's ViewModel. */
  vm: LaunchViewModel;
}

/** The orb the launch hands over to: the start page's, or the welcome screen's; none when neither shows. */
function landing(): Element | null {
  const start = document.getElementById('start-page');
  return (start && !start.hidden && document.getElementById('start-orb')) || document.querySelector(WELCOME_ORB);
}

/** Moves and scales `orb` onto `target`, so the two are one orb as the stage dissolves. */
function handOff(orb: HTMLElement | null, target: Element | null): void {
  const [from, to] = [orb?.getBoundingClientRect(), target?.getBoundingClientRect()];
  if (!orb || !from?.width || !to?.width) return;
  orb.style.transform = `translate(${to.left - from.left}px, ${to.top - from.top}px) scale(${to.width / from.width})`;
}

/** A click or a key ends the launch at once. */
function useEndOnInput(vm: LaunchViewModel): void {
  useEffect(() => {
    const end = () => vm.end();
    for (const type of ['pointerdown', 'keydown']) window.addEventListener(type, end, { once: true });
    return () => ['pointerdown', 'keydown'].forEach((type) => window.removeEventListener(type, end));
  }, [vm]);
}

/** The launch stage. */
export function Launch({ vm }: LaunchProps) {
  const { phase } = useViewModel(vm);
  const stage = useRef<HTMLDivElement>(null);
  useEndOnInput(vm);
  useLayoutEffect(() => {
    if (phase !== 'leaving' || !stage.current) return;
    // The orb's entrance is off once the stage is leaving; a layout pass first lets its move be a transition.
    void stage.current.offsetWidth;
    handOff(stage.current.querySelector<HTMLElement>('#launch-orb'), landing());
  }, [phase]);
  return (
    <div
      className={phase === 'gone' ? 'launch' : `launch ${phase}`}
      id="launch"
      aria-hidden="true"
      hidden={phase === 'gone'}
      ref={stage}
    >
      <div className="launch-edge">
        <i></i>
      </div>
      <svg className="launch-rings" viewBox="0 0 512 512">
        <circle className="launch-ring back" cx="276" cy="276" r="160" />
        <circle className="launch-ring front" cx="236" cy="236" r="160" />
      </svg>
      <Orb size="xl" id="launch-orb" />
      <div className="launch-word">Oya</div>
    </div>
  );
}
