/** The detail screens behind the connect screen: AI apps on this computer, and your own AI key. */
import { useViewModel } from '../../../hooks/index.ts';
import { useEffect } from 'react';
import { Button } from '../../../ui/index.ts';
import type { SetupViewModel } from '../view-models/setup-view-model.ts';
import { TechnicalAgents } from './technical-agents.tsx';
import { TechnicalModel } from './technical-model.tsx';
import './technical-setup.css';

/** Technical-only detail navigation preserves the other onboarding paths. */
export function TechnicalFlow({ vm }: { /** Existing manual connection and technical state. */ vm: SetupViewModel }) {
  const flow = vm.onboarding.technical;
  useEffect(() => () => flow.cancel(), [flow]);
  const s = useViewModel(flow);
  if (s.view === 'agents')
    return (
      <>
        <TechnicalBack vm={vm} />
        <TechnicalAgents vm={flow.agents} finish={() => vm.onboarding.finish()} />
      </>
    );
  if (s.view === 'model')
    return (
      <>
        <TechnicalBack vm={vm} />
        <TechnicalModel vm={flow.model} />
      </>
    );
  return null;
}
/** Back returns to the connect screen and cancels pending credentials, so no hidden work is left behind. */
function TechnicalBack({ vm }: { /** The technical view owns its active attempt. */ vm: SetupViewModel }) {
  return (
    <Button className="welcome-back" onClick={() => vm.onboarding.choose(null)}>
      ← Back
    </Button>
  );
}
