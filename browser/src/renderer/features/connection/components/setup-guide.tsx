/** The walkthrough can be reopened in Settings without resetting the browser connection. */
import { useRef, useState } from 'react';
import { Button } from '../../../ui/index.ts';
import { SETUP_GUIDE } from '../model/setup-constants.ts';
import { SetupIllustration } from './setup-illustration.tsx';

/** Steps are explicitly advanced by the person, never inferred from opening a URL. */
export function GuideSteps({
  step,
  move,
  finish,
  openConsole,
}: {
  /** Bounded position. */ step: number;
  /** Step navigation. */ move: (delta: number) => void;
  /** Skip or finish. */ finish: () => void;
  /** Current workspace destination. */ openConsole: () => void;
}) {
  const item = SETUP_GUIDE[step];
  return (
    <section className="setup-guide">
      <p className="eyebrow">
        Control plane · {step + 1} of {SETUP_GUIDE.length}
      </p>
      <h2 tabIndex={-1}>{item.title}</h2>
      <figure className="setup-guide-picture">
        <SetupIllustration scene={item.scene} />
        <figcaption>Illustration</figcaption>
      </figure>
      <p>{item.detail}</p>
      <Button variant="secondary" onClick={openConsole}>
        Open control plane
      </Button>
      <nav aria-label="Setup guide">
        <Button disabled={!step} onClick={() => move(-1)}>
          Back
        </Button>
        <Button variant="primary" onClick={() => (step === SETUP_GUIDE.length - 1 ? finish() : move(1))}>
          {step === SETUP_GUIDE.length - 1 ? 'Finish guide' : 'Next'}
        </Button>
        <Button onClick={finish}>Skip walkthrough</Button>
      </nav>
    </section>
  );
}

/** Existing Settings owns focus trapping and native view stacking. */
export function SettingsSetupGuide({
  openConsole,
}: {
  /** Opens main's current workspace. */ openConsole: () => void;
}) {
  const [step, setStep] = useState(0);
  const details = useRef<HTMLDetailsElement>(null);
  const finish = () => {
    setStep(0);
    if (details.current) details.current.open = false;
  };
  return (
    <details className="settings-setup-guide" ref={details}>
      <summary>Setup guide</summary>
      <GuideSteps step={step} move={(delta) => setStep(step + delta)} finish={finish} openConsole={openConsole} />
    </details>
  );
}
