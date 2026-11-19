/** Three equally weighted technical routes replace the workspace-key-first setup. */
import { useViewModel } from '../../../hooks/index.ts';
import { useEffect } from 'react';
import { Button } from '../../../ui/index.ts';
import type { SetupViewModel } from '../view-models/setup-view-model.ts';
import { SetupChoice } from './setup-choice.tsx';
import type { Brand } from './brand-marks.tsx';
import { TechnicalAgents } from './technical-agents.tsx';
import { TechnicalModel } from './technical-model.tsx';
import './technical-setup.css';

/** Signing in works with Oya itself. */
const OYA: Brand[] = [{ name: 'Oya', logo: 'oya' }];
/** The AI apps Oya can plug into on this computer. */
const AI_APPS: Brand[] = [
  { name: 'Claude Code', logo: 'claude' },
  { name: 'Codex' },
  { name: 'Cursor', logo: 'cursor' },
  { name: 'VS Code' },
];
/** Where an AI key can come from. */
const KEY_PROVIDERS: Brand[] = [
  { name: 'Gemini', logo: 'gemini' },
  { name: 'Claude', logo: 'claude' },
  { name: 'OpenRouter', logo: 'openrouter' },
];

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
  return <TechnicalChoices vm={vm} />;
}
/** Back cancels pending credentials instead of leaving hidden work behind the next route. */
function TechnicalBack({ vm }: { /** The technical view owns its active attempt. */ vm: SetupViewModel }) {
  return (
    <Button className="welcome-back" onClick={() => vm.onboarding.technical.show('choices')}>
      ← Back
    </Button>
  );
}
/** Each full row is the same native control with a compact explanatory picture. */
function TechnicalChoices({ vm }: { /** Shared setup owner. */ vm: SetupViewModel }) {
  const flow = vm.onboarding.technical;
  const s = useViewModel(flow);
  return (
    <>
      <h2 tabIndex={-1}>How do you want to connect?</h2>
      <div className="technical-choices">
        <SetupChoice
          compact
          title="Sign in with Oya"
          caption="You need an Oya account"
          brands={OYA}
          disabled={s.busy}
          onClick={() => void flow.signIn()}
        />
        <SetupChoice
          compact
          title="Use an AI app on this computer"
          caption="You need one of these installed"
          brands={AI_APPS}
          disabled={s.busy}
          onClick={() => flow.show('agents')}
        />
        <SetupChoice
          compact
          title="Use your own AI key"
          caption="You need a key from one of these, and an Oya account"
          brands={KEY_PROVIDERS}
          disabled={s.busy}
          onClick={() => flow.show('model')}
        />
      </div>
      {s.busy && (
        <>
          <p role="status">{s.progress}</p>
          <Button onClick={() => flow.cancel()}>Cancel sign-in</Button>
        </>
      )}
      {s.error && (
        <div className="setup-error" role="alert">
          <p>{s.error}</p>
          <Button onClick={() => void flow.signIn()}>Try again</Button>
        </div>
      )}
      <Button className="technical-advanced" disabled={s.busy} onClick={() => vm.show('manual')}>
        Connect to my own server
      </Button>
    </>
  );
}
